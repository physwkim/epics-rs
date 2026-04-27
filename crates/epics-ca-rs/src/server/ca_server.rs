//! Channel Access server — CaServer and CaServerBuilder.
//!
//! CaServerBuilder delegates all IOC-level bootstrap logic to
//! [`epics_base_rs::server::ioc_builder::IocBuilder`] and adds only
//! CA-specific configuration (port, access security).

use std::sync::Arc;

use epics_base_rs::error::{CaError, CaResult};
use epics_base_rs::runtime::net::CA_SERVER_PORT;
use epics_base_rs::server::record::Record;
use epics_base_rs::types::EpicsValue;

use super::{addr_list, beacon, tcp, udp};
use epics_base_rs::server::database::PvDatabase;
use epics_base_rs::server::scan::ScanScheduler;
use epics_base_rs::server::{access_security, autosave, device_support, ioc_builder, iocsh};

/// Builder for CaServer configuration.
///
/// IOC-level methods (`pv`, `record`, `db_file`, `register_device_support`,
/// etc.) delegate to the inner [`ioc_builder::IocBuilder`].  Only `port()`,
/// `acf()`, and `acf_file()` are CA-specific.
pub struct CaServerBuilder {
    ioc: ioc_builder::IocBuilder,
    port: u16,
    acf: Option<access_security::AccessSecurityConfig>,
    /// Captured by `acf_file(path)` so the built server can later
    /// `reload_acf()` from the same source. None when the ACF was
    /// supplied in-memory via `acf(config)`.
    acf_path: Option<String>,
    /// Optional CA-over-TLS configuration. When set, accepted TCP
    /// connections are wrapped in a `tokio_rustls::server::TlsStream`
    /// before the CA handshake runs.
    #[cfg(feature = "experimental-rust-tls")]
    tls: Option<crate::tls::TlsConfig>,
    /// Optional mDNS instance name for service discovery. When set
    /// (and `discovery` feature is enabled), the server announces
    /// itself as `<instance>._epics-ca._tcp.local.` on the link-local
    /// segment.
    mdns_instance: Option<String>,
    /// Extra TXT key=value pairs attached to the mDNS announce.
    mdns_txt: Vec<(String, String)>,
}

impl CaServerBuilder {
    pub fn new() -> Self {
        Self {
            ioc: ioc_builder::IocBuilder::new(),
            port: CA_SERVER_PORT,
            acf: None,
            acf_path: None,
            #[cfg(feature = "experimental-rust-tls")]
            tls: None,
            mdns_instance: None,
            mdns_txt: Vec::new(),
        }
    }

    /// Announce this IOC via mDNS as
    /// `<instance>._epics-ca._tcp.local.`. Requires the `discovery`
    /// cargo feature; without it the call still compiles but emits a
    /// warning at startup and announces nothing.
    pub fn announce_mdns(mut self, instance: impl Into<String>) -> Self {
        self.mdns_instance = Some(instance.into());
        self
    }

    /// Attach a key=value pair to the mDNS announce TXT record.
    /// Useful for site-wide metadata: `version`, `asg`, `owner`.
    pub fn announce_txt(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.mdns_txt.push((key.into(), value.into()));
        self
    }

    /// Enable CA over TLS using the supplied server-side configuration.
    /// Built with the `tls` cargo feature.
    #[cfg(feature = "experimental-rust-tls")]
    pub fn with_tls(mut self, tls: crate::tls::TlsConfig) -> Self {
        self.tls = Some(tls);
        self
    }

    // ── CA-specific methods ──────────────────────────────────────────

    /// Set the port for both UDP and TCP (default: 5064).
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Load an access security configuration file. The path is
    /// retained so `CaServer::reload_acf()` can later re-read it.
    pub fn acf_file(mut self, path: &str) -> CaResult<Self> {
        let content = std::fs::read_to_string(path).map_err(CaError::Io)?;
        self.acf = Some(access_security::parse_acf(&content)?);
        self.acf_path = Some(path.to_string());
        Ok(self)
    }

    /// Set access security configuration directly.
    pub fn acf(mut self, config: access_security::AccessSecurityConfig) -> Self {
        self.acf = Some(config);
        self
    }

    // ── IOC-delegated methods ────────────────────────────────────────

    /// Add a simple PV to be created on server start.
    pub fn pv(mut self, name: &str, initial: EpicsValue) -> Self {
        self.ioc = self.ioc.pv(name, initial);
        self
    }

