//! Full asynRecord implementation for Rust.
//!
//! Provides connection management, trace control, and I/O testing
//! via a complete port of the C asynRecord (67 fields).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::port_handle::PortHandle;
use crate::trace::TraceManager;

// ===== Global Port Registry =====

/// Entry in the global port registry.
#[derive(Clone)]
pub struct PortEntry {
    pub handle: PortHandle,
    pub trace: Arc<TraceManager>,
}

/// Global registry of ports (name → PortEntry).
static PORT_REGISTRY: OnceLock<Mutex<HashMap<String, PortEntry>>> = OnceLock::new();

fn get_port_registry() -> &'static Mutex<HashMap<String, PortEntry>> {
    PORT_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register a port so asynRecord instances can find it.
/// Called by applications after creating a port runtime.
pub fn register_port(name: &str, handle: PortHandle, trace: Arc<TraceManager>) {
    let mut reg = get_port_registry().lock().unwrap();
    reg.insert(name.to_string(), PortEntry { handle, trace });
}

/// Look up a port by name.
pub fn get_port(name: &str) -> Option<PortEntry> {
    let reg = get_port_registry().lock().ok()?;
    reg.get(name).cloned()
}

/// Return the asyn record type factory for injection into IocBuilder.
pub fn asyn_record_factory() -> (&'static str, epics_base_rs::server::RecordFactory) {
    ("asyn", Box::new(|| Box::new(super::AsynRecord::default())))
}

/// Register the "asyn" record type via the global registry (legacy).
/// Prefer `asyn_record_factory()` with `IocBuilder::register_record_type()`.
pub fn register_asyn_record_type() {
    epics_base_rs::server::db_loader::register_record_type(
        "asyn",
        Box::new(|| Box::new(super::AsynRecord::default())),
    );
}

// ===== Transfer Mode =====

