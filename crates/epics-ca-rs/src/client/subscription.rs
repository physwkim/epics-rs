use std::collections::HashMap;
use std::time::SystemTime;

use epics_base_rs::runtime::sync::mpsc;

use epics_base_rs::error::CaResult;
use epics_base_rs::server::snapshot::Snapshot;
use epics_base_rs::types::{DbFieldType, EpicsValue, decode_dbr};

use super::types::TransportCommand;

pub(crate) struct SubscriptionRecord {
    pub subid: u32,
    pub cid: u32,
    pub data_type: u16,
    pub count: u32,
    pub mask: u16,
    pub callback_tx: mpsc::UnboundedSender<CaResult<Snapshot>>,
    pub needs_restore: bool,
    /// Client-side deadband: suppress callback if |new - old| < deadband.
    pub deadband: f64,
    /// Last delivered scalar value (for deadband filtering).
    pub last_value: Option<f64>,
}

pub(crate) struct SubscriptionRegistry {
    subscriptions: HashMap<u32, SubscriptionRecord>,
}

impl SubscriptionRegistry {
    pub fn new() -> Self {
        Self {
            subscriptions: HashMap::new(),
        }
    }

    pub fn add(&mut self, rec: SubscriptionRecord) {
        self.subscriptions.insert(rec.subid, rec);
    }

    pub fn remove(&mut self, subid: u32) -> Option<SubscriptionRecord> {
        self.subscriptions.remove(&subid)
    }

    pub fn on_monitor_data(&mut self, subid: u32, data_type: u16, count: u32, data: &[u8]) {
        if let Some(rec) = self.subscriptions.get_mut(&subid) {
            let snapshot = if data_type <= 6 {
                let dbr_type = match DbFieldType::from_u16(data_type) {
                    Ok(t) => t,
                    Err(e) => {
                        let _ = rec.callback_tx.send(Err(e));
                        return;
                    }
                };
                match EpicsValue::from_bytes_array(dbr_type, data, count as usize) {
                    Ok(value) => Snapshot::new(value, 0, 0, SystemTime::now()),
                    Err(e) => { let _ = rec.callback_tx.send(Err(e)); return; }
                }
            } else {
                match decode_dbr(data_type, data, count as usize) {
                    Ok(s) => s,
                    Err(e) => { let _ = rec.callback_tx.send(Err(e)); return; }
                }
            };

            // Client-side deadband filtering (scalar values only)
            if rec.deadband > 0.0 {
                if let Some(new_val) = snapshot.value.to_f64() {
                    if let Some(old_val) = rec.last_value {
                        if (new_val - old_val).abs() < rec.deadband {
                            return; // Suppress — within deadband
                        }
                    }
                    rec.last_value = Some(new_val);
                }
            }

            let _ = rec.callback_tx.send(Ok(snapshot));
        }
    }

    /// Mark all subscriptions for a given server's channels as needing restore.
    /// Returns the cids that were affected.
    pub fn mark_disconnected(&mut self, cids: &[u32]) {
        for rec in self.subscriptions.values_mut() {
            if cids.contains(&rec.cid) {
                rec.needs_restore = true;
            }
        }
    }

    /// Generate restore commands for subscriptions tied to the given cid,
    /// using the new sid.
    /// Restore subscriptions after reconnect. Returns (restored, failed) counts.
    pub fn restore_for_channel(
        &mut self,
        cid: u32,
        new_sid: u32,
        server_addr: std::net::SocketAddr,
        transport_tx: &mpsc::UnboundedSender<TransportCommand>,
    ) -> (u32, u32) {
        let mut restored = 0u32;
        let mut failed = 0u32;
        // Collect stale subids first (callback receiver dropped)
        let stale: Vec<u32> = self.subscriptions.values()
            .filter(|rec| rec.cid == cid && rec.needs_restore && rec.callback_tx.is_closed())
            .map(|rec| rec.subid)
            .collect();
        for subid in &stale {
            self.subscriptions.remove(subid);
            failed += 1;
        }
        for rec in self.subscriptions.values_mut() {
            if rec.cid == cid && rec.needs_restore {
                rec.needs_restore = false;
                let _ = transport_tx.send(TransportCommand::Subscribe {
                    sid: new_sid,
                    data_type: rec.data_type,
                    count: rec.count,
                    subid: rec.subid,
                    mask: rec.mask,
                    server_addr,
                });
                restored += 1;
            }
        }
        (restored, failed)
    }

    /// Number of active subscriptions.
    #[allow(dead_code)]
    pub fn count(&self) -> usize {
        self.subscriptions.len()
    }

    /// Remove subscriptions whose callback receiver has been dropped.
    /// Returns the subids that were removed.
    ///
    /// Not currently called — channel drop sends ClearChannel to the IOC
    /// which cleans up server-side subscriptions automatically.
    #[allow(dead_code)]
    pub fn cleanup_closed(&mut self) -> Vec<u32> {
        let closed: Vec<u32> = self
            .subscriptions
            .iter()
            .filter(|(_, rec)| rec.callback_tx.is_closed())
            .map(|(&subid, _)| subid)
            .collect();
        for subid in &closed {
            self.subscriptions.remove(subid);
        }
        closed
    }

    /// Get subscription info for generating CANCEL commands
    pub fn get(&self, subid: u32) -> Option<&SubscriptionRecord> {
        self.subscriptions.get(&subid)
    }

    /// Get all subscriptions for a given cid
    pub fn for_cid(&self, cid: u32) -> Vec<u32> {
        self.subscriptions
            .iter()
            .filter(|(_, rec)| rec.cid == cid)
            .map(|(&subid, _)| subid)
            .collect()
    }
}
