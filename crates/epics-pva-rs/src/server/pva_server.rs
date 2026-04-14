//! PVA server wrapper — mirrors the [`CaServer`] pattern for pvAccess.

use std::collections::HashMap;
use std::sync::Arc;

use epics_base_rs::error::{CaError, CaResult};
use epics_base_rs::server::database::PvDatabase;
use epics_base_rs::server::ioc_builder;
use epics_base_rs::server::record::Record;
use epics_base_rs::server::scan::ScanScheduler;
use epics_base_rs::server::{access_security, autosave, iocsh};
use epics_base_rs::types::EpicsValue;
use spvirit_server::monitor::MonitorRegistry;
use spvirit_server::{PvaServerConfig, run_pva_server_with_registry};

use super::bridge::{PvDatabaseStore, start_monitor_bridge, start_store_monitor_bridge};
use spvirit_server::PvStore;

// ── Builder ──────────────────────────────────────────────────────────────

/// Builder for constructing a [`PvaServer`] with simple PVs and/or records.
pub struct PvaServerBuilder {
    ioc: ioc_builder::IocBuilder,
    port: u16,
    acf: Option<access_security::AccessSecurityConfig>,
}

impl PvaServerBuilder {
    pub fn new() -> Self {
        Self {
            ioc: ioc_builder::IocBuilder::new(),
            port: epics_base_rs::runtime::net::PVA_SERVER_PORT,
            acf: None,
        }
    }

    /// Set the TCP port (UDP = port + 1).
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Add a simple PV.
    pub fn pv(mut self, name: &str, initial: EpicsValue) -> Self {
        self.ioc = self.ioc.pv(name, initial);
        self
    }

    /// Add a record.
    pub fn record(mut self, name: &str, record: impl Record) -> Self {
        self.ioc = self.ioc.record(name, record);
        self
    }

    /// Load records from a `.db` string.
    pub fn db_string(mut self, content: &str, macros: &HashMap<String, String>) -> CaResult<Self> {
        self.ioc = self.ioc.db_string(content, macros)?;
        Ok(self)
    }

    /// Load records from a `.db` file.
    pub fn db_file(mut self, path: &str, macros: &HashMap<String, String>) -> CaResult<Self> {
        self.ioc = self.ioc.db_file(path, macros)?;
        Ok(self)
    }

    /// Build the server.
    pub async fn build(self) -> CaResult<PvaServer> {
        let (db, autosave_config) = self.ioc.build().await?;
        let acf = Arc::new(self.acf);
        Ok(PvaServer {
            db,
            port: self.port,
            acf,
            autosave_config,
            autosave_manager: None,
        })
    }
}

// ── PvaServer ────────────────────────────────────────────────────────────

/// A pvAccess server (IOC) backed by a [`PvDatabase`].
///
/// Mirrors the [`epics_ca_rs::server::CaServer`] API, but serves PVs over
/// pvAccess instead of Channel Access.
pub struct PvaServer {
    db: Arc<PvDatabase>,
    port: u16,
    #[allow(dead_code)]
    acf: Arc<Option<access_security::AccessSecurityConfig>>,
    autosave_config: Option<autosave::SaveSetConfig>,
    autosave_manager: Option<Arc<autosave::AutosaveManager>>,
}

impl PvaServer {
    /// Create a builder for configuring the server.
    pub fn builder() -> PvaServerBuilder {
        PvaServerBuilder::new()
    }

    /// Construct from pre-populated parts (called by [`super::run_pva_ioc`]).
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
            acf: Arc::new(acf),
            autosave_config,
            autosave_manager,
        }
    }

    pub fn database(&self) -> &Arc<PvDatabase> {
        &self.db
    }

    /// Add a simple PV at runtime.
    pub async fn add_pv(&self, name: &str, initial: EpicsValue) {
        self.db.add_pv(name, initial).await;
    }

    /// Set a PV value (notifies subscribers).
    pub async fn put(&self, name: &str, value: EpicsValue) -> CaResult<()> {
        self.db.put_pv(name, value).await
    }

    /// Get a PV value.
    pub async fn get(&self, name: &str) -> CaResult<EpicsValue> {
        self.db.get_pv(name).await
    }

    /// Run server + interactive iocsh. Shell exit stops the server.
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

    /// Run the PVA server (UDP search + TCP handler + beacon + scan
    /// scheduler + monitor bridge). Runs indefinitely.
    ///
    /// Uses the default [`PvDatabaseStore`] backend, wiring single-record
    /// snapshots + simple PVs onto the PVA wire. For richer routing — qsrv
    /// group PVs, access control, pvRequest field filtering — construct a
    /// `QsrvPvStore` and call [`Self::run_with_store`] instead.
    pub async fn run(&self) -> CaResult<()> {
        let store = Arc::new(PvDatabaseStore::new(self.db.clone()));
        let registry = Arc::new(MonitorRegistry::new());

        // PvDatabase has a specialized bridge (subscribes to record events
        // directly). Use it, not the generic store bridge, to avoid the
        // extra channel hop for the default path.
        start_monitor_bridge(self.db.clone(), registry.clone()).await;

        self.run_with_store_and_registry(store, registry).await
    }

    /// Run the PVA server with a caller-supplied [`PvStore`] implementation.
    ///
    /// This is the entry point used by the qsrv daemon (`qsrv-rs`) and by
    /// IOCs that want to expose group PVs or custom access control. The
    /// generic monitor bridge forwards each PV's `store.subscribe(...)`
    /// stream to the spvirit monitor registry.
    pub async fn run_with_store<S: PvStore + 'static>(&self, store: Arc<S>) -> CaResult<()> {
        let registry = Arc::new(MonitorRegistry::new());
        start_store_monitor_bridge(store.clone(), registry.clone()).await;
        self.run_with_store_and_registry(store, registry).await
    }

    async fn run_with_store_and_registry<S: PvStore + 'static>(
        &self,
        store: Arc<S>,
        registry: Arc<MonitorRegistry>,
    ) -> CaResult<()> {
        let config = PvaServerConfig {
            tcp_port: self.port,
            udp_port: self.port + 1,
            ..Default::default()
        };

        let db_scan = self.db.clone();
        let scanner = ScanScheduler::new(db_scan);

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

        let result = tokio::select! {
            res = run_pva_server_with_registry(store, config, registry) => {
                res.map_err(|e| CaError::InvalidValue(e.to_string()))
            }
            _ = scanner.run() => {
                eprintln!("Scan scheduler exited");
                Ok(())
            }
        };

        if let Some(h) = autosave_handle {
            h.abort();
        }
        result
    }
}
