//! Composite (`PvStructure`) and field (`PvField`) value types.

use std::fmt;

use super::field::FieldDesc;
use super::scalar::ScalarValue;

/// Runtime PV field value (recursive). Mirrors the full pvData value space.
#[derive(Debug, Clone, PartialEq)]
pub enum PvField {
    Scalar(ScalarValue),
    ScalarArray(Vec<ScalarValue>),
    Structure(PvStructure),
    StructureArray(Vec<PvStructure>),
    /// A union value — `selector >= 0` and `value` is the chosen variant's
    /// concrete `PvField`. `selector == -1` indicates a null union (no
    /// variant selected).
    Union {
        selector: i32,
        variant_name: String,
        value: Box<PvField>,
    },
    UnionArray(Vec<UnionItem>),
    /// "Any" — variant carries its own [`FieldDesc`]. Empty descriptor +
    /// null value indicates "no value".
    Variant(Box<VariantValue>),
    VariantArray(Vec<VariantValue>),
    /// Explicit empty value (used by null union / null variant).
    Null,
}

/// One element of a union array — same shape as the [`PvField::Union`] arm.
#[derive(Debug, Clone, PartialEq)]
pub struct UnionItem {
    pub selector: i32,
    pub variant_name: String,
    pub value: PvField,
}

/// Variant value: a [`FieldDesc`] paired with its concrete value. An empty
/// variant (no value present) carries the `null` field discriminator.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantValue {
    pub desc: Option<FieldDesc>,
    pub value: PvField,
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
            Self::StructureArray(arr) => {
                write!(f, "[")?;
                for (i, s) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{s}")?;
                }
                write!(f, "]")
            }
            Self::Union {
                selector,
                variant_name,
                value,
            } => {
                if *selector < 0 {
                    write!(f, "(null)")
                } else {
                    write!(f, "{variant_name}={value}")
                }
            }
            Self::UnionArray(items) => {
                write!(f, "[")?;
                for (i, it) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    if it.selector < 0 {
                        write!(f, "(null)")?;
                    } else {
                        write!(f, "{}={}", it.variant_name, it.value)?;
                    }
                }
                write!(f, "]")
            }
            Self::Variant(v) => write!(f, "{}", v.value),
            Self::VariantArray(items) => {
                write!(f, "[")?;
                for (i, it) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", it.value)?;
                }
                write!(f, "]")
            }
            Self::Null => write!(f, "null"),
        }
    }
}

/// A PVA structure with ordered named fields.
#[derive(Debug, Clone, PartialEq)]
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

    pub fn get_field_mut(&mut self, name: &str) -> Option<&mut PvField> {
        self.fields
            .iter_mut()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v)
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

    /// Add (or overwrite) a field.
    pub fn set(&mut self, name: &str, value: PvField) {
        for entry in &mut self.fields {
            if entry.0 == name {
                entry.1 = value;
                return;
            }
        }
        self.fields.push((name.to_string(), value));
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
        s.set("value", PvField::Scalar(ScalarValue::Double(7.5)));
        s.set("alarm", PvField::Structure(PvStructure::new("alarm_t")));
        s.set("timeStamp", PvField::Structure(PvStructure::new("time_t")));
        assert_eq!(s.get_value(), Some(&ScalarValue::Double(7.5)));
        assert_eq!(s.get_alarm().unwrap().struct_id, "alarm_t");
        assert_eq!(s.get_timestamp().unwrap().struct_id, "time_t");
    }

    #[test]
    fn set_overwrites() {
        let mut s = PvStructure::new("test");
        s.set("v", PvField::Scalar(ScalarValue::Int(1)));
        s.set("v", PvField::Scalar(ScalarValue::Int(2)));
        assert_eq!(s.fields.len(), 1);
        if let Some(PvField::Scalar(ScalarValue::Int(n))) = s.get_field("v") {
            assert_eq!(*n, 2);
        } else {
            panic!("expected scalar int");
        }
    }
}
