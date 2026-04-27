mod beacon_monitor;
mod search;
mod state;
mod subscription;
mod transport;
mod types;

use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use std::time::Duration;

use epics_base_rs::runtime::sync::{broadcast, mpsc, oneshot};

use crate::channel::{AccessRights, ChannelInfo, alloc_cid, alloc_ioid, alloc_subid};
use crate::protocol::*;
use crate::repeater;
use epics_base_rs::error::{CaError, CaResult};
use epics_base_rs::server::snapshot::{DbrClass, Snapshot};
use epics_base_rs::types::{DbFieldType, EpicsValue, decode_dbr};

pub use state::{ChannelState, ConnectionEvent};

use state::ChannelInner;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Type of diagnostic event recorded in the event history.
#[derive(Debug, Clone)]
pub enum DiagEvent {
    Connected {
        pv: String,
        server: SocketAddr,
    },
    Disconnected {
        server: SocketAddr,
        channels: usize,
    },
    Reconnected {
        pv: String,
        restored: u32,
        stale: u32,
    },
    Unresponsive {
        server: SocketAddr,
    },
    Responsive {
        server: SocketAddr,
    },
    BeaconAnomaly {
        server: SocketAddr,
    },
}

impl std::fmt::Display for DiagEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connected { pv, server } => write!(f, "Connected {pv} @ {server}"),
            Self::Disconnected { server, channels } => {
                write!(f, "Disconnected {server} ({channels} channels)")
            }
            Self::Reconnected {
                pv,
                restored,
                stale,
            } => write!(f, "Reconnected {pv} (restored={restored}, stale={stale})"),
            Self::Unresponsive { server } => write!(f, "Unresponsive {server}"),
            Self::Responsive { server } => write!(f, "Responsive {server}"),
            Self::BeaconAnomaly { server } => write!(f, "Beacon anomaly {server}"),
        }
    }
}

/// Timestamped diagnostic event.
#[derive(Debug, Clone)]
pub struct DiagRecord {
    pub time: std::time::Instant,
    pub event: DiagEvent,
}

const EVENT_HISTORY_CAPACITY: usize = 256;

/// Diagnostic counters for CA client health monitoring.
pub struct CaDiagnostics {
    pub connections: AtomicU64,
    pub disconnections: AtomicU64,
    pub reconnections: AtomicU64,
    pub unresponsive_events: AtomicU64,
    pub subscriptions_restored: AtomicU64,
    pub subscriptions_stale: AtomicU64,
    pub beacon_anomalies: AtomicU64,
    pub search_requests: AtomicU64,
    /// Monitor updates dropped because the application's queue was full.
    /// Slow consumers should bump up EPICS_CA_MONITOR_QUEUE or call
    /// recv() more often.
    pub dropped_monitors: AtomicU64,
    /// Ring buffer of recent events for post-mortem analysis.
    history: std::sync::Mutex<Vec<DiagRecord>>,
}

impl Default for CaDiagnostics {
    fn default() -> Self {
        Self {
            connections: AtomicU64::new(0),
            disconnections: AtomicU64::new(0),
            reconnections: AtomicU64::new(0),
            unresponsive_events: AtomicU64::new(0),
            subscriptions_restored: AtomicU64::new(0),
            subscriptions_stale: AtomicU64::new(0),
            beacon_anomalies: AtomicU64::new(0),
            search_requests: AtomicU64::new(0),
            dropped_monitors: AtomicU64::new(0),
            history: std::sync::Mutex::new(Vec::with_capacity(EVENT_HISTORY_CAPACITY)),
        }
    }
}

impl CaDiagnostics {
    /// Record a diagnostic event with the current timestamp.
    pub fn record(&self, event: DiagEvent) {
        let record = DiagRecord {
            time: std::time::Instant::now(),
            event,
        };
        if let Ok(mut history) = self.history.lock() {
            if history.len() >= EVENT_HISTORY_CAPACITY {
                history.remove(0);
            }
            history.push(record);
        }
    }

    /// Get a snapshot of counters + recent event history.
    pub fn snapshot(&self) -> DiagnosticsSnapshot {
        let history = self.history.lock().map(|h| h.clone()).unwrap_or_default();
        DiagnosticsSnapshot {
            connections: self.connections.load(Ordering::Relaxed),
            disconnections: self.disconnections.load(Ordering::Relaxed),
            reconnections: self.reconnections.load(Ordering::Relaxed),
            unresponsive_events: self.unresponsive_events.load(Ordering::Relaxed),
            subscriptions_restored: self.subscriptions_restored.load(Ordering::Relaxed),
            subscriptions_stale: self.subscriptions_stale.load(Ordering::Relaxed),
            beacon_anomalies: self.beacon_anomalies.load(Ordering::Relaxed),
            search_requests: self.search_requests.load(Ordering::Relaxed),
            dropped_monitors: self.dropped_monitors.load(Ordering::Relaxed),
            history,
        }
    }
}

/// Point-in-time snapshot of diagnostic counters + event history.
#[derive(Debug, Clone)]
pub struct DiagnosticsSnapshot {
    pub connections: u64,
    pub disconnections: u64,
    pub reconnections: u64,
    pub unresponsive_events: u64,
    pub subscriptions_restored: u64,
    pub subscriptions_stale: u64,
    pub beacon_anomalies: u64,
    pub search_requests: u64,
    pub dropped_monitors: u64,
    pub history: Vec<DiagRecord>,
}

impl std::fmt::Display for DiagnosticsSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Connections:            {}", self.connections)?;
        writeln!(f, "Disconnections:         {}", self.disconnections)?;
        writeln!(f, "Reconnections:          {}", self.reconnections)?;
        writeln!(f, "Unresponsive events:    {}", self.unresponsive_events)?;
        writeln!(f, "Subscriptions restored: {}", self.subscriptions_restored)?;
        writeln!(f, "Subscriptions stale:    {}", self.subscriptions_stale)?;
        writeln!(f, "Beacon anomalies:       {}", self.beacon_anomalies)?;
        writeln!(f, "Search requests:        {}", self.search_requests)?;
        writeln!(f, "Dropped monitors:       {}", self.dropped_monitors)?;
        if !self.history.is_empty() {
            writeln!(f, "Recent events ({}):", self.history.len())?;
            let start = self
                .history
                .first()
                .map(|r| r.time)
                .unwrap_or_else(std::time::Instant::now);
            for rec in &self.history {
                let elapsed = rec.time.duration_since(start);
                writeln!(f, "  +{:.1}s  {}", elapsed.as_secs_f64(), rec.event)?;
            }
        }
        Ok(())
    }
}
use subscription::SubscriptionRegistry;
use types::*;

