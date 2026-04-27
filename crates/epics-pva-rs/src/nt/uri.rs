//! `epics:nt/NTURI:1.0` builder.
//!
//! pvxs uses NTURI for RPC-style argument passing — query: struct of
//! typed fields, plus scheme/authority/path strings.

use crate::pvdata::{FieldDesc, PvField, PvStructure, ScalarType, ScalarValue};

#[derive(Clone)]
struct UriArg {
    name: String,
    desc: FieldDesc,
}

/// Builder for `NTURI`. Each `arg` adds a named field to the `query`
/// sub-structure with the given type. `build()` returns the FieldDesc
/// shape; `create()` returns a default-initialised value with the
/// scheme/authority/path strings empty.
pub struct NTURI {
    args: Vec<UriArg>,
}

impl NTURI {
    pub fn new() -> Self {
        Self { args: Vec::new() }
    }

    pub fn arg(mut self, name: impl Into<String>, desc: FieldDesc) -> Self {
        self.args.push(UriArg {
            name: name.into(),
            desc,
        });
        self
    }

    pub fn arg_scalar(self, name: impl Into<String>, ty: ScalarType) -> Self {
        self.arg(name, FieldDesc::Scalar(ty))
    }

    pub fn build(&self) -> FieldDesc {
        let query_fields: Vec<(String, FieldDesc)> = self
            .args
            .iter()
            .map(|a| (a.name.clone(), a.desc.clone()))
            .collect();
        FieldDesc::Structure {
            struct_id: "epics:nt/NTURI:1.0".into(),
            fields: vec![
                ("scheme".into(), FieldDesc::Scalar(ScalarType::String)),
                ("authority".into(), FieldDesc::Scalar(ScalarType::String)),
                ("path".into(), FieldDesc::Scalar(ScalarType::String)),
                (
                    "query".into(),
                    FieldDesc::Structure {
                        struct_id: String::new(),
                        fields: query_fields,
                    },
                ),
            ],
        }
    }

    pub fn create(&self) -> PvField {
        let mut root = PvStructure::new("epics:nt/NTURI:1.0");
        for fld in [("scheme", ""), ("authority", ""), ("path", "")] {
            root.fields.push((
                fld.0.into(),
                PvField::Scalar(ScalarValue::String(fld.1.into())),
            ));
        }
        let mut query = PvStructure::new("");
        for a in &self.args {
            query
                .fields
                .push((a.name.clone(), default_for(&a.desc)));
        }
        root.fields.push(("query".into(), PvField::Structure(query)));
        PvField::Structure(root)
    }
}

impl Default for NTURI {
    fn default() -> Self {
        Self::new()
    }
}

fn default_for(desc: &FieldDesc) -> PvField {
    match desc {
        FieldDesc::Scalar(t) => PvField::Scalar(default_scalar(*t)),
        FieldDesc::ScalarArray(_) => PvField::ScalarArray(Vec::new()),
        FieldDesc::Variant => {
            PvField::Variant(Box::new(crate::pvdata::VariantValue {
                desc: None,
                value: PvField::Null,
            }))
        }
        _ => PvField::Null,
    }
}

fn default_scalar(t: ScalarType) -> ScalarValue {
    match t {
        ScalarType::Boolean => ScalarValue::Boolean(false),
        ScalarType::Byte => ScalarValue::Byte(0),
        ScalarType::Short => ScalarValue::Short(0),
        ScalarType::Int => ScalarValue::Int(0),
        ScalarType::Long => ScalarValue::Long(0),
        ScalarType::UByte => ScalarValue::UByte(0),
        ScalarType::UShort => ScalarValue::UShort(0),
        ScalarType::UInt => ScalarValue::UInt(0),
        ScalarType::ULong => ScalarValue::ULong(0),
        ScalarType::Float => ScalarValue::Float(0.0),
        ScalarType::Double => ScalarValue::Double(0.0),
        ScalarType::String => ScalarValue::String(String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nt_uri_with_two_args_has_query_struct_with_two_fields() {
        let u = NTURI::new()
            .arg_scalar("arg1", ScalarType::UInt)
            .arg_scalar("arg2", ScalarType::String)
            .build();
        if let FieldDesc::Structure { struct_id, fields } = u {
            assert_eq!(struct_id, "epics:nt/NTURI:1.0");
            let query = fields
                .iter()
                .find_map(|(n, d)| if n == "query" { Some(d) } else { None })
                .expect("query");
            if let FieldDesc::Structure {
                fields: qfields, ..
            } = query
            {
                assert_eq!(qfields.len(), 2);
                let names: Vec<&str> = qfields.iter().map(|(n, _)| n.as_str()).collect();
                assert_eq!(names, vec!["arg1", "arg2"]);
            }
        }
    }
}
