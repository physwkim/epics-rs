//! pvxs-compatible output formatting for PVA values and type descriptors.
//!
//! Matches the output style of C++ pvxs `pvget`, `pvinfo`, etc.

use spvirit_codec::spvd_decode::{DecodedValue, FieldDesc, FieldType, StructureDesc, TypeCode};

// ─── pvinfo formatting (type descriptor) ────────────────────────────────────

/// Format a `StructureDesc` in pvxs `pvinfo` style.
///
/// ```text
/// epics:nt/NTNDArray:1.0
///     union value
///         boolean[] booleanValue
///     codec_t codec
///         string name
/// ```
pub fn format_info(desc: &StructureDesc) -> String {
    format_info_indented(desc, 0)
}

/// Format a `StructureDesc` with a base indentation level.
/// Used by pvinfo-rs to render under the `Type:` header.
pub fn format_info_indented(desc: &StructureDesc, base_depth: usize) -> String {
    let mut out = String::new();
    let indent = "    ".repeat(base_depth);
    let id = desc.struct_id.as_deref().unwrap_or("structure");
    out.push_str(&format!("{indent}{id}\n"));
    for field in &desc.fields {
        format_info_field(&mut out, field, base_depth + 1);
    }
    out
}

fn format_info_field(out: &mut String, field: &FieldDesc, depth: usize) {
    let indent = "    ".repeat(depth);
    match &field.field_type {
        FieldType::Structure(desc) => {
            let id = desc.struct_id.as_deref().unwrap_or("structure");
            out.push_str(&format!("{indent}{id} {}\n", field.name));
            for f in &desc.fields {
                format_info_field(out, f, depth + 1);
            }
        }
        FieldType::StructureArray(desc) => {
            let id = desc.struct_id.as_deref().unwrap_or("structure");
            out.push_str(&format!("{indent}{id}[] {}\n", field.name));
            // Show element structure indented
            let inner_indent = "    ".repeat(depth + 1);
            out.push_str(&format!("{inner_indent}{id}\n"));
            for f in &desc.fields {
                format_info_field(out, f, depth + 2);
            }
        }
        FieldType::Union(fields) => {
            out.push_str(&format!("{indent}union {}\n", field.name));
            for f in fields {
                format_info_field(out, f, depth + 1);
            }
        }
        FieldType::UnionArray(fields) => {
            out.push_str(&format!("{indent}union[] {}\n", field.name));
            for f in fields {
                format_info_field(out, f, depth + 1);
            }
        }
        _ => {
            out.push_str(&format!(
                "{indent}{} {}\n",
                pvxs_type_name(&field.field_type),
                field.name
            ));
        }
    }
}

/// Type name in pvxs style (all array types fully qualified).
fn pvxs_type_name(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Scalar(tc) => typecode_name(*tc),
        FieldType::ScalarArray(tc) => typecode_array_name(*tc),
        FieldType::String => "string",
        FieldType::StringArray => "string[]",
        FieldType::Variant => "any",
        FieldType::VariantArray => "any[]",
        FieldType::BoundedString(_) => "string",
        // Structure/Union handled by caller
        FieldType::Structure(_) => "structure",
        FieldType::StructureArray(_) => "structure[]",
        FieldType::Union(_) => "union",
        FieldType::UnionArray(_) => "union[]",
    }
}

fn typecode_name(tc: TypeCode) -> &'static str {
    match tc {
        TypeCode::Boolean => "boolean",
        TypeCode::Int8 => "byte",
        TypeCode::Int16 => "short",
        TypeCode::Int32 => "int",
        TypeCode::Int64 => "long",
        TypeCode::UInt8 => "ubyte",
        TypeCode::UInt16 => "ushort",
        TypeCode::UInt32 => "uint",
        TypeCode::UInt64 => "ulong",
        TypeCode::Float32 => "float",
        TypeCode::Float64 => "double",
        TypeCode::String => "string",
        _ => "unknown",
    }
}