    /// Add a record to be created on server start.
    pub fn record(mut self, name: &str, record: impl Record) -> Self {
        self.ioc = self.ioc.record(name, record);
        self
    }

    /// Add a pre-boxed record to be created on server start.
    pub fn record_boxed(mut self, name: &str, record: Box<dyn Record>) -> Self {
        self.ioc = self.ioc.record_boxed(name, record);
        self
    }

    /// Load records from a .db file.
    pub fn db_file(
        mut self,
        path: &str,
        macros: &std::collections::HashMap<String, String>,
    ) -> CaResult<Self> {
        self.ioc = self.ioc.db_file(path, macros)?;
        Ok(self)
    }

    /// Load records from a .db string.
    pub fn db_string(
        mut self,
        content: &str,
        macros: &std::collections::HashMap<String, String>,
    ) -> CaResult<Self> {
        self.ioc = self.ioc.db_string(content, macros)?;
        Ok(self)
    }

    /// Register a device support factory by DTYP name.
    pub fn register_device_support<F>(mut self, dtyp: &str, factory: F) -> Self
    where
        F: Fn() -> Box<dyn device_support::DeviceSupport> + Send + Sync + 'static,
    {
        self.ioc = self.ioc.register_device_support(dtyp, factory);
        self
    }

    /// Register an external record type factory.
    pub fn register_record_type<F>(mut self, type_name: &str, factory: F) -> Self
    where
        F: Fn() -> Box<dyn Record> + Send + Sync + 'static,
    {
        self.ioc = self.ioc.register_record_type(type_name, factory);
        self
    }

    /// Register a subroutine function by name (for sub records).
    pub fn register_subroutine<F>(mut self, name: &str, func: F) -> Self
    where
        F: Fn(&mut dyn Record) -> CaResult<()> + Send + Sync + 'static,
    {
        self.ioc = self.ioc.register_subroutine(name, func);
        self
    }

    /// Configure autosave with a save set configuration.
    pub fn autosave(mut self, config: autosave::SaveSetConfig) -> Self {
        self.ioc = self.ioc.autosave(config);
        self
    }

    /// Build the server.
    pub async fn build(self) -> CaResult<CaServer> {
        let (db, autosave_config) = self.ioc.build().await?;
        let acf = Arc::new(tokio::sync::RwLock::new(self.acf));
        #[cfg(feature = "experimental-rust-tls")]
        let tls = self.tls.and_then(|t| match t {
            crate::tls::TlsConfig::Server(arc) => Some(arc),
            crate::tls::TlsConfig::Client(_) => {
                tracing::warn!("client-side TlsConfig passed to CaServer; ignoring");
                None
            }
        });
        Ok(CaServer {
            db,
            port: self.port,
            acf,
            acf_source_path: std::sync::Mutex::new(self.acf_path),
            autosave_config,
            autosave_manager: None,
            conn_events: None,
            after_init_hooks: std::sync::Mutex::new(Vec::new()),
            #[cfg(feature = "experimental-rust-tls")]
            tls,
            mdns_instance: self.mdns_instance,
            mdns_txt: self.mdns_txt,
        })
    }
}

/// A Channel Access server (IOC) that hosts process variables.
pub struct CaServer {
    db: Arc<PvDatabase>,
    port: u16,
    /// Active access security configuration. Wrapped in `RwLock` so
    /// `reload_acf` can swap it without restarting the server. Access
    /// checks acquire a read lock; reload acquires write.
    acf: Arc<tokio::sync::RwLock<Option<access_security::AccessSecurityConfig>>>,
    /// Path the ACF was originally loaded from, retained so the no-arg
    /// `reload_acf()` knows which file to re-read. None when the server
    /// was built via the in-memory `acf(config)` setter.
    acf_source_path: std::sync::Mutex<Option<String>>,
    autosave_config: Option<autosave::SaveSetConfig>,
    autosave_manager: Option<Arc<autosave::AutosaveManager>>,
    /// Optional broadcast channel for connection lifecycle events.
    /// Subscribers (e.g. ca-gateway) get one event per accept/disconnect.
    conn_events: Option<tokio::sync::broadcast::Sender<crate::server::tcp::ServerConnectionEvent>>,
    /// Callbacks to run after PINI processing (e.g., start pollers).
    after_init_hooks: std::sync::Mutex<Vec<Box<dyn FnOnce() + Send>>>,
    /// Optional TLS configuration. When set, accepted TCP connections
    /// are wrapped in a `tokio_rustls::server::TlsStream` before the
    /// CA handshake runs. mTLS configurations additionally extract a
    /// verified peer identity for ACF rule matching.
    #[cfg(feature = "experimental-rust-tls")]
    tls: Option<Arc<tokio_rustls::rustls::ServerConfig>>,
    /// mDNS instance name to announce as. None disables announce.
    mdns_instance: Option<String>,
    /// Extra TXT key=value pairs for the mDNS announce.
    mdns_txt: Vec<(String, String)>,
}

