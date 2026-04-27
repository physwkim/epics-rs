//! Composite (`PvStructure`) and field (`PvField`) value types.

use std::fmt;

use super::scalar::ScalarValue;

/// Runtime PV field value (recursive).
#[derive(Debug, Clone)]
pub enum PvField {
    Scalar(ScalarValue),
    ScalarArray(Vec<ScalarValue>),
    Structure(PvStructure),
}

impl fmt::Display for PvField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scalar(v) => write!(f, "{v}"),
            Self::ScalarArray(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Self::Structure(s) => write!(f, "{s}"),
        }
    }
}

/// A PVA structure with ordered named fields.
#[derive(Debug, Clone)]
pub struct PvStructure {
    pub struct_id: String,
    pub fields: Vec<(String, PvField)>,
}

impl PvStructure {
    pub fn new(struct_id: &str) -> Self {
        Self {
            struct_id: struct_id.to_string(),
            fields: Vec::new(),
        }
    }

    pub fn get_field(&self, name: &str) -> Option<&PvField> {
        self.fields.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    pub fn get_value(&self) -> Option<&ScalarValue> {
        match self.get_field("value")? {
            PvField::Scalar(v) => Some(v),
            _ => None,
        }
    }

    pub fn get_alarm(&self) -> Option<&PvStructure> {
        match self.get_field("alarm")? {
            PvField::Structure(s) => Some(s),
            _ => None,
        }
    }

    pub fn get_timestamp(&self) -> Option<&PvStructure> {
        match self.get_field("timeStamp")? {
            PvField::Structure(s) => Some(s),
            _ => None,
        }
    }
}

impl fmt::Display for PvStructure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // For display, just show the value field if it exists (NTScalar-like).
        if let Some(val) = self.get_value() {
            write!(f, "{val}")
        } else {
            write!(f, "structure {} {{", self.struct_id)?;
            for (name, field) in &self.fields {
                write!(f, " {name}={field}")?;
            }
            write!(f, " }}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_structure() {
        let s = PvStructure::new("epics:nt/NTScalar:1.0");
        assert_eq!(s.struct_id, "epics:nt/NTScalar:1.0");
        assert!(s.fields.is_empty());
        assert!(s.get_value().is_none());
    }

    #[test]
    fn lookup_value_alarm_timestamp() {
        let mut s = PvStructure::new("epics:nt/NTScalar:1.0");
        s.fields
            .push(("value".into(), PvField::Scalar(ScalarValue::Double(7.5))));
        s.fields.push((
            "alarm".into(),
            PvField::Structure(PvStructure::new("alarm_t")),
        ));
        s.fields.push((
            "timeStamp".into(),
            PvField::Structure(PvStructure::new("time_t")),
        ));
        assert_eq!(s.get_value(), Some(&ScalarValue::Double(7.5)));
        assert_eq!(s.get_alarm().unwrap().struct_id, "alarm_t");
        assert_eq!(s.get_timestamp().unwrap().struct_id, "time_t");
    }
}
