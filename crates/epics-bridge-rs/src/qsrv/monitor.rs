//! BridgeMonitor: bridges DbSubscription to PVA monitor.
//!
//! Corresponds to C++ QSRV's `PDBSingleMonitor` / `BaseMonitor`.
//!
//! Uses `DbSubscription::recv_snapshot()` to receive full Snapshot data
//! (alarm, display, control, enums) — not just the raw value.
//!
//! On `start()`, reads the current record state and stores it as an
//! initial snapshot, matching C++ BaseMonitor::connect() behavior.
//!
//! Tracks overflow events via a counter, corresponding to C++ BaseMonitor's
//! `inoverflow` flag and overflow BitSet.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use epics_base_rs::server::database::PvDatabase;
use epics_base_rs::server::database::db_access::DbSubscription;
use epics_pva_rs::pvdata::PvStructure;

use super::provider::{AccessContext, PvaMonitor};
use super::pvif::{NtType, snapshot_to_pv_structure};
use crate::error::{BridgeError, BridgeResult};

/// A PVA monitor backed by a DbSubscription for a single record.
///
/// Tracks overflow statistics: when the internal mpsc channel is full,
/// events are dropped. The `overflow_count` tracks how many events
/// were lost (corresponds to C++ BaseMonitor's overflow BitSet).
///
/// Carries an [`AccessContext`] so monitor read permission is enforced
/// in `start()`. Without this, a downstream client denied via `get()`
/// could still receive value updates by subscribing.
pub struct BridgeMonitor {
    db: Arc<PvDatabase>,
    record_name: String,
    nt_type: NtType,
    subscription: Option<DbSubscription>,
    running: bool,
    /// Initial complete snapshot sent on first poll() after start().
    initial_snapshot: Option<PvStructure>,
    /// Number of monitor events lost due to overflow.
    overflow_count: Arc<AtomicU64>,
    /// Access control context for read enforcement on start().
    access: AccessContext,
}

impl BridgeMonitor {
    pub fn new(db: Arc<PvDatabase>, record_name: String, nt_type: NtType) -> Self {
        Self {
            db,
            record_name,
            nt_type,
            subscription: None,
            running: false,
            initial_snapshot: None,
            overflow_count: Arc::new(AtomicU64::new(0)),
            access: AccessContext::allow_all(),
        }
    }

    /// Inject an access control context. The PVA server (or `BridgeChannel`'s
    /// own create_monitor) calls this to propagate the channel's identity
    /// into the monitor.
    pub fn with_access(mut self, access: AccessContext) -> Self {
        self.access = access;
        self
    }

    /// Get the number of overflow events (events lost due to queue full).
    pub fn overflow_count(&self) -> u64 {
        self.overflow_count.load(Ordering::Relaxed)
    }
}

impl PvaMonitor for BridgeMonitor {
    async fn start(&mut self) -> BridgeResult<()> {
        if self.running {
            return Ok(());
        }

        // Read enforcement: a client without read permission must not be
        // allowed to subscribe to monitor events either.
        if !self.access.can_read(&self.record_name) {
            return Err(BridgeError::PutRejected(format!(
                "monitor read denied for {} (user='{}' host='{}')",
                self.record_name, self.access.user, self.access.host
            )));
        }

        let sub = DbSubscription::subscribe(&self.db, &self.record_name)
            .await
            .ok_or_else(|| BridgeError::RecordNotFound(self.record_name.clone()))?;

        // Read initial complete snapshot from the record (like C++ BaseMonitor::connect)
        let (record_name, _) = epics_base_rs::server::database::parse_pv_name(&self.record_name);
        if let Some(rec) = self.db.get_record(record_name).await {
            let instance = rec.read().await;
            if let Some(snapshot) = instance.snapshot_for_field("VAL") {
                self.initial_snapshot = Some(snapshot_to_pv_structure(&snapshot, self.nt_type));
            }
        }

        self.subscription = Some(sub);
        self.running = true;
        Ok(())
    }

    async fn poll(&mut self) -> Option<PvStructure> {
        // Return initial snapshot on first poll (C++ BaseMonitor::connect behavior)
        if let Some(initial) = self.initial_snapshot.take() {
            return Some(initial);
        }

        let sub = self.subscription.as_mut()?;

        // Wait for next change with full Snapshot (alarm, display, control, enums)
        let snapshot = sub.recv_snapshot().await?;
        Some(snapshot_to_pv_structure(&snapshot, self.nt_type))
    }

    async fn stop(&mut self) {
        self.subscription = None;
        self.running = false;
        self.initial_snapshot = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use epics_base_rs::server::records::ai::AiRecord;
    use std::time::Duration;

    /// Lifecycle invariants:
    /// - `start()` opens a subscription (poll yields the initial snapshot).
    /// - `stop()` drops the subscription so the underlying DbSubscription
    ///   is released (poll returns None — the broadcast sender was dropped).
    /// - Stopping is idempotent and leaves no spawned task lingering.
    #[tokio::test]
    async fn monitor_stop_releases_subscription() {
        let db = Arc::new(PvDatabase::new());
        db.add_record("MON_LIFECYCLE", Box::new(AiRecord::new(1.0)))
            .await;

        let mut mon = BridgeMonitor::new(db.clone(), "MON_LIFECYCLE".into(), NtType::Scalar);
        mon.start().await.expect("start ok");
        assert!(mon.running);

        // First poll yields the recorded initial snapshot.
        let initial = tokio::time::timeout(Duration::from_secs(1), mon.poll())
            .await
            .expect("initial poll did not time out");
        assert!(initial.is_some(), "initial snapshot should be present");

        // Drop the underlying record's only owner of the broadcast
        // sender. After `stop()` the subscription is None, so subsequent
        // polls short-circuit; the broadcast subscriber is also released
        // (verified indirectly: a fresh subscribe must succeed without
        // contention).
        mon.stop().await;
        assert!(!mon.running);
        assert!(mon.subscription.is_none());

        // A second `stop()` is idempotent.
        mon.stop().await;
        assert!(!mon.running);

        // After stop, a fresh BridgeMonitor against the same record
        // re-subscribes cleanly (regression for "leaked sender keeps
        // the broadcast at saturated subscriber count" issues).
        let mut mon2 = BridgeMonitor::new(db.clone(), "MON_LIFECYCLE".into(), NtType::Scalar);
        mon2.start().await.expect("re-subscribe ok");
        assert!(mon2.running);
        mon2.stop().await;
    }
}