impl CaServer {
    /// Create a builder for configuring the server.
    pub fn builder() -> CaServerBuilder {
        CaServerBuilder::new()
    }

    /// Construct a CaServer from pre-populated parts.
    /// Used by [`ioc_app::IocApplication`] after st.cmd execution and device support wiring.
    pub fn from_parts(
        db: Arc<PvDatabase>,
        port: u16,
        acf: Option<access_security::AccessSecurityConfig>,
        autosave_config: Option<autosave::SaveSetConfig>,
        autosave_manager: Option<Arc<autosave::AutosaveManager>>,
    ) -> Self {
        Self {
            db,
            port,
            acf: Arc::new(tokio::sync::RwLock::new(acf)),
            acf_source_path: std::sync::Mutex::new(None),
            autosave_config,
            autosave_manager,
            conn_events: None,
            after_init_hooks: std::sync::Mutex::new(Vec::new()),
            #[cfg(feature = "experimental-rust-tls")]
            tls: None,
            mdns_instance: None,
            mdns_txt: Vec::new(),
        }
    }

    /// Re-read the ACF file the server was originally configured with
    /// and atomically swap in the new configuration. The new rules take
    /// effect on the next access check (CREATE_CHAN, HOST_NAME, or
    /// CLIENT_NAME message); already-allocated channel access bits stay
    /// in place until re-evaluated.
    ///
    /// Errors when no source path is registered. Use `reload_acf_from`
    /// with an explicit path when the server was constructed via
    /// `acf(config)` rather than `acf_file(path)`.
    pub async fn reload_acf(&self) -> CaResult<()> {
        let path = self
            .acf_source_path
            .lock()
            .map_err(|_| CaError::InvalidValue("acf_source_path lock poisoned".into()))?
            .clone();
        match path {
            Some(p) => self.reload_acf_from(&p).await,
            None => Err(CaError::InvalidValue(
                "no ACF source path registered; use reload_acf_from() with an explicit path"
                    .into(),
            )),
        }
    }

    /// Re-read ACF from an arbitrary path. Use this when the source has
    /// moved or when the server was originally configured in-memory.
    pub async fn reload_acf_from(&self, path: &str) -> CaResult<()> {
        let content = std::fs::read_to_string(path).map_err(CaError::Io)?;
        let parsed = access_security::parse_acf(&content)?;
        {
            let mut guard = self.acf.write().await;
            *guard = Some(parsed);
        }
        if let Ok(mut p) = self.acf_source_path.lock() {
            *p = Some(path.to_string());
        }
        tracing::info!(path = %path, "ACF reloaded; new rules apply to subsequent access checks");
        metrics::counter!("ca_server_acf_reloads_total").increment(1);
        Ok(())
    }

    /// Returns the path the ACF was loaded from, if any.
    pub fn acf_source_path(&self) -> Option<String> {
        self.acf_source_path.lock().ok().and_then(|g| g.clone())
    }

    /// Set callbacks to run after PINI processing completes.
    pub fn set_after_init_hooks(&mut self, hooks: Vec<Box<dyn FnOnce() + Send>>) {
        *self.after_init_hooks.lock().unwrap() = hooks;
    }

    /// Subscribe to connection lifecycle events. Returns a broadcast
    /// receiver that receives [`ServerConnectionEvent::Connected`] /
    /// `Disconnected` for each accepted client.
    ///
    /// Idempotent: calling multiple times shares the same broadcast sender.
    pub fn connection_events(
        &mut self,
    ) -> tokio::sync::broadcast::Receiver<crate::server::tcp::ServerConnectionEvent> {
        match &self.conn_events {
            Some(tx) => tx.subscribe(),
            None => {
                let (tx, rx) = tokio::sync::broadcast::channel(64);
                self.conn_events = Some(tx);
                rx
            }
        }
    }

