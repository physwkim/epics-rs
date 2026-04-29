//! Per-peer book-keeping for [`crate::server_native::PvaServer::report`].
//!
//! Mirrors pvxs `Server::report()` at the "live peers + per-peer
//! channel/op counts" granularity. The accept loop registers an entry
//! when it accepts a connection; the per-connection task updates the
//! mutable counters as it processes commands; the entry is removed on
//! disconnect.
//!
//! Lock granularity: the registry is a [`parking_lot::RwLock`] over a
//! [`std::collections::HashMap`]. Mutations (insert / remove / update)
//! take the write lock briefly; the [`PvaServer::report`] read takes
//! the read lock for the snapshot. Concurrent connection handlers
//! never block each other on this lock — each holds its own
//! [`Arc<PeerEntry>`] and updates its own atomic counters without
//! re-entering the registry.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

/// Per-connection counters held in [`PeerRegistry`].
///
/// Counters are [`AtomicU64`] so the connection handler can update them
/// without locking the registry. `connected_at` is set once at
/// registration; the rest grow over the connection's lifetime.
#[derive(Debug)]
pub struct PeerEntry {
    /// When the connection was accepted (server clock).
    pub connected_at: SystemTime,
    /// Last time the read loop bumped its rx watermark (Unix nanos).
    pub last_rx_nanos: AtomicU64,
    /// Live channels currently open on this connection.
    pub channels: AtomicU64,
    /// Total CREATE_CHANNEL successes since connect (resets to 0
    /// across reconnects since the entry is replaced).
    pub channels_created: AtomicU64,
    /// Total operation INITs (GET / PUT / MONITOR / RPC) seen.
    pub ops_init: AtomicU64,
    /// Total bytes read off the socket.
    pub bytes_in: AtomicU64,
    /// Total bytes pushed into the writer mpsc.
    pub bytes_out: AtomicU64,
    /// Whether TLS is in effect for this connection (recorded at
    /// accept). pvxs surfaces `secure` similarly.
    pub tls: bool,
}

impl PeerEntry {
    pub(crate) fn new(tls: bool) -> Arc<Self> {
        Arc::new(Self {
            connected_at: SystemTime::now(),
            last_rx_nanos: AtomicU64::new(now_nanos()),
            channels: AtomicU64::new(0),
            channels_created: AtomicU64::new(0),
            ops_init: AtomicU64::new(0),
            bytes_in: AtomicU64::new(0),
            bytes_out: AtomicU64::new(0),
            tls,
        })
    }

    pub(crate) fn touch_rx(&self, n: usize) {
        self.last_rx_nanos.store(now_nanos(), Ordering::Relaxed);
        self.bytes_in.fetch_add(n as u64, Ordering::Relaxed);
    }

    pub(crate) fn touch_tx(&self, n: usize) {
        self.bytes_out.fetch_add(n as u64, Ordering::Relaxed);
    }

    pub(crate) fn channel_added(&self) {
        self.channels.fetch_add(1, Ordering::Relaxed);
        self.channels_created.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn channel_removed(&self) {
        self.channels.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn op_init(&self) {
        self.ops_init.fetch_add(1, Ordering::Relaxed);
    }
}

/// Concurrent map of `SocketAddr → Arc<PeerEntry>`. The accept loop
/// inserts on connect and removes on disconnect; the
/// `PvaServer::report()` reader snapshots without blocking writers.
#[derive(Debug, Default)]
pub struct PeerRegistry {
    inner: parking_lot::RwLock<HashMap<SocketAddr, Arc<PeerEntry>>>,
}

impl PeerRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub(crate) fn insert(&self, peer: SocketAddr, entry: Arc<PeerEntry>) {
        self.inner.write().insert(peer, entry);
    }

    pub(crate) fn remove(&self, peer: SocketAddr) {
        self.inner.write().remove(&peer);
    }

    /// Snapshot the registry into a Vec of (peer, snapshot) pairs.
    /// Cloned out so the caller doesn't hold the read lock across
    /// further work.
    pub fn snapshot(&self) -> Vec<(SocketAddr, PeerSnapshot)> {
        let g = self.inner.read();
        g.iter()
            .map(|(addr, e)| (*addr, PeerSnapshot::from(e.as_ref())))
            .collect()
    }

    /// Total number of currently-active connections.
    pub fn len(&self) -> usize {
        self.inner.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Lock-free snapshot returned by [`PeerRegistry::snapshot`].
#[derive(Debug, Clone)]
pub struct PeerSnapshot {
    pub connected_at: SystemTime,
    pub last_rx_nanos: u64,
    pub channels: u64,
    pub channels_created: u64,
    pub ops_init: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub tls: bool,
}

impl From<&PeerEntry> for PeerSnapshot {
    fn from(e: &PeerEntry) -> Self {
        Self {
            connected_at: e.connected_at,
            last_rx_nanos: e.last_rx_nanos.load(Ordering::Relaxed),
            channels: e.channels.load(Ordering::Relaxed),
            channels_created: e.channels_created.load(Ordering::Relaxed),
            ops_init: e.ops_init.load(Ordering::Relaxed),
            bytes_in: e.bytes_in.load(Ordering::Relaxed),
            bytes_out: e.bytes_out.load(Ordering::Relaxed),
            tls: e.tls,
        }
    }
}

fn now_nanos() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_remove_snapshot_roundtrip() {
        let reg = PeerRegistry::new();
        let addr: SocketAddr = "127.0.0.1:5075".parse().unwrap();
        assert!(reg.is_empty());
        let entry = PeerEntry::new(false);
        entry.channel_added();
        entry.touch_rx(64);
        reg.insert(addr, entry.clone());
        let snap = reg.snapshot();
        assert_eq!(snap.len(), 1);
        let (a, s) = &snap[0];
        assert_eq!(*a, addr);
        assert_eq!(s.channels, 1);
        assert_eq!(s.bytes_in, 64);
        reg.remove(addr);
        assert!(reg.is_empty());
    }
}