/// CA client with persistent channels and auto-reconnection.
pub struct CaClient {
    search_tx: mpsc::UnboundedSender<SearchRequest>,
    transport_tx: mpsc::UnboundedSender<TransportCommand>,
    coord_tx: mpsc::UnboundedSender<CoordRequest>,
    diagnostics: Arc<CaDiagnostics>,
    _coordinator: tokio::task::JoinHandle<()>,
    _search_task: tokio::task::JoinHandle<()>,
    _transport_task: tokio::task::JoinHandle<()>,
    _beacon_task: tokio::task::JoinHandle<()>,
}

/// Internal coordinator requests from CaChannel / public API
#[allow(dead_code)]
enum CoordRequest {
    RegisterChannel {
        cid: u32,
        pv_name: String,
        conn_tx: broadcast::Sender<ConnectionEvent>,
    },
    WaitConnected {
        cid: u32,
        reply: oneshot::Sender<()>,
    },
    GetChannelInfo {
        cid: u32,
        reply: oneshot::Sender<Option<ChannelSnapshot>>,
    },
    Subscribe {
        cid: u32,
        subid: u32,
        mask: u16,
        deadband: f64,
        callback_tx: mpsc::Sender<CaResult<Snapshot>>,
        reply: oneshot::Sender<CaResult<()>>,
    },
    Unsubscribe {
        subid: u32,
    },
    MonitorConsumed {
        subid: u32,
    },
    DropChannel {
        cid: u32,
    },
    ReadNotify {
        cid: u32,
        ioid: u32,
        reply: oneshot::Sender<CaResult<(u16, u32, Vec<u8>)>>,
    },
    WriteNotify {
        cid: u32,
        ioid: u32,
        value: EpicsValue,
        reply: oneshot::Sender<CaResult<()>>,
    },
    /// Beacon anomaly detected — rescan channels for this server.
    ForceRescanServer {
        server_addr: SocketAddr,
    },
    /// Graceful shutdown: clear all channels on their servers before exiting.
    Shutdown {
        reply: oneshot::Sender<()>,
    },
}

#[derive(Clone)]
struct ChannelSnapshot {
    sid: u32,
    native_type: DbFieldType,
    element_count: u32,
    server_addr: SocketAddr,
    access_rights: AccessRights,
    state: ChannelState,
    pv_name: String,
}

impl CaClient {
    pub async fn new() -> CaResult<Self> {
        // Run repeater registration in background — don't block client startup.
        epics_base_rs::runtime::task::spawn(async { repeater::ensure_repeater().await });

        let addr_list = parse_addr_list()?;
        let nameserver_addrs = parse_nameserver_list();

        let (search_tx, search_rx) = mpsc::unbounded_channel();
        let (search_resp_tx, search_resp_rx) = mpsc::unbounded_channel();

        let (transport_tx, transport_rx) = mpsc::unbounded_channel();
        let (transport_evt_tx, transport_evt_rx) = mpsc::unbounded_channel();

        let (coord_tx, coord_rx) = mpsc::unbounded_channel();

        let search_task = epics_base_rs::runtime::task::spawn(search::run_search_engine(
            addr_list,
            nameserver_addrs,
            search_rx,
            search_resp_tx,
        ));

        let transport_task = epics_base_rs::runtime::task::spawn(transport::run_transport_manager(
            transport_rx,
            transport_evt_tx,
        ));

        let diagnostics = Arc::new(CaDiagnostics::default());

        let coordinator = epics_base_rs::runtime::task::spawn(run_coordinator(
            coord_rx,
            search_resp_rx,
            transport_evt_rx,
            search_tx.clone(),
            transport_tx.clone(),
            diagnostics.clone(),
        ));

        let beacon_task = epics_base_rs::runtime::task::spawn(beacon_monitor::run_beacon_monitor(
            coord_tx.clone(),
        ));

        Ok(Self {
            search_tx,
            transport_tx,
            coord_tx,
            diagnostics,
            _coordinator: coordinator,
            _search_task: search_task,
            _transport_task: transport_task,
            _beacon_task: beacon_task,
        })
    }

    /// Get a snapshot of diagnostic counters.
    pub fn diagnostics(&self) -> DiagnosticsSnapshot {
        self.diagnostics.snapshot()
    }

    /// Graceful shutdown: send ClearChannel for all connected channels
    /// so servers can release resources immediately.
    pub async fn shutdown(&self) {
        let (tx, rx) = oneshot::channel();
        let _ = self.coord_tx.send(CoordRequest::Shutdown { reply: tx });
        // Wait briefly for the clear commands to be sent
        let _ = tokio::time::timeout(Duration::from_secs(2), rx).await;
    }

    /// Create a persistent channel. Returns immediately (starts searching in background).
    pub fn create_channel(&self, name: &str) -> CaChannel {
        let cid = alloc_cid();
        let (conn_tx, _) = broadcast::channel(16);

        // EPICS_CA_USE_SHELL_VARS=YES expands ${VAR}/$(VAR) tokens in PV
        // names against the process environment, matching libca behaviour.
        let pv_name = if epics_base_rs::runtime::env::get_or("EPICS_CA_USE_SHELL_VARS", "NO")
            .eq_ignore_ascii_case("YES")
        {
            expand_shell_vars(name)
        } else {
            name.to_string()
        };

        let _ = self.coord_tx.send(CoordRequest::RegisterChannel {
            cid,
            pv_name: pv_name.clone(),
            conn_tx: conn_tx.clone(),
        });

        let _ = self.search_tx.send(SearchRequest::Schedule {
            cid,
            pv_name: pv_name.clone(),
            reason: SearchReason::Initial,
            initial_lane: 0,
        });

        CaChannel {
            cid,
            pv_name,
            coord_tx: self.coord_tx.clone(),
            transport_tx: self.transport_tx.clone(),
            conn_tx,
        }
    }

    // --- Legacy one-shot API (backwards-compatible) ---

    pub async fn caget(&self, pv_name: &str) -> CaResult<(DbFieldType, EpicsValue)> {
        let ch = self.create_channel(pv_name);
        ch.wait_connected(Duration::from_secs(3)).await?;
        let result = ch.get().await;
        let _ = self
            .coord_tx
            .send(CoordRequest::DropChannel { cid: ch.cid });
        result
    }

    /// Fire-and-forget write (CA_PROTO_WRITE). Matches C `caput` behavior.
    pub async fn caput(&self, pv_name: &str, value_str: &str) -> CaResult<()> {
        let ch = self.create_channel(pv_name);
        ch.wait_connected(Duration::from_secs(3)).await?;

        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.coord_tx.send(CoordRequest::GetChannelInfo {
            cid: ch.cid,
            reply: reply_tx,
        });
        let snap = reply_rx
            .await
            .map_err(|_| CaError::Shutdown)?
            .ok_or(CaError::Disconnected)?;

        let value = EpicsValue::parse(snap.native_type, value_str)?;
        ch.put_nowait(&value).await?;
        let _ = self
            .coord_tx
            .send(CoordRequest::DropChannel { cid: ch.cid });
        Ok(())
    }

