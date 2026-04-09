//! Downstream CA server adapter for the gateway.
//!
//! Hosts a [`CaServer`] backed by a shadow [`PvDatabase`]. The shadow
//! database is populated by the [`UpstreamManager`] as upstream
//! subscriptions establish. Downstream clients see PVs as if they
//! were normal in-process PVs — the gateway is transparent on the wire.
//!
//! ## Pre-registration vs lazy resolution
//!
//! The current implementation pre-registers all PVs from the `.pvlist`
//! at gateway startup, eagerly subscribing to each upstream. This differs
//! from C++ ca-gateway, which uses lazy on-demand resolution: a downstream
//! search triggers an upstream search, and the PV is added to the cache
//! only after the upstream IOC responds.
//!
//! Lazy resolution requires a search-hook in `epics-base-rs::PvDatabase`
//! that calls back into the gateway when an unknown name is searched.
//! That hook doesn't exist yet — adding it is a future enhancement.
//! Pre-registration works for any pvlist where the patterns are
//! enumerable (literal names) or where you're willing to subscribe
//! to a known set of upstream PVs at startup.

use std::sync::Arc;

use epics_base_rs::server::database::PvDatabase;
use epics_ca_rs::server::{CaServer, ServerConnectionEvent};
use tokio::sync::Mutex;
use tokio::sync::broadcast;

use crate::error::BridgeResult;

/// Downstream CA server adapter.
///
/// Wraps a [`CaServer`] that serves the gateway's shadow [`PvDatabase`].
/// All actual CA protocol handling (search, connect, get, put, monitor)
/// is delegated to `epics-ca-rs`.
///
/// The CaServer is held inside a `Mutex` because [`CaServer::connection_events`]
/// requires `&mut self` to install the broadcast sender on first call.
/// After installation, the server is moved out and run.
pub struct DownstreamServer {
    server: Mutex<Option<CaServer>>,
    /// Cached shadow DB pointer for `database()` accessor.
    shadow_db: Arc<PvDatabase>,
}

impl DownstreamServer {
    /// Create a new downstream server bound to `port`, serving from
    /// the given shadow database.
    pub fn new(shadow_db: Arc<PvDatabase>, port: u16) -> Self {
        let server = CaServer::from_parts(shadow_db.clone(), port, None, None, None);
        Self {
            server: Mutex::new(Some(server)),
            shadow_db,
        }
    }

    /// Get the underlying shadow database.
    pub fn database(&self) -> &Arc<PvDatabase> {
        &self.shadow_db
    }

    /// Subscribe to connection lifecycle events. Must be called BEFORE
    /// [`run`] (which moves the server out of the Mutex).
    pub async fn connection_events(&self) -> Option<broadcast::Receiver<ServerConnectionEvent>> {
        let mut guard = self.server.lock().await;
        guard.as_mut().map(|s| s.connection_events())
    }

    /// Run the CA server (blocks until shutdown).
    ///
    /// Spawn this in a tokio task — it accepts incoming TCP connections
    /// from downstream clients, handles UDP search broadcasts, and emits
    /// beacons. After this is called, [`connection_events`] returns None.
    pub async fn run(&self) -> BridgeResult<()> {
        let server = {
            let mut guard = self.server.lock().await;
            match guard.take() {
                Some(s) => s,
                None => {
                    return Err(crate::error::BridgeError::PutRejected(
                        "DownstreamServer already running or consumed".into(),
                    ));
                }
            }
        };
        server
            .run()
            .await
            .map_err(|e| crate::error::BridgeError::PutRejected(format!("CaServer run: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn construct_downstream() {
        let db = Arc::new(PvDatabase::new());
        let downstream = DownstreamServer::new(db.clone(), 0);
        // Just verify it constructs without panicking
        assert!(Arc::ptr_eq(downstream.database(), &db));
    }

    #[tokio::test]
    async fn connection_events_subscribe() {
        let db = Arc::new(PvDatabase::new());
        let downstream = DownstreamServer::new(db, 0);
        let rx = downstream.connection_events().await;
        assert!(rx.is_some(), "expected receiver");
    }
}
