//! Per-PV channel state machine.
//!
//! A [`Channel`] is the long-lived handle through which ops (GET / PUT /
//! MONITOR / RPC) reach a server. Internally:
//!
//! ```text
//!   Idle
//!     │  ensure_active()
//!     ▼
//!   Searching ────► Connecting ────► Active
//!     ▲                                 │
//!     │  ServerConn closed              │
//!     └─────────────────────────────────┘
//! ```
//!
//! Multiple ops can ride on the same channel concurrently: each gets a
//! fresh `ioid` and registers with the underlying [`ServerConn`] router.
//! Reconnect is **automatic** and transparent to monitor consumers — see
//! [`crate::client_native::ops_v2::op_monitor_handle`] for the loop that
//! re-issues INIT/START on each new server conn.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use parking_lot::RwLock;
use tokio::sync::{Mutex, Notify};

use crate::error::{PvaError, PvaResult};

use super::beacon_throttle::BeaconTracker;
use super::search_engine::SearchEngine;
use super::server_conn::ServerConn;

static NEXT_CID: AtomicU32 = AtomicU32::new(1);

#[derive(Clone)]
pub enum ChannelState {
    Idle,
    Searching,
    Connecting,
    Active { server: Arc<ServerConn>, sid: u32 },
    Closed,
}

pub struct Channel {
    pub pv_name: String,
    pub cid: u32,
    state: RwLock<ChannelState>,
    /// Serializes state transitions so concurrent ops don't open multiple
    /// connections.
    transition_lock: Mutex<()>,
    /// Pulsed whenever the state changes. Monitor loops await this to learn
    /// of disconnect / reconnect.
    pub state_changed: Notify,
    user: String,
    host: String,
    op_timeout: std::time::Duration,
    /// Shared connection pool (so multiple channels to the same server
    /// share a single TCP virtual circuit).
    pool: Arc<ConnectionPool>,
    resolver: Resolver,
    /// Alternative server addresses cached from the most recent search
    /// (excluding the one currently being tried). Multi-server failover:
    /// if `ensure_active` fails to connect or `CREATE_CHANNEL`s to the
    /// first server, it pops the next alternative before falling back to
    /// a fresh UDP search.
    alternatives: parking_lot::Mutex<Vec<std::net::SocketAddr>>,
    /// Earliest instant at which `ensure_active` is allowed to attempt a
    /// fresh connect. Set after a connect/CreateChannel failure to throttle
    /// reconnect storms — pvxs Channel::disconnect (client.cpp:155-163)
    /// pushes Connecting-stage failures 10 buckets (≈10 s) into the
    /// future. We accumulate exponentially per consecutive failure with a
    /// hard cap so a flapping server can't make every monitor caller spin.
    holdoff_until: parking_lot::Mutex<Option<std::time::Instant>>,
    /// Consecutive connect failures since the last successful Active
    /// transition. Used to scale `holdoff_until`.
    connect_fail_count: std::sync::atomic::AtomicU32,
    /// TCP `EPICS_PVA_NAME_SERVERS` fallbacks. Tried in order when UDP
    /// search yields no candidates. Empty for direct-mode and for
    /// channels created without name-server config.
    name_servers: Vec<std::net::SocketAddr>,
}

/// How a channel resolves its PV name to a server address.
enum Resolver {
    /// Use the SearchEngine — full UDP search + retry + beacon listener.
    Search(SearchEngine),
    /// Connect directly to a known address (used by `PvaClientBuilder::server_addr`).
    Direct(std::net::SocketAddr),
}

/// Pool of live `ServerConn`s, keyed by server address.
///
/// Optionally configured with a TLS client config — when present, every
/// new connection is upgraded to TLS via `pvas://` semantics.
#[derive(Default)]
pub struct ConnectionPool {
    inner: parking_lot::Mutex<std::collections::HashMap<std::net::SocketAddr, Arc<ServerConn>>>,
    tls: parking_lot::Mutex<Option<Arc<crate::auth::TlsClientConfig>>>,
}

impl ConnectionPool {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Enable TLS for every subsequent connect call.
    pub fn set_tls(&self, tls: Option<Arc<crate::auth::TlsClientConfig>>) {
        *self.tls.lock() = tls;
    }

    pub async fn get_or_connect(
        self: &Arc<Self>,
        addr: std::net::SocketAddr,
        user: &str,
        host: &str,
        op_timeout: std::time::Duration,
    ) -> PvaResult<Arc<ServerConn>> {
        // Fast path: existing alive conn.
        {
            let map = self.inner.lock();
            if let Some(conn) = map.get(&addr).cloned() {
                if conn.is_alive() {
                    return Ok(conn);
                }
            }
        }
        // Drop dead entry and connect fresh.
        {
            let mut map = self.inner.lock();
            if let Some(conn) = map.get(&addr) {
                if !conn.is_alive() {
                    map.remove(&addr);
                }
            }
        }
        let tls = self.tls.lock().clone();
        let fresh = match tls {
            Some(cfg) => {
                ServerConn::connect_tls(addr, &addr.ip().to_string(), cfg, user, host, op_timeout)
                    .await?
            }
            None => ServerConn::connect(addr, user, host, op_timeout).await?,
        };
        let mut map = self.inner.lock();
        // Race: someone else may have inserted. Prefer an alive existing one.
        if let Some(existing) = map.get(&addr).cloned() {
            if existing.is_alive() {
                return Ok(existing);
            }
        }
        map.insert(addr, fresh.clone());
        Ok(fresh)
    }