    /// Write with completion callback (CA_PROTO_WRITE_NOTIFY). Matches C `caput -c`.
    pub async fn caput_callback(
        &self,
        pv_name: &str,
        value_str: &str,
        timeout_secs: f64,
    ) -> CaResult<()> {
        let ch = self.create_channel(pv_name);
        let timeout = Duration::from_secs_f64(timeout_secs);
        ch.wait_connected(timeout).await?;

        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.coord_tx.send(CoordRequest::GetChannelInfo {
            cid: ch.cid,
            reply: reply_tx,
        });
        let snap = reply_rx
            .await
            .map_err(|_| CaError::Shutdown)?
            .ok_or(CaError::Disconnected)?;

        let value = EpicsValue::parse(snap.native_type, value_str)?;
        ch.put_with_timeout(&value, timeout).await?;
        let _ = self
            .coord_tx
            .send(CoordRequest::DropChannel { cid: ch.cid });
        Ok(())
    }

    pub async fn cainfo(&self, pv_name: &str) -> CaResult<ChannelInfo> {
        let ch = self.create_channel(pv_name);
        ch.wait_connected(Duration::from_secs(3)).await?;

        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.coord_tx.send(CoordRequest::GetChannelInfo {
            cid: ch.cid,
            reply: reply_tx,
        });
        let snap = reply_rx
            .await
            .map_err(|_| CaError::Shutdown)?
            .ok_or(CaError::Disconnected)?;

        let _ = self
            .coord_tx
            .send(CoordRequest::DropChannel { cid: ch.cid });

        Ok(ChannelInfo {
            pv_name: snap.pv_name,
            server_addr: snap.server_addr,
            native_type: snap.native_type,
            element_count: snap.element_count,
            access_rights: snap.access_rights,
        })
    }