fn typecode_array_name(tc: TypeCode) -> &'static str {
    match tc {
        TypeCode::Boolean => "boolean[]",
        TypeCode::Int8 => "byte[]",
        TypeCode::Int16 => "short[]",
        TypeCode::Int32 => "int[]",
        TypeCode::Int64 => "long[]",
        TypeCode::UInt8 => "ubyte[]",
        TypeCode::UInt16 => "ushort[]",
        TypeCode::UInt32 => "uint[]",
        TypeCode::UInt64 => "ulong[]",
        TypeCode::Float32 => "float[]",
        TypeCode::Float64 => "double[]",
        TypeCode::String => "string[]",
        _ => "array",
    }
}

// ─── pvget raw/verbose formatting (type + value) ────────────────────────────

/// Format value with type descriptors in pvxs raw/verbose style.
///
/// ```text
/// SIM1:cam1:Gain_RBV epics:nt/NTScalar:1.0
///     double value 1
///     alarm_t alarm
///         int severity 0
/// ```
pub fn format_raw(pv_name: &str, desc: &StructureDesc, value: &DecodedValue) -> String {
    let mut out = String::new();
    let id = desc.struct_id.as_deref().unwrap_or("structure");
    out.push_str(&format!("{pv_name} {id} \n"));
    if let DecodedValue::Structure(fields) = value {
        for (name, val) in fields {
            if let Some(fd) = desc.fields.iter().find(|f| f.name == *name) {
                format_raw_field(&mut out, fd, val, 1);
            }
        }
    }
    out
}

fn format_raw_field(out: &mut String, desc: &FieldDesc, value: &DecodedValue, depth: usize) {
    let indent = "    ".repeat(depth);
    match (&desc.field_type, value) {
        (FieldType::Structure(sdesc), DecodedValue::Structure(fields)) => {
            let id = sdesc.struct_id.as_deref().unwrap_or("structure");
            // Check if it's a timestamp — format specially
            if is_timestamp_struct(sdesc) {
                let ts_str = format_timestamp(fields);
                out.push_str(&format!("{indent}{id} {} {ts_str}\n", desc.name));
            } else if is_enum_struct(sdesc) {
                // enum_t: show "(index) choice" summary on the same line
                let summary = format_enum_summary(fields);
                out.push_str(&format!("{indent}{id} {} {summary}\n", desc.name));
            } else {
                out.push_str(&format!("{indent}{id} {}\n", desc.name));
            }
            for (name, val) in fields {
                if let Some(fd) = sdesc.fields.iter().find(|f| f.name == *name) {
                    format_raw_field(out, fd, val, depth + 1);
                }
            }
        }
        (FieldType::StructureArray(sdesc), DecodedValue::Array(items)) => {
            let id = sdesc.struct_id.as_deref().unwrap_or("structure");
            out.push_str(&format!("{indent}{id}[] {}\n", desc.name));
            for item in items {
                if let DecodedValue::Structure(fields) = item {
                    out.push_str(&format!("{}    {id} \n", indent));
                    for (name, val) in fields {
                        if let Some(fd) = sdesc.fields.iter().find(|f| f.name == *name) {
                            format_raw_field(out, fd, val, depth + 2);
                        }
                    }
                }
            }
        }
        (FieldType::Union(_), DecodedValue::Structure(fields)) => {
            // Union displayed as selected variant
            if let Some((variant_name, variant_val)) = fields.first() {
                let type_name = value_type_name(variant_val);
                out.push_str(&format!(
                    "{indent}union {}\n{indent}    {type_name} {variant_name} {}\n",
                    desc.name,
                    format_value_inline(variant_val)
                ));
            } else {
                out.push_str(&format!("{indent}union {}\n", desc.name));
            }
        }
        _ => {
            let type_name = pvxs_type_name(&desc.field_type);
            out.push_str(&format!(
                "{indent}{type_name} {} {}\n",
                desc.name,
                format_value_inline(value)
            ));
        }
    }
}

// ─── pvget NT mode formatting ───────────────────────────────────────────────

/// Format value in NT mode (default pvget output).
///
/// NTScalar: `PV_NAME timestamp value`
/// NTEnum: `PV_NAME timestamp (index) choice`
/// Other: falls back to raw mode.
pub fn format_nt(pv_name: &str, desc: &StructureDesc, value: &DecodedValue) -> String {
    let struct_id = desc.struct_id.as_deref().unwrap_or("");
    let fields = match value {
        DecodedValue::Structure(f) => f,
        _ => return format!("{pv_name} {value}\n"),
    };

    if struct_id.starts_with("epics:nt/NTScalar:") {
        format_nt_scalar(pv_name, fields)
    } else if struct_id.starts_with("epics:nt/NTEnum:") {
        format_nt_enum(pv_name, fields)
    } else {
        // Fall back to raw for non-NT or complex types
        format_raw(pv_name, desc, value)
    }
}

