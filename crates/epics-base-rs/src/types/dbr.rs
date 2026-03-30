use crate::error::{CaError, CaResult};

// DBR type ranges: native(0-6), STS(7-13), TIME(14-20), GR(21-27), CTRL(28-34)
pub const DBR_STS_STRING: u16 = 7;
pub const DBR_TIME_STRING: u16 = 14;
pub const DBR_TIME_SHORT: u16 = 15;
pub const DBR_TIME_FLOAT: u16 = 16;
pub const DBR_TIME_ENUM: u16 = 17;
pub const DBR_TIME_CHAR: u16 = 18;
pub const DBR_TIME_LONG: u16 = 19;
pub const DBR_TIME_DOUBLE: u16 = 20;

/// EPICS DBR field types (native types only)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum DbFieldType {
    String = 0,
    Short = 1,  // aka Int16
    Float = 2,
    Enum = 3,
    Char = 4,   // aka UInt8
    Long = 5,   // aka Int32
    Double = 6,
}

impl DbFieldType {
    pub fn from_u16(v: u16) -> CaResult<Self> {
        match v {
            0 => Ok(Self::String),
            1 => Ok(Self::Short),
            2 => Ok(Self::Float),
            3 => Ok(Self::Enum),
            4 => Ok(Self::Char),
            5 => Ok(Self::Long),
            6 => Ok(Self::Double),
            _ => Err(CaError::UnsupportedType(v)),
        }
    }

    /// Size in bytes for a single element of this type
    pub fn element_size(&self) -> usize {
        match self {
            Self::String => 40, // MAX_STRING_SIZE
            Self::Short | Self::Enum => 2,
            Self::Float | Self::Long => 4,
            Self::Char => 1,
            Self::Double => 8,
        }
    }

    /// Return the DBR_TIME_xxx type code for this native type.
    pub fn time_dbr_type(&self) -> u16 {
        *self as u16 + 14
    }

    /// Return the DBR_CTRL_xxx type code for this native type.
    pub fn ctrl_dbr_type(&self) -> u16 {
        *self as u16 + 28
    }
}

/// Extract the native DBF type index (0-6) from any DBR type code.
fn dbr_native_index(dbr_type: u16) -> Option<u16> {
    match dbr_type {
        0..=6 => Some(dbr_type),
        7..=13 => Some(dbr_type - 7),
        14..=20 => Some(dbr_type - 14),
        21..=27 => Some(dbr_type - 21),
        28..=34 => Some(dbr_type - 28),
        _ => None,
    }
}

pub fn native_type_for_dbr(dbr_type: u16) -> CaResult<DbFieldType> {
    match dbr_native_index(dbr_type) {
        Some(idx) => DbFieldType::from_u16(idx),
        None => Err(CaError::UnsupportedType(dbr_type)),
    }
}