    pub fn close_dead(&self) {
        let mut map = self.inner.lock();
        map.retain(|_, conn| conn.is_alive());
    }

    /// Drop every cached connection. The underlying `ServerConn`s live
    /// only as long as some `Arc` to them is held, so callers should
    /// already have dropped any operation handles that hold a clone.
    /// Used by `PvaClient::close` for explicit shutdown.
    pub fn clear(&self) {
        self.inner.lock().clear();
    }
}

impl Channel {
    pub fn new(
        pv_name: String,
        user: String,
        host: String,
        op_timeout: std::time::Duration,
        pool: Arc<ConnectionPool>,
        search: SearchEngine,
    ) -> Self {
        Self::new_with_name_servers(pv_name, user, host, op_timeout, pool, search, Vec::new())
    }

    /// Like [`Self::new`] but also accepts a TCP name-server fallback
    /// list. Pinged in order whenever UDP search yields no candidates.
    pub fn new_with_name_servers(
        pv_name: String,
        user: String,
        host: String,
        op_timeout: std::time::Duration,
        pool: Arc<ConnectionPool>,
        search: SearchEngine,
        name_servers: Vec<std::net::SocketAddr>,
    ) -> Self {
        Self {
            pv_name,
            cid: NEXT_CID.fetch_add(1, Ordering::Relaxed),
            state: RwLock::new(ChannelState::Idle),
            transition_lock: Mutex::new(()),
            state_changed: Notify::new(),
            user,
            host,
            op_timeout,
            pool,
            resolver: Resolver::Search(search),
            alternatives: parking_lot::Mutex::new(Vec::new()),
            holdoff_until: parking_lot::Mutex::new(None),
            connect_fail_count: std::sync::atomic::AtomicU32::new(0),
            name_servers,
        }
    }

    /// Construct a channel that targets a fixed server address (no UDP search).
    pub fn new_direct(
        pv_name: String,
        user: String,
        host: String,
        op_timeout: std::time::Duration,
        pool: Arc<ConnectionPool>,
        addr: std::net::SocketAddr,
    ) -> Self {
        Self {
            pv_name,
            cid: NEXT_CID.fetch_add(1, Ordering::Relaxed),
            state: RwLock::new(ChannelState::Idle),
            transition_lock: Mutex::new(()),
            state_changed: Notify::new(),
            user,
            host,
            op_timeout,
            pool,
            resolver: Resolver::Direct(addr),
            alternatives: parking_lot::Mutex::new(Vec::new()),
            holdoff_until: parking_lot::Mutex::new(None),
            connect_fail_count: std::sync::atomic::AtomicU32::new(0),
            name_servers: Vec::new(),
        }
    }

    pub fn current_state(&self) -> ChannelState {
        self.state.read().clone()
    }

    pub fn is_active(&self) -> bool {
        matches!(*self.state.read(), ChannelState::Active { ref server, .. } if server.is_alive())
    }

    pub fn close(&self) {
        let mut s = self.state.write();
        *s = ChannelState::Closed;
        self.state_changed.notify_waiters();
    }

