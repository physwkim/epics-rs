use std::time::{SystemTime, UNIX_EPOCH};

use epics_base_rs::error::CaResult;
use epics_base_rs::server::device_support::{DeviceReadOutcome, DeviceSupport};
use epics_base_rs::server::record::Record;
use epics_base_rs::types::EpicsValue;

use chrono::Local;

/// EPICS epoch offset: seconds from Unix epoch (1970-01-01) to EPICS epoch (1990-01-01).
const EPICS_EPOCH_OFFSET: u64 = 631152000;

/// "Time of Day" device support for stringin records.
///
/// Reads the current time and formats it as a string.
/// Format depends on PHAS field:
/// - PHAS=0: "Mon DD, YYYY HH:MM:SS"
/// - PHAS!=0: "MM/DD/YY HH:MM:SS"
///
/// Ported from `devTimeOfDay.c` (`devSiTodString`).
pub struct TimeOfDayStringDeviceSupport;

impl Default for TimeOfDayStringDeviceSupport {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeOfDayStringDeviceSupport {
    pub fn new() -> Self {
        Self
    }
}

impl DeviceSupport for TimeOfDayStringDeviceSupport {
    fn dtyp(&self) -> &str {
        "Time of Day"
    }

    fn read(&mut self, record: &mut dyn Record) -> CaResult<DeviceReadOutcome> {
        let now = Local::now();

        // Check PHAS field to determine format (PHAS is managed by CommonFields,
        // but we can read it via get_field if available; default to format 0)
        let phas = record
            .get_field("PHAS")
            .and_then(|v| match v {
                EpicsValue::Short(s) => Some(s),
                _ => None,
            })
            .unwrap_or(0);

        let formatted = if phas != 0 {
            now.format("%m/%d/%y %H:%M:%S").to_string()
        } else {
            now.format("%b %d, %Y %H:%M:%S").to_string()
        };

        record.put_field("VAL", EpicsValue::String(formatted))?;
        Ok(DeviceReadOutcome::computed())
    }

    fn write(&mut self, _record: &mut dyn Record) -> CaResult<()> {
        Ok(())
    }
}

/// "Sec Past Epoch" device support for ai records.
///
/// Reads the current time as seconds past the EPICS epoch (1990-01-01).
/// If PHAS field is nonzero, includes fractional seconds.
///
/// Ported from `devTimeOfDay.c` (`devAiTodSeconds`).
pub struct SecPastEpochDeviceSupport;

impl Default for SecPastEpochDeviceSupport {
    fn default() -> Self {
        Self::new()
    }
}

impl SecPastEpochDeviceSupport {
    pub fn new() -> Self {
        Self
    }
}

impl DeviceSupport for SecPastEpochDeviceSupport {
    fn dtyp(&self) -> &str {
        "Sec Past Epoch"
    }

    fn read(&mut self, record: &mut dyn Record) -> CaResult<DeviceReadOutcome> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        let sec_past_epoch = now.as_secs().saturating_sub(EPICS_EPOCH_OFFSET);

        let phas = record
            .get_field("PHAS")
            .and_then(|v| match v {
                EpicsValue::Short(s) => Some(s),
                _ => None,
            })
            .unwrap_or(0);

        let val = if phas != 0 {
            sec_past_epoch as f64 + (now.subsec_nanos() as f64 / 1e9)
        } else {
            sec_past_epoch as f64
        };

        record.put_field("VAL", EpicsValue::Double(val))?;
        Ok(DeviceReadOutcome::computed())
    }

    fn write(&mut self, _record: &mut dyn Record) -> CaResult<()> {
        Ok(())
    }
}