fn format_nt_scalar(pv_name: &str, fields: &[(String, DecodedValue)]) -> String {
    let val = find_field(fields, "value")
        .map(|v| format_scalar_value(v))
        .unwrap_or_default();
    let ts = find_field(fields, "timeStamp")
        .map(|v| {
            if let DecodedValue::Structure(ts_fields) = v {
                format_timestamp(ts_fields)
            } else {
                "<undefined>".to_string()
            }
        })
        .unwrap_or_else(|| "<undefined>".to_string());
    // pvxs aligns columns: PV_NAME  timestamp  value
    format!("{pv_name} {ts} {val}\n")
}

fn format_nt_enum(pv_name: &str, fields: &[(String, DecodedValue)]) -> String {
    let ts = find_field(fields, "timeStamp")
        .map(|v| {
            if let DecodedValue::Structure(ts_fields) = v {
                format_timestamp(ts_fields)
            } else {
                "<undefined>".to_string()
            }
        })
        .unwrap_or_else(|| "<undefined>".to_string());

    let (index, choice) =
        if let Some(DecodedValue::Structure(enum_fields)) = find_field(fields, "value") {
            let idx = find_field(enum_fields, "index")
                .map(|v| format_scalar_value(v))
                .unwrap_or_else(|| "0".to_string());
            let choice =
                if let Some(DecodedValue::Array(choices)) = find_field(enum_fields, "choices") {
                    let idx_num: usize = idx.parse().unwrap_or(0);
                    choices
                        .get(idx_num)
                        .map(|v| {
                            if let DecodedValue::String(s) = v {
                                s.clone()
                            } else {
                                format!("{v}")
                            }
                        })
                        .unwrap_or_default()
                } else {
                    String::new()
                };
            (idx, choice)
        } else {
            ("0".to_string(), String::new())
        };

    format!("{pv_name} {ts} ({index}) {choice}\n")
}

// ─── JSON formatting ────────────────────────────────────────────────────────

/// Format value as JSON (pvget -M json style).
pub fn format_json(pv_name: &str, value: &DecodedValue) -> String {
    let json = decoded_to_json(value);
    format!("{pv_name} {json}\n")
}

fn decoded_to_json(value: &DecodedValue) -> String {
    match value {
        DecodedValue::Null => "null".to_string(),
        DecodedValue::Boolean(v) => format!("{v}"),
        DecodedValue::Int8(v) => format!("{v}"),
        DecodedValue::Int16(v) => format!("{v}"),
        DecodedValue::Int32(v) => format!("{v}"),
        DecodedValue::Int64(v) => format!("{v}"),
        DecodedValue::UInt8(v) => format!("{v}"),
        DecodedValue::UInt16(v) => format!("{v}"),
        DecodedValue::UInt32(v) => format!("{v}"),
        DecodedValue::UInt64(v) => format!("{v}"),
        DecodedValue::Float32(v) => {
            if v.fract() == 0.0 {
                format!("{v:.1}")
            } else {
                format!("{v}")
            }
        }
        DecodedValue::Float64(v) => {
            if v.fract() == 0.0 {
                format!("{v:.1}")
            } else {
                format!("{v}")
            }
        }
        DecodedValue::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        DecodedValue::Array(arr) => {
            let items: Vec<String> = arr.iter().map(|v| decoded_to_json(v)).collect();
            format!("[{}]", items.join(","))
        }
        DecodedValue::Structure(fields) => {
            let items: Vec<String> = fields
                .iter()
                .map(|(name, val)| format!("{}:{}", name, decoded_to_json(val)))
                .collect();
            format!("{{{}}}", items.join(","))
        }
        DecodedValue::Raw(data) => format!("\"<{} bytes>\"", data.len()),
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn find_field<'a>(fields: &'a [(String, DecodedValue)], name: &str) -> Option<&'a DecodedValue> {
    fields.iter().find(|(n, _)| n == name).map(|(_, v)| v)
}