    /// Monitor a PV with callback (legacy API).
    pub async fn camonitor<F>(&self, pv_name: &str, mut callback: F) -> CaResult<()>
    where
        F: FnMut(EpicsValue),
    {
        let ch = self.create_channel(pv_name);
        let mut monitor = ch.subscribe().await?;

        while let Some(result) = monitor.recv().await {
            match result {
                Ok(snap) => callback(snap.value),
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }
}

/// A persistent CA channel with auto-reconnection.
#[derive(Clone)]
pub struct CaChannel {
    cid: u32,
    pv_name: String,
    coord_tx: mpsc::UnboundedSender<CoordRequest>,
    transport_tx: mpsc::UnboundedSender<TransportCommand>,
    conn_tx: broadcast::Sender<ConnectionEvent>,
}

impl CaChannel {
    pub async fn wait_connected(&self, timeout: Duration) -> CaResult<()> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.coord_tx.send(CoordRequest::WaitConnected {
            cid: self.cid,
            reply: reply_tx,
        });
        tokio::time::timeout(timeout, reply_rx)
            .await
            .map_err(|_| CaError::ChannelNotFound(self.pv_name.clone()))?
            .map_err(|_| CaError::Shutdown)
    }

    /// Get channel-level metadata (native type, element count, host, access rights)
    /// without performing a CA read.
    pub async fn info(&self) -> CaResult<ChannelInfo> {
        let (info_tx, info_rx) = oneshot::channel();
        let _ = self.coord_tx.send(CoordRequest::GetChannelInfo {
            cid: self.cid,
            reply: info_tx,
        });
        let snap = info_rx
            .await
            .map_err(|_| CaError::Shutdown)?
            .ok_or(CaError::Disconnected)?;
        if !snap.state.is_operational() {
            return Err(CaError::Disconnected);
        }
        Ok(ChannelInfo {
            pv_name: snap.pv_name,
            server_addr: snap.server_addr,
            native_type: snap.native_type,
            element_count: snap.element_count,
            access_rights: snap.access_rights,
        })
    }

    pub async fn get(&self) -> CaResult<(DbFieldType, EpicsValue)> {
        self.get_with_timeout(Duration::from_secs(30)).await
    }

    pub async fn get_with_timeout(&self, timeout: Duration) -> CaResult<(DbFieldType, EpicsValue)> {
        let (info_tx, info_rx) = oneshot::channel();
        let _ = self.coord_tx.send(CoordRequest::GetChannelInfo {
            cid: self.cid,
            reply: info_tx,
        });
        let snap = info_rx
            .await
            .map_err(|_| CaError::Shutdown)?
            .ok_or(CaError::Disconnected)?;

        if !snap.state.is_operational() {
            return Err(CaError::Disconnected);
        }

        let ioid = alloc_ioid();
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.coord_tx.send(CoordRequest::ReadNotify {
            cid: self.cid,
            ioid,
            reply: reply_tx,
        });

        let _ = self.transport_tx.send(TransportCommand::ReadNotify {
            sid: snap.sid,
            data_type: snap.native_type as u16,
            count: snap.element_count,
            ioid,
            server_addr: snap.server_addr,
        });

        let (data_type, count, data) = tokio::time::timeout(timeout, reply_rx)
            .await
            .map_err(|_| CaError::Timeout)?
            .map_err(|_| CaError::Shutdown)??;

        let dbr_type = DbFieldType::from_u16(data_type)?;
        let value = EpicsValue::from_bytes_array(dbr_type, &data, count as usize)?;
        Ok((dbr_type, value))
    }

    /// Get a PV value with metadata. Use `DbrClass::Time` for timestamp + alarm,
    /// or `DbrClass::Ctrl` for full control metadata (units, limits, precision).
    /// Pass `count` to limit the number of array elements (0 = all).
    pub async fn get_with_metadata(&self, class: DbrClass) -> CaResult<Snapshot> {
        self.get_with_metadata_count(class, 0).await
    }

    /// Get a PV value with metadata, requesting at most `count` elements.
    /// Pass 0 for the full element count.
    pub async fn get_with_metadata_count(&self, class: DbrClass, count: u32) -> CaResult<Snapshot> {
        let (info_tx, info_rx) = oneshot::channel();
        let _ = self.coord_tx.send(CoordRequest::GetChannelInfo {
            cid: self.cid,
            reply: info_tx,
        });
        let snap = info_rx
            .await
            .map_err(|_| CaError::Shutdown)?
            .ok_or(CaError::Disconnected)?;

        if !snap.state.is_operational() {
            return Err(CaError::Disconnected);
        }

        let request_count = if count > 0 {
            count.min(snap.element_count)
        } else {
            snap.element_count
        };

        let native = DbFieldType::from_u16(snap.native_type as u16)?;
        let request_type = match class {
            DbrClass::Time => native.time_dbr_type(),
            DbrClass::Ctrl => native.ctrl_dbr_type(),
            DbrClass::Sts => native as u16 + 7,
            DbrClass::Gr => native as u16 + 21,
            DbrClass::Plain => native as u16,
        };

        let ioid = alloc_ioid();
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.coord_tx.send(CoordRequest::ReadNotify {
            cid: self.cid,
            ioid,
            reply: reply_tx,
        });

        let _ = self.transport_tx.send(TransportCommand::ReadNotify {
            sid: snap.sid,
            data_type: request_type,
            count: request_count,
            ioid,
            server_addr: snap.server_addr,
        });

        let (data_type, resp_count, data) = tokio::time::timeout(Duration::from_secs(30), reply_rx)
            .await
            .map_err(|_| CaError::Timeout)?
            .map_err(|_| CaError::Shutdown)??;

        decode_dbr(data_type, &data, resp_count as usize)
    }

    pub async fn put(&self, value: &EpicsValue) -> CaResult<()> {
        let (info_tx, info_rx) = oneshot::channel();
        let _ = self.coord_tx.send(CoordRequest::GetChannelInfo {
            cid: self.cid,
            reply: info_tx,
        });
        let snap = info_rx
            .await
            .map_err(|_| CaError::Shutdown)?
            .ok_or(CaError::Disconnected)?;

        if !snap.state.is_operational() {
            return Err(CaError::Disconnected);
        }

        let ioid = alloc_ioid();
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.coord_tx.send(CoordRequest::WriteNotify {
            cid: self.cid,
            ioid,
            value: value.clone(),
            reply: reply_tx,
        });

        let payload = value.to_bytes();
        let count = value.count() as u32;
        let _ = self.transport_tx.send(TransportCommand::WriteNotify {
            sid: snap.sid,
            data_type: snap.native_type as u16,
            count,
            ioid,
            payload,
            server_addr: snap.server_addr,
        });

        // Default put timeout configurable via EPICS_CA_PUT_TIMEOUT (seconds).
        let default_secs = epics_base_rs::runtime::env::get("EPICS_CA_PUT_TIMEOUT")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(30.0);
        tokio::time::timeout(Duration::from_secs_f64(default_secs), reply_rx)
            .await
            .map_err(|_| CaError::Timeout)?
            .map_err(|_| CaError::Shutdown)?
    }

    /// Write with completion callback and configurable timeout.
    pub async fn put_with_timeout(&self, value: &EpicsValue, timeout: Duration) -> CaResult<()> {
        let (info_tx, info_rx) = oneshot::channel();
        let _ = self.coord_tx.send(CoordRequest::GetChannelInfo {
            cid: self.cid,
            reply: info_tx,
        });
        let snap = info_rx
            .await
            .map_err(|_| CaError::Shutdown)?
            .ok_or(CaError::Disconnected)?;

        if !snap.state.is_operational() {
            return Err(CaError::Disconnected);
        }

        let ioid = alloc_ioid();
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.coord_tx.send(CoordRequest::WriteNotify {
            cid: self.cid,
            ioid,
            value: value.clone(),
            reply: reply_tx,
        });

        let payload = value.to_bytes();
        let count = value.count() as u32;
        let _ = self.transport_tx.send(TransportCommand::WriteNotify {
            sid: snap.sid,
            data_type: snap.native_type as u16,
            count,
            ioid,
            payload,
            server_addr: snap.server_addr,
        });

        tokio::time::timeout(timeout, reply_rx)
            .await
            .map_err(|_| CaError::Timeout)?
            .map_err(|_| CaError::Shutdown)?
    }

    /// Fire-and-forget put (CA_PROTO_WRITE). Returns immediately without
    /// waiting for server acknowledgement. Used by ophyd's EpicsMotor.set()
    /// which monitors DMOV for completion instead.
    pub async fn put_nowait(&self, value: &EpicsValue) -> CaResult<()> {
        let (info_tx, info_rx) = oneshot::channel();
        let _ = self.coord_tx.send(CoordRequest::GetChannelInfo {
            cid: self.cid,
            reply: info_tx,
        });
        let snap = info_rx
            .await
            .map_err(|_| CaError::Shutdown)?
            .ok_or(CaError::Disconnected)?;

        if !snap.state.is_operational() {
            return Err(CaError::Disconnected);
        }

        let payload = value.to_bytes();
        let count = value.count() as u32;
        let _ = self.transport_tx.send(TransportCommand::Write {
            sid: snap.sid,
            data_type: snap.native_type as u16,
            count,
            payload,
            server_addr: snap.server_addr,
        });

        Ok(())
    }

    pub async fn subscribe(&self) -> CaResult<MonitorHandle> {
        self.subscribe_with_deadband(0.0).await
    }

    /// Subscribe with client-side deadband filtering.
    /// Events where |new - old| < deadband are suppressed (scalar values only).
    pub async fn subscribe_with_deadband(&self, deadband: f64) -> CaResult<MonitorHandle> {
        let subid = alloc_subid();
        // Bounded queue prevents unbounded memory growth on slow consumers.
        // EVENTS_OFF will fire when outstanding hits FLOW_CONTROL_OFF_THRESHOLD,
        // but the queue gives the application a buffer before drops kick in.
        let queue_size = epics_base_rs::runtime::env::get("EPICS_CA_MONITOR_QUEUE")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(256)
            .max(8);
        let (callback_tx, callback_rx) = mpsc::channel(queue_size);

        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.coord_tx.send(CoordRequest::Subscribe {
            cid: self.cid,
            subid,
            mask: DBE_VALUE | DBE_LOG | DBE_ALARM,
            deadband,
            callback_tx,
            reply: reply_tx,
        });

        reply_rx.await.map_err(|_| CaError::Shutdown)??;

        Ok(MonitorHandle {
            subid,
            callback_rx,
            coord_tx: self.coord_tx.clone(),
        })
    }

    pub fn connection_events(&self) -> broadcast::Receiver<ConnectionEvent> {
        self.conn_tx.subscribe()
    }
}

impl Drop for CaChannel {
    fn drop(&mut self) {
        let _ = self
            .coord_tx
            .send(CoordRequest::DropChannel { cid: self.cid });
    }
}

/// Handle for a monitor subscription. Dropping it cancels the subscription.
pub struct MonitorHandle {
    subid: u32,
    callback_rx: mpsc::Receiver<CaResult<Snapshot>>,
    coord_tx: mpsc::UnboundedSender<CoordRequest>,
}

impl MonitorHandle {
    pub async fn recv(&mut self) -> Option<CaResult<Snapshot>> {
        let result = self.callback_rx.recv().await;
        if result.is_some() {
            let _ = self
                .coord_tx
                .send(CoordRequest::MonitorConsumed { subid: self.subid });
        }
        result
    }
}

impl Drop for MonitorHandle {
    fn drop(&mut self) {
        let _ = self
            .coord_tx
            .send(CoordRequest::Unsubscribe { subid: self.subid });
    }
}

// --- Coordinator ---

const FLOW_CONTROL_OFF_THRESHOLD: usize = 10;
const FLOW_CONTROL_ON_THRESHOLD: usize = 5;

#[derive(Default)]
struct FlowControlState {
    outstanding: usize,
    active: bool,
}

fn flow_control_note_queued(
    flow_control: &mut HashMap<SocketAddr, FlowControlState>,
    server_addr: SocketAddr,
    transport_tx: &mpsc::UnboundedSender<TransportCommand>,
) {
    let state = flow_control.entry(server_addr).or_default();
    state.outstanding = state.outstanding.saturating_add(1);
    if !state.active && state.outstanding >= FLOW_CONTROL_OFF_THRESHOLD {
        let _ = transport_tx.send(TransportCommand::EventsOff { server_addr });
        state.active = true;
    }
}

fn flow_control_note_consumed(
    flow_control: &mut HashMap<SocketAddr, FlowControlState>,
    server_addr: SocketAddr,
    count: usize,
    transport_tx: &mpsc::UnboundedSender<TransportCommand>,
) {
    if count == 0 {
        return;
    }
    let Some(state) = flow_control.get_mut(&server_addr) else {
        return;
    };
    state.outstanding = state.outstanding.saturating_sub(count);
    if state.active && state.outstanding <= FLOW_CONTROL_ON_THRESHOLD {
        let _ = transport_tx.send(TransportCommand::EventsOn { server_addr });
        state.active = false;
    }
    if !state.active && state.outstanding == 0 {
        flow_control.remove(&server_addr);
    }
}

async fn run_coordinator(
    mut coord_rx: mpsc::UnboundedReceiver<CoordRequest>,
    mut search_rx: mpsc::UnboundedReceiver<SearchResponse>,
    mut transport_rx: mpsc::UnboundedReceiver<TransportEvent>,
    search_tx: mpsc::UnboundedSender<SearchRequest>,
    transport_tx: mpsc::UnboundedSender<TransportCommand>,
    diag: Arc<CaDiagnostics>,
) {
    let mut channels: HashMap<u32, ChannelInner> = HashMap::new();
    let mut pending_wait_connected: HashMap<u32, Vec<oneshot::Sender<()>>> = HashMap::new();
    let mut pending_found: HashMap<u32, SocketAddr> = HashMap::new();
    let mut subscriptions = SubscriptionRegistry::new();
    let mut read_waiters: HashMap<u32, (u32, oneshot::Sender<CaResult<(u16, u32, Vec<u8>)>>)> =
        HashMap::new();
    let mut write_waiters: HashMap<u32, (u32, oneshot::Sender<CaResult<()>>)> = HashMap::new();
    // Reverse index: server_addr -> set of cids last seen on that server.
    // Keep disconnected channels indexed so beacon anomalies can trigger
    // immediate re-search for the affected IOC.
    let mut server_channels: HashMap<SocketAddr, HashSet<u32>> = HashMap::new();
    let mut flow_control: HashMap<SocketAddr, FlowControlState> = HashMap::new();

    loop {
        tokio::select! {
            req = coord_rx.recv() => {
                let Some(req) = req else { return };
                match req {
                    CoordRequest::RegisterChannel { cid, pv_name, conn_tx } => {
                        // Drain any waiters that arrived before registration.
                        let early_waiters = pending_wait_connected
                            .remove(&cid)
                            .unwrap_or_default();
                        channels.insert(cid, ChannelInner {
                            cid,
                            pv_name: pv_name.clone(),
                            state: ChannelState::Searching,
                            sid: 0,
                            native_type: None,
                            element_count: 0,
                            server_addr: None,
                            access_rights: AccessRights::from_u32(0),
                            connect_waiters: early_waiters,
                            conn_tx,
                            reconnect_count: 0,
                            last_connected_at: None,
                        });
                        // Process any Found response that arrived before registration.
                        if let Some(server_addr) = pending_found.remove(&cid) {
                            let ch = channels.get_mut(&cid).unwrap();
                            ch.state = ChannelState::Connecting;
                            ch.server_addr = Some(server_addr);
                            server_channels.entry(server_addr).or_default().insert(cid);
                            let _ = transport_tx.send(TransportCommand::CreateChannel {
                                cid,
                                pv_name,
                                server_addr,
                            });
                        }
                    }
                    CoordRequest::WaitConnected { cid, reply } => {
                        if let Some(ch) = channels.get_mut(&cid) {
                            if ch.state == ChannelState::Connected {
                                let _ = reply.send(());
                            } else {
                                ch.connect_waiters.push(reply);
                            }
                        } else {
                            // Channel not yet registered — stash the waiter
                            // so RegisterChannel can drain it when it arrives.
                            pending_wait_connected
                                .entry(cid)
                                .or_default()
                                .push(reply);
                        }
                    }
                    CoordRequest::GetChannelInfo { cid, reply } => {
                        let snap = channels.get(&cid).and_then(|ch| {
                            Some(ChannelSnapshot {
                                sid: ch.sid,
                                native_type: ch.native_type?,
                                element_count: ch.element_count,
                                server_addr: ch.server_addr?,
                                access_rights: ch.access_rights,
                                state: ch.state,
                                pv_name: ch.pv_name.clone(),
                            })
                        });
                        let _ = reply.send(snap);
                    }
                    CoordRequest::Subscribe { cid, subid, mask, deadband, callback_tx, reply } => {
                        if let Some(ch) = channels.get(&cid) {
                            let server_addr = ch.server_addr.unwrap_or_else(|| {
                                SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
                            });
                            let connected = ch.state == ChannelState::Connected;
                            let data_type = ch.native_type.map(|t| t as u16 + 14);
                            let count = ch.native_type.map(|_| ch.element_count);

                            subscriptions.add(subscription::SubscriptionRecord {
                                subid,
                                cid,
                                data_type,
                                count,
                                mask,
                                server_addr,
                                deadband,
                                callback_tx,
                                needs_restore: !connected,
                                last_value: None,
                                pending_deliveries: 0,
                            });

                            if connected {
                                let _ = transport_tx.send(TransportCommand::Subscribe {
                                    sid: ch.sid,
                                    data_type: data_type.expect("connected channel has native type"),
                                    count: count.expect("connected channel has element count"),
                                    subid,
                                    mask,
                                    server_addr,
                                });
                            }
                            let _ = reply.send(Ok(()));
                        } else {
                            let _ = reply.send(Err(CaError::Disconnected));
                        }
                    }
                    CoordRequest::Unsubscribe { subid } => {
                        if let Some(rec) = subscriptions.get(subid) {
                            let cid = rec.cid;
                            if let Some(ch) = channels.get(&cid) {
                                if ch.state == ChannelState::Connected {
                                    if let Some(data_type) = rec.data_type {
                                        let _ = transport_tx.send(TransportCommand::Unsubscribe {
                                            sid: ch.sid,
                                            subid,
                                            data_type,
                                            server_addr: ch.server_addr.unwrap(),
                                        });
                                    }
                                }
                            }
                        }
                        if let Some(rec) = subscriptions.remove(subid) {
                            flow_control_note_consumed(
                                &mut flow_control,
                                rec.server_addr,
                                rec.pending_deliveries,
                                &transport_tx,
                            );
                        }
                    }
                    CoordRequest::MonitorConsumed { subid } => {
                        if let Some(server_addr) = subscriptions.mark_consumed(subid) {
                            flow_control_note_consumed(
                                &mut flow_control,
                                server_addr,
                                1,
                                &transport_tx,
                            );
                        }
                    }
                    CoordRequest::DropChannel { cid } => {
                        // Cancel all subscriptions for this channel
                        let sub_ids = subscriptions.for_cid(cid);
                        for subid in sub_ids {
                            if let Some(rec) = subscriptions.get(subid) {
                                if let Some(ch) = channels.get(&cid) {
                                    if ch.state == ChannelState::Connected {
                                        if let Some(data_type) = rec.data_type {
                                            let _ = transport_tx.send(TransportCommand::Unsubscribe {
                                                sid: ch.sid,
                                                subid,
                                                data_type,
                                                server_addr: ch.server_addr.unwrap(),
                                            });
                                        }
                                    }
                                }
                            }
                            if let Some(rec) = subscriptions.remove(subid) {
                                flow_control_note_consumed(
                                    &mut flow_control,
                                    rec.server_addr,
                                    rec.pending_deliveries,
                                    &transport_tx,
                                );
                            }
                        }

                        // Clear channel on server + clean reverse index
                        if let Some(ch) = channels.get(&cid) {
                            if ch.state.is_operational() {
                                let _ = transport_tx.send(TransportCommand::ClearChannel {
                                    cid,
                                    sid: ch.sid,
                                    server_addr: ch.server_addr.unwrap(),
                                });
                            }
                            // Cancel search for any non-connected state
                            match ch.state {
                                ChannelState::Searching
                                | ChannelState::Connecting
                                | ChannelState::Disconnected => {
                                    let _ = search_tx.send(SearchRequest::Cancel { cid });
                                }
                                _ => {}
                            }
                            if let Some(addr) = ch.server_addr {
                                remove_server_channel(&mut server_channels, addr, cid);
                            }
                        }
                        channels.remove(&cid);
                    }
                    CoordRequest::ReadNotify { cid, ioid, reply } => {
                        read_waiters.insert(ioid, (cid, reply));
                    }
                    CoordRequest::WriteNotify { cid, ioid, value: _, reply } => {
                        write_waiters.insert(ioid, (cid, reply));
                    }
                    CoordRequest::Shutdown { reply } => {
                        // Send ClearChannel for all connected channels
                        for ch in channels.values() {
                            if ch.state.is_operational() {
                                if let Some(addr) = ch.server_addr {
                                    let _ = transport_tx.send(TransportCommand::ClearChannel {
                                        cid: ch.cid,
                                        sid: ch.sid,
                                        server_addr: addr,
                                    });
                                }
                            }
                        }
                        let _ = reply.send(());
                        return; // Exit coordinator loop
                    }
                    CoordRequest::ForceRescanServer { server_addr } => {
                        diag.beacon_anomalies.fetch_add(1, Ordering::Relaxed);
                        diag.record(DiagEvent::BeaconAnomaly { server: server_addr });
                        // Like the C CA client (libca), rescan ALL disconnected/
                        // searching channels on any beacon anomaly.  The beacon
                        // address may use INADDR_ANY and won't match our stored
                        // server_addr, so a per-server lookup is unreliable.
                        let mut probed_servers = HashSet::new();
                        for ch in channels.values() {
                            if ch.state == ChannelState::Disconnected
                                || ch.state == ChannelState::Searching
                            {
                                let _ = search_tx.send(SearchRequest::Schedule {
                                    cid: ch.cid,
                                    pv_name: ch.pv_name.clone(),
                                    reason: SearchReason::BeaconAnomaly,
                                    initial_lane: 0,
                                });
                            } else if ch.state.is_operational() {
                                // Beacon anomaly on a connected server: send
                                // immediate echo probe to detect dead TCP faster
                                // (matches C EPICS beaconAnomaly watchdog flag).
                                if let Some(addr) = ch.server_addr {
                                    if probed_servers.insert(addr) {
                                        let _ = transport_tx.send(
                                            TransportCommand::EchoProbe {
                                                server_addr: addr,
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            resp = search_rx.recv() => {
                let Some(resp) = resp else { return };
                match resp {
                    SearchResponse::Found { cid, server_addr } => {
                        if let Some(ch) = channels.get_mut(&cid) {
                            if ch.state == ChannelState::Searching || ch.state == ChannelState::Disconnected {
                                if let Some(old_addr) = ch.server_addr {
                                    remove_server_channel(&mut server_channels, old_addr, cid);
                                }
                                ch.state = ChannelState::Connecting;
                                ch.server_addr = Some(server_addr);
                                server_channels.entry(server_addr).or_default().insert(cid);
                                let _ = transport_tx.send(TransportCommand::CreateChannel {
                                    cid,
                                    pv_name: ch.pv_name.clone(),
                                    server_addr,
                                });
                            }
                        } else {
                            // Channel not registered yet — stash the Found
                            // response so RegisterChannel can process it.
                            pending_found.insert(cid, server_addr);
                        }
                    }
                }
            }
            evt = transport_rx.recv() => {
                let Some(evt) = evt else { return };
                match evt {
                    TransportEvent::ChannelCreated { cid, sid, data_type, element_count, access, server_addr } => {
                        if let Some(ch) = channels.get_mut(&cid) {
                            let dbr_type = DbFieldType::from_u16(data_type).ok();
                            ch.state = ChannelState::Connected;
                            ch.sid = sid;
                            ch.native_type = dbr_type;
                            ch.element_count = element_count;
                            ch.server_addr = Some(server_addr);
                            ch.access_rights = access;
                            ch.last_connected_at = Some(std::time::Instant::now());

                            // Wake connect waiters
                            for waiter in ch.connect_waiters.drain(..) {
                                let _ = waiter.send(());
                            }

                            // Broadcast connected + access rights events
                            let _ = ch.conn_tx.send(ConnectionEvent::Connected);
                            let _ = ch.conn_tx.send(ConnectionEvent::AccessRightsChanged {
                                read: access.read,
                                write: access.write,
                            });

                            // Restore subscriptions
                            let (restored, stale) = subscriptions.restore_for_channel(
                                cid,
                                sid,
                                data_type,
                                element_count,
                                server_addr,
                                &transport_tx,
                            );
                            diag.connections.fetch_add(1, Ordering::Relaxed);
                            diag.record(DiagEvent::Connected { pv: ch.pv_name.clone(), server: server_addr });
                            if restored > 0 || stale > 0 {
                                diag.reconnections.fetch_add(1, Ordering::Relaxed);
                                diag.subscriptions_restored.fetch_add(restored as u64, Ordering::Relaxed);
                                diag.subscriptions_stale.fetch_add(stale as u64, Ordering::Relaxed);
                                diag.record(DiagEvent::Reconnected { pv: ch.pv_name.clone(), restored, stale });
                                eprintln!("CA: {}: restored {restored} subscriptions ({stale} stale removed)", ch.pv_name);
                            }

                            // Notify search engine of successful connect (clears penalty).
                            let _ = search_tx.send(SearchRequest::ConnectResult {
                                cid,
                                success: true,
                                server_addr,
                            });
                        }
                    }
                    TransportEvent::ReadResponse { ioid, data_type, count, data } => {
                        if let Some((_, waiter)) = read_waiters.remove(&ioid) {
                            let _ = waiter.send(Ok((data_type, count, data)));
                        }
                    }
                    TransportEvent::ReadError { ioid, eca_status } => {
                        if let Some((_, waiter)) = read_waiters.remove(&ioid) {
                            let _ = waiter.send(Err(CaError::Protocol(
                                format!("server returned ECA error {eca_status:#06x}")
                            )));
                        }
                    }
                    TransportEvent::WriteResponse { ioid, status } => {
                        if let Some((_, waiter)) = write_waiters.remove(&ioid) {
                            if status == 1 || status == ECA_NORMAL {
                                let _ = waiter.send(Ok(()));
                            } else {
                                let _ = waiter.send(Err(CaError::WriteFailed(status)));
                            }
                        }
                    }
                    TransportEvent::MonitorData { subid, data_type, count, data } => {
                        use subscription::MonitorDeliveryOutcome;
                        match subscriptions.on_monitor_data(subid, data_type, count, &data) {
                            MonitorDeliveryOutcome::Queued(server_addr) => {
                                flow_control_note_queued(
                                    &mut flow_control,
                                    server_addr,
                                    &transport_tx,
                                );
                            }
                            MonitorDeliveryOutcome::Dropped(_server_addr) => {
                                diag.dropped_monitors.fetch_add(1, Ordering::Relaxed);
                            }
                            MonitorDeliveryOutcome::Filtered
                            | MonitorDeliveryOutcome::NotFound => {}
                        }
                    }
                    TransportEvent::AccessRightsChanged { cid, access } => {
                        if let Some(ch) = channels.get_mut(&cid) {
                            ch.access_rights = access;
                            let _ = ch.conn_tx.send(ConnectionEvent::AccessRightsChanged {
                                read: access.read,
                                write: access.write,
                            });
                        }
                    }
                    TransportEvent::ChannelCreateFailed { cid } => {
                        if let Some(ch) = channels.get_mut(&cid) {
                            let server_addr = ch.server_addr;
                            // Keep connect waiters pending. ChannelCreateFailed
                            // only means this specific attempt/server failed;
                            // the channel will immediately re-search and may
                            // still connect before the caller's timeout.
                            ch.state = ChannelState::Disconnected;
                            let _ = ch.conn_tx.send(ConnectionEvent::Disconnected);
                            let _ = search_tx.send(SearchRequest::Schedule {
                                cid,
                                pv_name: ch.pv_name.clone(),
                                reason: SearchReason::Reconnect,
                                initial_lane: 0,
                            });
                            // Notify search engine of failed connect (penalty box).
                            if let Some(addr) = server_addr {
                                let _ = search_tx.send(SearchRequest::ConnectResult {
                                    cid,
                                    success: false,
                                    server_addr: addr,
                                });
                            }
                        }
                    }
                    TransportEvent::ServerError { .. } => {
                        // Logged in transport layer; no further action needed
                    }
                    TransportEvent::TcpClosed { server_addr } => {
                        flow_control.remove(&server_addr);
                        handle_disconnect(&mut channels, &mut subscriptions, &mut server_channels, &search_tx, server_addr, &diag, &mut read_waiters, &mut write_waiters);
                    }
                    TransportEvent::ServerDisconnect { cid, server_addr } => {
                        // Single channel disconnect (CA_PROTO_SERVER_DISCONN)
                        if let Some(ch) = channels.get_mut(&cid) {
                            if ch.server_addr == Some(server_addr) {
                                ch.state = ChannelState::Disconnected;
                                let _ = ch.conn_tx.send(ConnectionEvent::Disconnected);

                                let cids = vec![cid];
                                let cleared = subscriptions.mark_disconnected(&cids);
                                for (addr, count) in cleared {
                                    flow_control_note_consumed(
                                        &mut flow_control,
                                        addr,
                                        count,
                                        &transport_tx,
                                    );
                                }

                                // Re-search
                                let _ = search_tx.send(SearchRequest::Schedule {
                                    cid,
                                    pv_name: ch.pv_name.clone(),
                                    reason: SearchReason::Reconnect,
                                    initial_lane: 0,
                                });
                            }
                        }
                    }
                    TransportEvent::CircuitUnresponsive { server_addr } => {
                        diag.unresponsive_events.fetch_add(1, Ordering::Relaxed);
                        diag.record(DiagEvent::Unresponsive { server: server_addr });
                        for ch in channels.values_mut() {
                            if ch.server_addr == Some(server_addr)
                                && ch.state == ChannelState::Connected
                            {
                                ch.state = ChannelState::Unresponsive;
                                let _ = ch.conn_tx.send(ConnectionEvent::Unresponsive);
                            }
                        }
                    }
                    TransportEvent::CircuitResponsive { server_addr } => {
                        diag.record(DiagEvent::Responsive { server: server_addr });
                        for ch in channels.values_mut() {
                            if ch.server_addr == Some(server_addr)
                                && ch.state == ChannelState::Unresponsive
                            {
                                ch.state = ChannelState::Connected;
                                let _ = ch.conn_tx.send(ConnectionEvent::Connected);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_disconnect(
    channels: &mut HashMap<u32, ChannelInner>,
    subscriptions: &mut SubscriptionRegistry,
    server_channels: &mut HashMap<SocketAddr, HashSet<u32>>,
    search_tx: &mpsc::UnboundedSender<SearchRequest>,
    server_addr: SocketAddr,
    diag: &CaDiagnostics,
    read_waiters: &mut HashMap<u32, (u32, oneshot::Sender<CaResult<(u16, u32, Vec<u8>)>>)>,
    write_waiters: &mut HashMap<u32, (u32, oneshot::Sender<CaResult<()>>)>,
) {
    let mut affected_cids = Vec::new();
    let now = std::time::Instant::now();

    for ch in channels.values_mut() {
        if ch.server_addr == Some(server_addr)
            && (ch.state.is_operational() || ch.state == ChannelState::Connecting)
        {
            ch.state = ChannelState::Disconnected;
            affected_cids.push(ch.cid);
            let _ = ch.conn_tx.send(ConnectionEvent::Disconnected);

            // Reconnection backoff: if the connection was short-lived (<30s),
            // increment reconnect_count for exponential backoff. Sustained
            // connections reset the counter.
            let sustained = ch
                .last_connected_at
                .map(|t| now.duration_since(t).as_secs() > 30)
                .unwrap_or(false);
            if sustained {
                ch.reconnect_count = 0;
            } else {
                ch.reconnect_count = ch.reconnect_count.saturating_add(1);
            }
            // Minimum lane 1 for bulk disconnects so the search engine
            // applies jitter and prevents a reconnection storm (similar
            // to C EPICS disconnectGovernorTimer batching).
            let initial_lane = ch.reconnect_count.clamp(1, 8);

            let _ = search_tx.send(SearchRequest::Schedule {
                cid: ch.cid,
                pv_name: ch.pv_name.clone(),
                reason: SearchReason::Reconnect,
                initial_lane,
            });
        }
    }
    if !affected_cids.is_empty() {
        diag.disconnections.fetch_add(1, Ordering::Relaxed);
        diag.record(DiagEvent::Disconnected {
            server: server_addr,
            channels: affected_cids.len(),
        });
    }
    // Clean up stale server_channels entries so beacon anomaly
    // lookups don't reference disconnected channels.
    server_channels.remove(&server_addr);
    let _ = subscriptions.mark_disconnected(&affected_cids);

    // Fail pending read/write waiters for affected channels so callers
    // don't hang forever waiting for a response that will never arrive.
    let affected: HashSet<u32> = affected_cids.into_iter().collect();
    let stale_reads: Vec<u32> = read_waiters
        .iter()
        .filter(|(_, (cid, _))| affected.contains(cid))
        .map(|(ioid, _)| *ioid)
        .collect();
    for ioid in stale_reads {
        if let Some((_, sender)) = read_waiters.remove(&ioid) {
            let _ = sender.send(Err(CaError::Disconnected));
        }
    }
    let stale_writes: Vec<u32> = write_waiters
        .iter()
        .filter(|(_, (cid, _))| affected.contains(cid))
        .map(|(ioid, _)| *ioid)
        .collect();
    for ioid in stale_writes {
        if let Some((_, sender)) = write_waiters.remove(&ioid) {
            let _ = sender.send(Err(CaError::Disconnected));
        }
    }
}

fn remove_server_channel(
    server_channels: &mut HashMap<SocketAddr, HashSet<u32>>,
    server_addr: SocketAddr,
    cid: u32,
) {
    if let Some(set) = server_channels.get_mut(&server_addr) {
        set.remove(&cid);
        if set.is_empty() {
            server_channels.remove(&server_addr);
        }
    }
}

fn resolve_host(host: &str, port: u16) -> CaResult<SocketAddr> {
    // Try direct IP parse first (fast path)
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return Ok(SocketAddr::V4(SocketAddrV4::new(ip, port)));
    }
    // DNS resolution — prefer IPv4 (CA protocol is IPv4-only)
    use std::net::ToSocketAddrs;
    let addr_str = format!("{host}:{port}");
    let addrs: Vec<SocketAddr> = addr_str
        .to_socket_addrs()
        .map_err(|e| CaError::Protocol(format!("cannot resolve '{host}': {e}")))?
        .collect();
    addrs
        .iter()
        .find(|a| a.is_ipv4())
        .or(addrs.first())
        .copied()
        .ok_or_else(|| CaError::Protocol(format!("no addresses for '{host}'")))
}

/// Expand shell-style `${VAR}` and `$(VAR)` references in `s` against the
/// process environment. Unknown variables expand to the empty string,
/// matching libca's expandedClient behaviour. Plain `$` and unmatched
/// braces/parens are left intact.
fn expand_shell_vars(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() {
            let close = match bytes[i + 1] {
                b'{' => Some(b'}'),
                b'(' => Some(b')'),
                _ => None,
            };
            if let Some(end) = close {
                if let Some(j) = bytes[i + 2..].iter().position(|&b| b == end) {
                    let name = &s[i + 2..i + 2 + j];
                    let value = epics_base_rs::runtime::env::get(name).unwrap_or_default();
                    out.push_str(&value);
                    i += 3 + j;
                    continue;
                }
            }
        }
        out.push(s.as_bytes()[i] as char);
        i += 1;
    }
    out
}

/// Parse `EPICS_CA_NAME_SERVERS` — whitespace-separated host[:port] entries
/// reachable over TCP. Returns an empty vec when the variable is unset, so
/// the search engine simply doesn't spawn any nameserver tasks.
fn parse_nameserver_list() -> Vec<SocketAddr> {
    let Some(list) = epics_base_rs::runtime::env::get("EPICS_CA_NAME_SERVERS") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in list.split_whitespace() {
        let resolved = if entry.contains(':') {
            entry.parse::<SocketAddr>().ok().or_else(|| {
                let (host, port_str) = entry.rsplit_once(':')?;
                let port: u16 = port_str.parse().ok()?;
                resolve_host(host, port).ok()
            })
        } else {
            resolve_host(entry, CA_SERVER_PORT).ok()
        };
        if let Some(addr) = resolved {
            out.push(addr);
        }
    }
    out
}

fn parse_addr_list() -> CaResult<Vec<SocketAddr>> {
    let mut addrs = Vec::new();

    if let Some(list) = epics_base_rs::runtime::env::get("EPICS_CA_ADDR_LIST") {
        for entry in list.split_whitespace() {
            let addr = if entry.contains(':') {
                // Try direct parse first, fall back to DNS resolution
                entry.parse::<SocketAddr>().or_else(|_| {
                    let (host, port_str) = entry.rsplit_once(':').unwrap();
                    let port: u16 = port_str
                        .parse()
                        .map_err(|e| CaError::Protocol(format!("bad port in '{entry}': {e}")))?;
                    resolve_host(host, port)
                })?
            } else {
                resolve_host(entry, CA_SERVER_PORT)?
            };
            addrs.push(addr);
        }
    }

    let auto_addr = epics_base_rs::runtime::env::get_or("EPICS_CA_AUTO_ADDR_LIST", "YES");

    if auto_addr.eq_ignore_ascii_case("YES") || addrs.is_empty() {
        addrs.push(SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::BROADCAST,
            CA_SERVER_PORT,
        )));
    }

    Ok(addrs)
}
