use epics_macros::EpicsRecord;

/// Minimal asyn record type.
/// In C EPICS, the asyn record monitors port connection status.
/// This implementation provides the CNCT field (default: connected)
/// so OPI displays show the correct connection state.
#[derive(EpicsRecord)]
#[record(type = "asyn")]
pub struct AsynRecord {
    /// Connection status: 0=Disconnected, 1=Connected
    #[field(type = "Long")]
    pub cnct: i32,
    /// Port name
    #[field(type = "String")]
    pub port: String,
    /// Trace I/O mask bit 2 (hex output)
    #[field(type = "Long")]
    pub tib2: i32,
}

impl Default for AsynRecord {
    fn default() -> Self {
        Self {
            cnct: 1, // Connected by default
            port: String::new(),
            tib2: 0,
        }
    }
}
