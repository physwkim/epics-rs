use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::Notify;

use epics_base_rs::runtime::sync::{Mutex, mpsc};

use crate::protocol::*;
use epics_base_rs::server::pv::{MonitorEvent, ProcessVariable};
use epics_base_rs::types::encode_dbr;

#[derive(Default)]
pub struct FlowControlGate {
    paused: AtomicBool,
    resumed: Notify,
}

impl FlowControlGate {
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Release);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Release);
        self.resumed.notify_waiters();
    }

    pub async fn wait_until_resumed(&self) {
        while self.paused.load(Ordering::Acquire) {
            self.resumed.notified().await;
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    pub async fn coalesce_while_paused(
        &self,
        rx: &mut mpsc::Receiver<MonitorEvent>,
        mut pending: MonitorEvent,
    ) -> Option<MonitorEvent> {
        while self.is_paused() {
            while let Ok(event) = rx.try_recv() {
                pending = event;
            }
            if !self.is_paused() {
                break;
            }
            tokio::select! {
                maybe_event = rx.recv() => match maybe_event {
                    Some(event) => pending = event,
                    None => return None,
                },
                _ = self.resumed.notified() => {}
            }
        }
        Some(pending)
    }
}

/// Spawn a task that forwards monitor events from a PV subscription to the client TCP stream.
/// Returns a handle that can be used to cancel the subscription.
pub fn spawn_monitor_sender(
    _pv: Arc<ProcessVariable>,
    sub_id: u32,
    data_type: u16,
    writer: Arc<Mutex<BufWriter<OwnedWriteHalf>>>,
    flow_control: Arc<FlowControlGate>,
    mut rx: mpsc::Receiver<MonitorEvent>,
) -> tokio::task::JoinHandle<()> {
    epics_base_rs::runtime::task::spawn(async move {
        while let Some(mut event) = rx.recv().await {
            if flow_control.is_paused() {
                let Some(coalesced) = flow_control.coalesce_while_paused(&mut rx, event).await
                else {
                    break;
                };
                event = coalesced;
            }
            let payload = match encode_dbr(data_type, &event.snapshot) {
                Ok(bytes) => bytes,
                Err(_) => break,
            };
            let element_count = event.snapshot.value.count() as u32;
            let mut padded = payload;
            padded.resize(align8(padded.len()), 0);

            let mut hdr = CaHeader::new(CA_PROTO_EVENT_ADD);
            // C client TCP parser requires 8-byte aligned postsize
            hdr.set_payload_size(padded.len(), element_count);
            hdr.data_type = data_type;
            hdr.cid = 1; // ECA_NORMAL status
            hdr.available = sub_id;

            let hdr_bytes = hdr.to_bytes_extended();
            let mut w = writer.lock().await;
            if w.write_all(&hdr_bytes).await.is_err() {
                break;
            }
            if w.write_all(&padded).await.is_err() {
                break;
            }
            let _ = w.flush().await;
        }
    })
}