fn is_timestamp_struct(desc: &StructureDesc) -> bool {
    desc.struct_id.as_deref() == Some("time_t")
}

fn is_enum_struct(desc: &StructureDesc) -> bool {
    desc.struct_id.as_deref() == Some("enum_t")
}

/// Format enum summary: "(index) ChoiceName"
fn format_enum_summary(fields: &[(String, DecodedValue)]) -> String {
    let idx = find_field(fields, "index")
        .map(|v| format_scalar_value(v))
        .unwrap_or_else(|| "0".to_string());
    let choice = if let Some(DecodedValue::Array(choices)) = find_field(fields, "choices") {
        let idx_num: usize = idx.parse().unwrap_or(0);
        choices
            .get(idx_num)
            .map(|v| {
                if let DecodedValue::String(s) = v {
                    s.clone()
                } else {
                    format!("{v}")
                }
            })
            .unwrap_or_default()
    } else {
        String::new()
    };
    format!("({idx}) {choice}")
}

/// Format an EPICS timestamp from a time_t structure fields.
/// Returns "YYYY-MM-DD HH:MM:SS.mmm" or "<undefined>" if epoch is 0.
fn format_timestamp(fields: &[(String, DecodedValue)]) -> String {
    let sec = match find_field(fields, "secondsPastEpoch") {
        Some(DecodedValue::Int64(s)) => *s,
        Some(DecodedValue::Int32(s)) => *s as i64,
        _ => return "<undefined>".to_string(),
    };
    if sec == 0 {
        return "<undefined>".to_string();
    }
    let nsec = match find_field(fields, "nanoseconds") {
        Some(DecodedValue::Int32(n)) => *n,
        _ => 0,
    };
    // PVA time_t uses POSIX epoch (1970-01-01) directly
    let unix_sec = sec;
    let dt = chrono::DateTime::from_timestamp(unix_sec, nsec as u32);
    match dt {
        Some(dt) => {
            let local = dt.with_timezone(&chrono::Local);
            format!("{}", local.format("%Y-%m-%d %H:%M:%S.%3f"))
        }
        None => "<undefined>".to_string(),
    }
}

fn format_scalar_value(v: &DecodedValue) -> String {
    match v {
        DecodedValue::Float64(f) => {
            if f.fract() == 0.0 && f.abs() < 1e15 {
                // pvxs prints "1" not "1.0" for integer-valued doubles
                format!("{}", *f as i64)
            } else {
                format!("{f}")
            }
        }
        DecodedValue::Float32(f) => {
            if f.fract() == 0.0 && f.abs() < 1e7 {
                format!("{}", *f as i32)
            } else {
                format!("{f}")
            }
        }
        DecodedValue::String(s) => s.clone(),
        DecodedValue::Boolean(b) => (if *b { "true" } else { "false" }).to_string(),
        other => format!("{other}"),
    }
}

fn format_value_inline(v: &DecodedValue) -> String {
    match v {
        DecodedValue::Array(arr) => {
            let items: Vec<String> = arr.iter().map(|v| format_scalar_value(v)).collect();
            format!("[{}]", items.join(", "))
        }
        DecodedValue::Structure(_) => String::new(), // handled by caller
        other => format_scalar_value(other),
    }
}

fn value_type_name(v: &DecodedValue) -> &'static str {
    match v {
        DecodedValue::Boolean(_) => "boolean",
        DecodedValue::Int8(_) => "byte",
        DecodedValue::Int16(_) => "short",
        DecodedValue::Int32(_) => "int",
        DecodedValue::Int64(_) => "long",
        DecodedValue::UInt8(_) => "ubyte",
        DecodedValue::UInt16(_) => "ushort",
        DecodedValue::UInt32(_) => "uint",
        DecodedValue::UInt64(_) => "ulong",
        DecodedValue::Float32(_) => "float",
        DecodedValue::Float64(_) => "double",
        DecodedValue::String(_) => "string",
        DecodedValue::Array(_) => "array",
        DecodedValue::Structure(_) => "structure",
        _ => "unknown",
    }
}
