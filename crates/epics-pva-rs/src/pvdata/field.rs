//! Field type description (introspection, no values).
//!
//! Includes a depth-first field-numbering walk used for monitor `BitSet`
//! deltas and `pvRequest` selectors.

use std::fmt;

use super::scalar::ScalarType;

/// Description of a field's type (for introspection, no values).
#[derive(Debug, Clone)]
pub enum FieldDesc {
    Scalar(ScalarType),
    ScalarArray(ScalarType),
    Structure {
        struct_id: String,
        fields: Vec<(String, FieldDesc)>,
    },
}

impl FieldDesc {
    /// Get the scalar type of a `value` field in a structure.
    pub fn value_scalar_type(&self) -> Option<ScalarType> {
        match self {
            FieldDesc::Structure { fields, .. } => {
                for (name, desc) in fields {
                    if name == "value" {
                        if let FieldDesc::Scalar(st) = desc {
                            return Some(*st);
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Number of immediate fields (for structures).
    pub fn field_count(&self) -> usize {
        match self {
            FieldDesc::Structure { fields, .. } => fields.len(),
            _ => 0,
        }
    }

    /// Total number of bit positions this descriptor occupies in a monitor
    /// `BitSet`. The root structure occupies bit 0; each nested field adds 1
    /// (and recursively for nested structures).
    ///
    /// pvData spec §5.4: "field n+1 follows field n, with sub-fields counted
    /// in declaration order before the next sibling".
    pub fn total_bits(&self) -> usize {
        1 + match self {
            FieldDesc::Structure { fields, .. } => fields
                .iter()
                .map(|(_, child)| child.total_bits())
                .sum::<usize>(),
            _ => 0,
        }
    }

    /// Resolve a dotted field path (e.g. `"alarm.severity"`) to the bit index
    /// it occupies in a monitor `BitSet`. Returns `None` if any path segment
    /// is missing or the root descriptor is not a structure.
    pub fn bit_for_path(&self, path: &str) -> Option<usize> {
        if path.is_empty() {
            return Some(0); // root
        }
        let parts: Vec<&str> = path.split('.').collect();
        find_bit_for_path(self, 0, &parts).map(|(idx, _)| idx)
    }
}

fn find_bit_for_path(desc: &FieldDesc, base: usize, path: &[&str]) -> Option<(usize, usize)> {
    if path.is_empty() {
        return Some((base, base + desc.total_bits()));
    }
    let head = path[0];
    let tail = &path[1..];
    if let FieldDesc::Structure { fields, .. } = desc {
        let mut offset = base + 1; // bit 0 = this structure itself
        for (name, child) in fields {
            if name == head {
                return find_bit_for_path(child, offset, tail);
            }
            offset += child.total_bits();
        }
    }
    None
}

impl fmt::Display for FieldDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_indent(f, 0)
    }
}

impl FieldDesc {
    fn fmt_indent(&self, f: &mut fmt::Formatter<'_>, indent: usize) -> fmt::Result {
        let pad = "    ".repeat(indent);
        match self {
            FieldDesc::Scalar(st) => write!(f, "{st}"),
            FieldDesc::ScalarArray(st) => write!(f, "{st}[]"),
            FieldDesc::Structure { struct_id, fields } => {
                if struct_id.is_empty() {
                    writeln!(f, "structure")?;
                } else {
                    writeln!(f, "structure {struct_id}")?;
                }
                for (name, desc) in fields {
                    write!(f, "{pad}    {name}: ")?;
                    desc.fmt_indent(f, indent + 1)?;
                    writeln!(f)?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nt_scalar_double() -> FieldDesc {
        FieldDesc::Structure {
            struct_id: "epics:nt/NTScalar:1.0".into(),
            fields: vec![
                ("value".into(), FieldDesc::Scalar(ScalarType::Double)),
                (
                    "alarm".into(),
                    FieldDesc::Structure {
                        struct_id: "alarm_t".into(),
                        fields: vec![
                            ("severity".into(), FieldDesc::Scalar(ScalarType::Int)),
                            ("status".into(), FieldDesc::Scalar(ScalarType::Int)),
                            ("message".into(), FieldDesc::Scalar(ScalarType::String)),
                        ],
                    },
                ),
                (
                    "timeStamp".into(),
                    FieldDesc::Structure {
                        struct_id: "time_t".into(),
                        fields: vec![
                            ("secondsPastEpoch".into(), FieldDesc::Scalar(ScalarType::Long)),
                            ("nanoseconds".into(), FieldDesc::Scalar(ScalarType::Int)),
                            ("userTag".into(), FieldDesc::Scalar(ScalarType::Int)),
                        ],
                    },
                ),
            ],
        }
    }

    #[test]
    fn total_bits_counts_root_plus_children() {
        let nt = nt_scalar_double();
        // 1 (root) + 1 (value) + 4 (alarm + 3 children) + 4 (timeStamp + 3 children) = 10
        assert_eq!(nt.total_bits(), 10);
    }

    #[test]
    fn bit_for_path_walks_depth_first() {
        let nt = nt_scalar_double();
        assert_eq!(nt.bit_for_path(""), Some(0));
        assert_eq!(nt.bit_for_path("value"), Some(1));
        assert_eq!(nt.bit_for_path("alarm"), Some(2));
        assert_eq!(nt.bit_for_path("alarm.severity"), Some(3));
        assert_eq!(nt.bit_for_path("alarm.status"), Some(4));
        assert_eq!(nt.bit_for_path("alarm.message"), Some(5));
        assert_eq!(nt.bit_for_path("timeStamp"), Some(6));
        assert_eq!(nt.bit_for_path("timeStamp.secondsPastEpoch"), Some(7));
        assert_eq!(nt.bit_for_path("timeStamp.nanoseconds"), Some(8));
        assert_eq!(nt.bit_for_path("timeStamp.userTag"), Some(9));
    }

    #[test]
    fn bit_for_path_unknown_returns_none() {
        let nt = nt_scalar_double();
        assert_eq!(nt.bit_for_path("doesNotExist"), None);
        assert_eq!(nt.bit_for_path("alarm.bogus"), None);
    }

    #[test]
    fn value_scalar_type_extraction() {
        let nt = nt_scalar_double();
        assert_eq!(nt.value_scalar_type(), Some(ScalarType::Double));
    }
}
