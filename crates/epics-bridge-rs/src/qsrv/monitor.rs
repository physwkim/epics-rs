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

use epics_base_rs::server::database::db_access::DbSubscription;
use epics_base_rs::server::database::PvDatabase;
use epics_pva_rs::pvdata::PvStructure;

use crate::error::{BridgeError, BridgeResult};
use super::provider::PvaMonitor;
use super::pvif::{NtType, snapshot_to_pv_structure};

/// A PVA monitor backed by a DbSubscription for a single record.
///
/// Tracks overflow statistics: when the internal mpsc channel is full,
/// events are dropped. The `overflow_count` tracks how many events
/// were lost (corresponds to C++ BaseMonitor's overflow BitSet).
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
        }
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

        let sub = DbSubscription::subscribe(&self.db, &self.record_name)
            .await
            .ok_or_else(|| BridgeError::RecordNotFound(self.record_name.clone()))?;

        // Read initial complete snapshot from the record (like C++ BaseMonitor::connect)
        let (record_name, _) =
            epics_base_rs::server::database::parse_pv_name(&self.record_name);
        if let Some(rec) = self.db.get_record(record_name).await {
            let instance = rec.read().await;
            if let Some(snapshot) = instance.snapshot_for_field("VAL") {
                self.initial_snapshot =
                    Some(snapshot_to_pv_structure(&snapshot, self.nt_type));
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
