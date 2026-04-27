//! NormativeTypes — wire-compatible builders for the standard PVA structure
//! IDs used across EPICS:
//!
//! - `epics:nt/NTScalar:1.0` ([`scalar`])
//! - `epics:nt/NTScalarArray:1.0` ([`scalar_array`])
//! - `epics:nt/NTEnum:1.0` ([`enum_t`])
//! - `epics:nt/NTNDArray:1.0` ([`nd_array`]) — areaDetector image
//! - `epics:nt/NTAttribute:1.0` (used by NTNDArray)
//! - `epics:nt/NTTable:1.0` (planned)
//!
//! Each module produces both the `FieldDesc` introspection and a `PvField`
//! value for use with the native client/server.

pub mod nd_array;