    /// Expose PV database for shell/external use.
    pub fn database(&self) -> &Arc<PvDatabase> {
        &self.db
    }

    /// Run server + interactive shell. Shell exit stops server.
    pub async fn run_with_shell<F>(self, register_fn: F) -> CaResult<()>
    where
        F: FnOnce(&iocsh::IocShell) + Send + 'static,
    {
        let db = self.db.clone();
        let handle = tokio::runtime::Handle::current();

        let autosave_cmds = self
            .autosave_manager
            .as_ref()
            .map(|mgr| autosave::iocsh::autosave_commands(mgr.clone()));

        let server = Arc::new(self);

        let server_clone = server.clone();
        let server_handle =
            epics_base_rs::runtime::task::spawn(async move { server_clone.run().await });

        let (tx, rx) = epics_base_rs::runtime::sync::oneshot::channel();
        std::thread::spawn(move || {
            let shell = iocsh::IocShell::new(db, handle);
            register_fn(&shell);
            if let Some(cmds) = autosave_cmds {
                for cmd in cmds {
                    shell.register(cmd);
                }
            }
            let result = shell.run_repl();
            let _ = tx.send(result);
        });

        let shell_result = rx.await;

        server_handle.abort();
        let _ = server_handle.await;

        match shell_result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => {
                eprintln!("shell error: {e}");
                Err(CaError::InvalidValue(e))
            }
            Err(_) => {
                eprintln!("shell thread dropped unexpectedly");
                Err(CaError::InvalidValue("shell thread dropped".to_string()))
            }
        }
    }

    /// Add a simple PV at runtime.
    pub async fn add_pv(&self, name: &str, initial: EpicsValue) {
        self.db.add_pv(name, initial).await;
    }

    /// Add a record at runtime.
    pub async fn add_record(&self, name: &str, record: impl Record) {
        self.db.add_record(name, Box::new(record)).await;
    }

    /// Set a PV value (notifies subscribers).
    pub async fn put(&self, name: &str, value: EpicsValue) -> CaResult<()> {
        self.db.put_pv(name, value).await
    }

    /// Get a PV value.
    pub async fn get(&self, name: &str) -> CaResult<EpicsValue> {
        self.db.get_pv(name).await
    }

    /// Run the server (UDP + TCP + beacon + scan scheduler).
    /// This function runs indefinitely.
    pub async fn run(&self) -> CaResult<()> {
        let db_udp = self.db.clone();
        let db_tcp = self.db.clone();
        let db_scan = self.db.clone();
        let acf = self.acf.clone();
        let port = self.port;

        let scanner = ScanScheduler::new(db_scan);

        // Spawn autosave: prefer existing manager, otherwise build one from SaveSetConfig
        let autosave_handle = if let Some(ref mgr) = self.autosave_manager {
            let mgr = mgr.clone();
            let db_save = self.db.clone();
            Some(mgr.start(db_save))
        } else if let Some(ref cfg) = self.autosave_config {
            let builder = autosave::AutosaveBuilder::new().add_set(cfg.clone());
            match builder.build().await {
                Ok(mgr) => {
                    let mgr = Arc::new(mgr);
                    let db_save = self.db.clone();
                    Some(mgr.start(db_save))
                }
                Err(e) => {
                    eprintln!("autosave: failed to start: {e}");
                    None
                }
            }
        } else {
            None
        };

        let (tcp_tx, tcp_rx) = tokio::sync::oneshot::channel();
        let beacon_reset = std::sync::Arc::new(tokio::sync::Notify::new());
        let beacon_reset_tcp = beacon_reset.clone();

        let conn_events = self.conn_events.clone();
        #[cfg(feature = "experimental-rust-tls")]
        let tls = match self.tls.clone() {
            Some(cfg) => Some(cfg),
            None => match crate::tls::server_from_env() {
                Ok(Some(crate::tls::TlsConfig::Server(arc))) => Some(arc),
                Ok(Some(crate::tls::TlsConfig::Client(_))) => {
                    tracing::warn!("client-side TlsConfig produced by server_from_env; ignoring");
                    None
                }
                Ok(None) => None,
                Err(e) => {
                    tracing::error!(error = %e,
                        "EPICS_CAS_TLS_* configuration is invalid; starting in plaintext mode");
                    None
                }
            },
        };
        #[cfg(feature = "experimental-rust-tls")]
        if tls.is_some() {
            tracing::warn!(
                "═══════════════════════════════════════════════════════════════════════\n  \
                 CA-over-TLS ENABLED — non-standard, Rust-only extension.\n  \
                 C tools (caget/caput/camonitor/EDM/MEDM/CSS) and pyepics CANNOT connect.\n  \
                 For interoperable encryption use network-layer (IPSec/WireGuard/VPN).\n  \
                 See doc/11-tls-design.md for rationale.\n  \
                 ═══════════════════════════════════════════════════════════════════════"
            );
            metrics::counter!("ca_server_tls_enabled_total").increment(1);
        }
        let tcp_handle = epics_base_rs::runtime::task::spawn(async move {
            #[cfg(feature = "experimental-rust-tls")]
            {
                tcp::run_tcp_listener(
                    db_tcp,
                    port,
                    acf,
                    tcp_tx,
                    beacon_reset_tcp,
                    conn_events,
                    tls,
                )
                .await
            }
            #[cfg(not(feature = "experimental-rust-tls"))]
            {
                tcp::run_tcp_listener(db_tcp, port, acf, tcp_tx, beacon_reset_tcp, conn_events)
                    .await
            }
        });
        let tcp_abort = tcp_handle.abort_handle();

        let tcp_port = tcp_rx.await.map_err(|_| {
            CaError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "TCP listener failed to start",
            ))
        })?;

        let udp_cfg = addr_list::from_env();
        eprintln!(
            "CA server: UDP search on port {port}, TCP on port {tcp_port}, beacons → {} address(es)",
            udp_cfg.beacon_addrs.len()
        );

        // mDNS announce: held for the lifetime of run(). Drops when
        // the function returns, deregistering us from the network.
        #[cfg(feature = "discovery")]
        let _mdns = if let Some(ref instance) = self.mdns_instance {
            match crate::discovery::MdnsBackend::announce_helper(
                instance,
                tcp_port,
                self.mdns_txt.clone(),
            ) {
                Ok(announcer) => {
                    tracing::info!(instance = %instance, port = tcp_port,
                        "mDNS announce active");
                    metrics::counter!("ca_server_mdns_announces_total").increment(1);
                    Some(announcer)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "mDNS announce failed; continuing without it");
                    None
                }
            }
        } else {
            None
        };
        #[cfg(not(feature = "discovery"))]
        if self.mdns_instance.is_some() {
            tracing::warn!(
                "mDNS announce requested via .announce_mdns() but built without `discovery` \
                 cargo feature; ignoring"
            );
        }

        // Spawn UDP responder as its own task so its waker isn't multiplexed
        // through a select! branch (which can drop/replace wakers between polls
        // and miss edge-triggered epoll events).
        let intf_addrs = udp_cfg.intf_addrs.clone();
        let ignore_addrs = udp_cfg.ignore_addrs.clone();
        let udp_handle = epics_base_rs::runtime::task::spawn(async move {
            udp::run_udp_search_responder(db_udp, port, tcp_port, intf_addrs, ignore_addrs).await
        });
        let udp_abort = udp_handle.abort_handle();

        let result = tokio::select! {
            r = udp_handle => {
                eprintln!("UDP responder exited: {r:?}");
                match r {
                    Ok(inner) => inner,
                    Err(e) => Err(CaError::Io(
                        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
                    )),
                }
            }
            r = tcp_handle => {
                eprintln!("TCP listener exited: {r:?}");
                match r {
                    Ok(inner) => inner,
                    Err(e) => Err(CaError::Io(
                        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
                    )),
                }
            }
            r = beacon::run_beacon_emitter(
                tcp_port,
                udp_cfg.beacon_addrs.clone(),
                udp_cfg.beacon_period,
                beacon_reset,
            ) => {
                eprintln!("Beacon emitter exited: {r:?}");
                r
            }
            _ = scanner.run_with_hooks(self.after_init_hooks.lock().unwrap().drain(..).collect()) => {
                eprintln!("Scan scheduler exited");
                Ok(())
            }
        };

        // Tear down spawned tasks whose JoinHandles were moved into the
        // select!. Calling abort() on a handle whose task already finished
        // is a no-op, so it's safe to call unconditionally.
        udp_abort.abort();
        tcp_abort.abort();
        if let Some(h) = autosave_handle {
            h.abort();
        }
        result
    }
}
