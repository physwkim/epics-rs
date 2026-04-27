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
//! [`crate::client_native::ops::op_monitor`] for the loop that re-issues
//! INIT/START on each new server conn.

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
}

/// How a channel resolves its PV name to a server address.
enum Resolver {
    /// Use the SearchEngine — full UDP search + retry + beacon listener.
    Search(SearchEngine),
    /// Connect directly to a known address (used by `PvaClientBuilder::server_addr`).
    Direct(std::net::SocketAddr),
}

/// Pool of live `ServerConn`s, keyed by server address.
#[derive(Default)]
pub struct ConnectionPool {
    inner: parking_lot::Mutex<std::collections::HashMap<std::net::SocketAddr, Arc<ServerConn>>>,
}

impl ConnectionPool {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
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
        let fresh = ServerConn::connect(addr, user, host, op_timeout).await?;
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

        self.set_state(ChannelState::Searching);
        let server_addr = match &self.resolver {
            Resolver::Search(engine) => engine.find(&self.pv_name).await?,
            Resolver::Direct(addr) => *addr,
        };

        self.set_state(ChannelState::Connecting);
        let server = self
            .pool
            .get_or_connect(server_addr, &self.user, &self.host, self.op_timeout)
            .await?;

        // Send CREATE_CHANNEL and wait for response.
        let sid = self.do_create_channel(&server).await?;

        self.set_state(ChannelState::Active {
            server: server.clone(),
            sid,
        });
        Ok((server, sid))
    }

    fn set_state(&self, new_state: ChannelState) {
        *self.state.write() = new_state;
        self.state_changed.notify_waiters();
    }

    async fn do_create_channel(&self, server: &Arc<ServerConn>) -> PvaResult<u32> {
        use crate::codec::PvaCodec;
        use super::decode::decode_create_channel_response;

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