    /// Ensure the channel is in `Active` state, transitioning through
    /// Searching → Connecting as needed. Returns the live `(ServerConn, sid)`
    /// pair.
    pub async fn ensure_active(&self) -> PvaResult<(Arc<ServerConn>, u32)> {
        // Quick happy-path check.
        {
            let s = self.state.read();
            if let ChannelState::Active { server, sid } = &*s {
                if server.is_alive() {
                    return Ok((server.clone(), *sid));
                }
            }
            if let ChannelState::Closed = &*s {
                return Err(PvaError::Protocol("channel closed".into()));
            }
        }

        // Serialize transitions across concurrent callers.
        let _guard = self.transition_lock.lock().await;

        // Connect-fail holdoff. After a recent connect/CreateChannel
        // failure we sleep for the remainder of the holdoff window
        // before re-issuing the search. pvxs Channel::disconnect
        // (client.cpp:155-163) implements the same idea with a
        // 10-bucket future-push on the search ring; here we
        // accumulate `2^min(fails-1, 4)` seconds (cap 16s) per
        // consecutive failure. Reset to zero on the next successful
        // Active transition.
        let now = std::time::Instant::now();
        let wait = {
            let mut h = self.holdoff_until.lock();
            match *h {
                Some(t) if t > now => Some(t - now),
                _ => {
                    *h = None;
                    None
                }
            }
        };
        if let Some(d) = wait {
            tokio::time::sleep(d).await;
        }

        // Re-check after acquiring the lock.
        {
            let s = self.state.read();
            if let ChannelState::Active { server, sid } = &*s {
                if server.is_alive() {
                    return Ok((server.clone(), *sid));
                }
            }
            if let ChannelState::Closed = &*s {
                return Err(PvaError::Protocol("channel closed".into()));
            }
        }

        // Pull a candidate server. Prefer cached alternatives from the
        // most recent multi-window search; otherwise issue a fresh search.
        // The lock guard from parking_lot is !Send, so we drop it before
        // any await.
        let cached: Option<Vec<std::net::SocketAddr>> = {
            let mut alts = self.alternatives.lock();
            if alts.is_empty() {
                None
            } else {
                Some(std::mem::take(&mut *alts))
            }
        };
        let mut candidates = match cached {
            Some(list) => list,
            None => {
                self.set_state(ChannelState::Searching);
                match &self.resolver {
                    Resolver::Search(engine) => {
                        engine.find_all(&self.pv_name).await.unwrap_or_default()
                    }
                    Resolver::Direct(addr) => vec![*addr],
                }
            }
        };

        // Append TCP name servers as final fallback candidates. pvxs
        // sends real SEARCH frames over a persistent TCP connection to
        // each name server (clientconn.cpp). For the common gateway-
        // self-serve case (gateway answers for any PV it proxies)
        // direct-connect to the name server's TCP port works
        // identically. Redirect-style chains aren't supported.
        if !self.name_servers.is_empty() {
            for ns in &self.name_servers {
                if !candidates.contains(ns) {
                    candidates.push(*ns);
                }
            }
        }

        if candidates.is_empty() {
            return Err(PvaError::Protocol("no servers found for PV".into()));
        }

        // Try each candidate in order; stash the rest as alternatives.
        let mut last_err: Option<PvaError> = None;
        for (idx, server_addr) in candidates.iter().enumerate() {
            self.set_state(ChannelState::Connecting);
            match self
                .pool
                .get_or_connect(*server_addr, &self.user, &self.host, self.op_timeout)
                .await
            {
                Err(e) => {
                    last_err = Some(e);
                    continue;
                }
                Ok(server) => match self.do_create_channel(&server).await {
                    Ok(sid) => {
                        // Stash remaining candidates as alternatives.
                        let leftovers: Vec<_> = candidates.iter().skip(idx + 1).copied().collect();
                        *self.alternatives.lock() = leftovers;
                        self.set_state(ChannelState::Active {
                            server: server.clone(),
                            sid,
                        });
                        // Successful Active — clear holdoff state so the
                        // next eventual disconnect starts the backoff
                        // ladder from scratch instead of inheriting an
                        // old failure count.
                        self.connect_fail_count
                            .store(0, std::sync::atomic::Ordering::Relaxed);
                        *self.holdoff_until.lock() = None;
                        return Ok((server, sid));
                    }
                    Err(e) => {
                        last_err = Some(e);
                        continue;
                    }
                },
            }
        }
        // Every candidate failed (search returned but TCP setup or
        // CREATE_CHANNEL bounced). Bump the consecutive-failure counter
        // and arm the holdoff window for the next call.
        let fails = self
            .connect_fail_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        let secs = 1u64 << fails.saturating_sub(1).min(4); // 1, 2, 4, 8, 16
        *self.holdoff_until.lock() =
            Some(std::time::Instant::now() + std::time::Duration::from_secs(secs));
        Err(last_err.unwrap_or_else(|| PvaError::Protocol("connect failed".into())))
    }

    fn set_state(&self, new_state: ChannelState) {
        *self.state.write() = new_state;
        self.state_changed.notify_waiters();
    }

    async fn do_create_channel(&self, server: &Arc<ServerConn>) -> PvaResult<u32> {
        use super::decode::decode_create_channel_response;
        use crate::codec::PvaCodec;

        let big_endian = matches!(server.byte_order, crate::proto::ByteOrder::Big);
        let codec = PvaCodec { big_endian };
        let req = codec.build_create_channel(self.cid, &self.pv_name);

        // Register a one-shot waiter for the CREATE_CHANNEL response.
        let waiter = server.register_cid_waiter(self.cid);
        server.send(req).await?;

        let frame = tokio::time::timeout(self.op_timeout, waiter)
            .await
            .map_err(|_| PvaError::Timeout)?
            .map_err(|_| PvaError::Protocol("create_channel response cancelled".into()))?;

        let resp = decode_create_channel_response(&frame)?;
        if !resp.status.is_success() {
            return Err(PvaError::Protocol(format!(
                "create_channel({}) failed: {:?}",
                self.pv_name, resp.status
            )));
        }
        Ok(resp.sid)
    }

    /// Wait until the channel transitions out of its current `Active` state
    /// (i.e. the `ServerConn` died). Used by monitor loops to drive
    /// reconnect.
    pub async fn wait_until_inactive(&self) {
        loop {
            let notify = self.state_changed.notified();
            if !self.is_active() {
                return;
            }
            notify.await;
        }
    }
}

// Used by tests / external code that wants to inspect throttle status.
impl Channel {
    pub fn beacon_tracker(&self) -> Option<Arc<BeaconTracker>> {
        match &self.resolver {
            Resolver::Search(engine) => Some(engine.beacons.clone()),
            Resolver::Direct(_) => None,
        }
    }
}
