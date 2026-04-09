//! Group PV JSON configuration parser.
//!
//! Parses C++ QSRV-compatible group definitions from JSON.
//! See `~/epics-base/modules/pva2pva/pdbApp/configparse.cpp` for the
//! original C++ parser.
//!
//! # JSON format
//!
//! ```json
//! {
//!   "GROUP:NAME": {
//!     "+id": "some/NT:1.0",
//!     "+atomic": true,
//!     "fieldName": {
//!       "+type": "scalar",
//!       "+channel": "RECORD:FIELD",
//!       "+trigger": "*",
//!       "+putorder": 0
//!     }
//!   }
//! }
//! ```

use serde::Deserialize;
use std::collections::HashMap;

use super::pvif::FieldMapping;
use crate::error::{BridgeError, BridgeResult};

/// Definition of a group PV (multiple records composited into one PvStructure).
#[derive(Debug, Clone)]
pub struct GroupPvDef {
    pub name: String,
    pub struct_id: Option<String>,
    pub atomic: bool,
    pub members: Vec<GroupMember>,
}

/// A single member within a group PV.
#[derive(Debug, Clone)]
pub struct GroupMember {
    /// Field path within the group structure (e.g., "temperature").
    pub field_name: String,
    /// Source record and field (e.g., "TEMP:ai.VAL").
    pub channel: String,
    /// How to map the record field to PVA structure.
    pub mapping: FieldMapping,
    /// Which fields to update when this member changes.
    pub triggers: TriggerDef,
    /// Ordering for put operations.
    pub put_order: i32,
    /// Optional structure ID for this member (from `+id`).
    pub struct_id: Option<String>,
}

/// Defines which group fields are updated when a member's source record changes.
#[derive(Debug, Clone)]
pub enum TriggerDef {
    /// `"*"` — update all fields in the group.
    All,
    /// Named fields — update only these fields.
    Fields(Vec<String>),
    /// `""` — never trigger a group update for this member.
    None,
}

/// Parse group definitions from a JSON string.
///
/// The JSON is a top-level object where each key is a group name.
pub fn parse_group_config(json: &str) -> BridgeResult<Vec<GroupPvDef>> {
    let root: HashMap<String, RawGroupDef> =
        serde_json::from_str(json).map_err(|e| BridgeError::GroupConfigError(e.to_string()))?;

    let mut groups = Vec::new();
    for (name, raw) in root {
        groups.push(raw_to_group_def(name, raw)?);
    }
    groups.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(groups)
}

/// Parse group definitions from a record's `info(Q:group, ...)` tag.
///
/// In C++ QSRV, records can declare group membership via:
/// ```text
/// record(ai, "TEMP:sensor") {
///     info(Q:group, {
///         "TEMP:group": {
///             "temperature": {"+channel": "VAL", "+type": "plain", "+trigger": "*"}
///         }
///     })
/// }
/// ```
///
/// The `record_name` is used as channel prefix: if `+channel` is a bare field
/// name (no `:` separator), it becomes `"record_name.FIELD"`.
pub fn parse_info_group(record_name: &str, json: &str) -> BridgeResult<Vec<GroupPvDef>> {
    let root: HashMap<String, RawGroupDef> =
        serde_json::from_str(json).map_err(|e| BridgeError::GroupConfigError(e.to_string()))?;

    let mut groups = Vec::new();
    for (name, raw) in root {
        let mut def = raw_to_group_def(name, raw)?;
        // Apply channel prefix: bare field names get record_name prefix
        for member in &mut def.members {
            if !member.channel.contains(':') && !member.channel.contains('.') {
                member.channel = format!("{}.{}", record_name, member.channel);
            }
        }
        groups.push(def);
    }
    groups.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(groups)
}

/// Merge additional group definitions into an existing set.
///
/// Members are appended to existing groups; new groups are created.
/// This supports the C++ pattern where multiple records contribute
/// members to the same group via separate info(Q:group) tags.
pub fn merge_group_defs(existing: &mut HashMap<String, GroupPvDef>, new_defs: Vec<GroupPvDef>) {
    for def in new_defs {
        if let Some(existing_def) = existing.get_mut(&def.name) {
            // Merge members into existing group
            existing_def.members.extend(def.members);
            // Update struct_id if newly specified
            if def.struct_id.is_some() {
                existing_def.struct_id = def.struct_id;
            }
            // atomic: use explicit setting if provided (C++ uses last-wins)
            // keep existing unless new def explicitly sets it
        } else {
            existing.insert(def.name.clone(), def);
        }
    }
}

// ---------------------------------------------------------------------------
// Internal JSON deserialization types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RawGroupDef {
    #[serde(rename = "+id")]
    id: Option<String>,
    #[serde(rename = "+atomic", default = "default_atomic")]
    atomic: bool,
    #[serde(flatten)]
    fields: HashMap<String, serde_json::Value>,
}

fn default_atomic() -> bool {
    true
}

fn raw_to_group_def(name: String, raw: RawGroupDef) -> BridgeResult<GroupPvDef> {
    let mut members = Vec::new();

    for (field_name, value) in &raw.fields {
        // Skip meta-keys (already extracted via named fields)
        if field_name.starts_with('+') {
            continue;
        }

        let member = parse_member(field_name, value)?;
        members.push(member);
    }

    // Sort by put_order for deterministic ordering
    members.sort_by_key(|m| m.put_order);

    // Validate trigger field references against actual member field names.
    // C++ QSRV does this in pdb.cpp:510-533 (trigger resolution phase).
    let member_names: std::collections::HashSet<&str> =
        members.iter().map(|m| m.field_name.as_str()).collect();

    for member in &members {
        if let TriggerDef::Fields(targets) = &member.triggers {
            for target in targets {
                if !member_names.contains(target.as_str()) {
                    return Err(BridgeError::GroupConfigError(format!(
                        "group '{}': member '{}' has trigger '{}' which is not a member of this group",
                        name, member.field_name, target
                    )));
                }
            }
        }
    }

    Ok(GroupPvDef {
        name,
        struct_id: raw.id,
        atomic: raw.atomic,
        members,
    })
}

fn parse_member(field_name: &str, value: &serde_json::Value) -> BridgeResult<GroupMember> {
    let obj = value.as_object().ok_or_else(|| {
        BridgeError::GroupConfigError(format!("field '{field_name}' must be an object"))
    })?;

    let channel = obj
        .get("+channel")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            BridgeError::GroupConfigError(format!("field '{field_name}' missing +channel"))
        })?
        .to_string();

    let mapping = match obj.get("+type").and_then(|v| v.as_str()) {
        Some("scalar") | None => FieldMapping::Scalar,
        Some("plain") => FieldMapping::Plain,
        Some("meta") => FieldMapping::Meta,
        Some("any") => FieldMapping::Any,
        Some("proc") => FieldMapping::Proc,
        Some(other) => {
            return Err(BridgeError::GroupConfigError(format!(
                "unknown +type '{other}' for field '{field_name}'"
            )));
        }
    };

    let triggers = match obj.get("+trigger").and_then(|v| v.as_str()) {
        Some("*") | None => TriggerDef::All,
        Some("") => TriggerDef::None,
        Some(s) => TriggerDef::Fields(s.split(',').map(|f| f.trim().to_string()).collect()),
    };

    let put_order = obj.get("+putorder").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

    let struct_id = obj
        .get("+id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(GroupMember {
        field_name: field_name.to_string(),
        channel,
        mapping,
        triggers,
        put_order,
        struct_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_group() {
        let json = r#"{
            "TEST:group": {
                "+id": "epics:nt/NTTable:1.0",
                "+atomic": true,
                "temperature": {
                    "+type": "scalar",
                    "+channel": "TEMP:ai",
                    "+trigger": "*",
                    "+putorder": 0
                },
                "pressure": {
                    "+type": "scalar",
                    "+channel": "PRESS:ai",
                    "+trigger": "temperature,pressure",
                    "+putorder": 1
                }
            }
        }"#;

        let groups = parse_group_config(json).unwrap();
        assert_eq!(groups.len(), 1);

        let g = &groups[0];
        assert_eq!(g.name, "TEST:group");
        assert_eq!(g.struct_id.as_deref(), Some("epics:nt/NTTable:1.0"));
        assert!(g.atomic);
        assert_eq!(g.members.len(), 2);

        let temp = &g.members[0];
        assert_eq!(temp.field_name, "temperature");
        assert_eq!(temp.channel, "TEMP:ai");
        assert_eq!(temp.mapping, FieldMapping::Scalar);
        assert!(matches!(temp.triggers, TriggerDef::All));
        assert_eq!(temp.put_order, 0);

        let press = &g.members[1];
        assert_eq!(press.field_name, "pressure");
        assert_eq!(press.channel, "PRESS:ai");
        if let TriggerDef::Fields(ref fields) = press.triggers {
            assert_eq!(fields, &["temperature", "pressure"]);
        } else {
            panic!("expected TriggerDef::Fields");
        }
    }

    #[test]
    fn parse_minimal_member() {
        let json = r#"{
            "GRP:min": {
                "val": {
                    "+channel": "REC:val"
                }
            }
        }"#;

        let groups = parse_group_config(json).unwrap();
        let m = &groups[0].members[0];
        assert_eq!(m.mapping, FieldMapping::Scalar); // default
        assert!(matches!(m.triggers, TriggerDef::All)); // default
        assert_eq!(m.put_order, 0); // default
    }

    #[test]
    fn parse_proc_mapping() {
        let json = r#"{
            "GRP:proc": {
                "trigger": {
                    "+type": "proc",
                    "+channel": "REC:proc",
                    "+trigger": ""
                }
            }
        }"#;

        let groups = parse_group_config(json).unwrap();
        let m = &groups[0].members[0];
        assert_eq!(m.mapping, FieldMapping::Proc);
        assert!(matches!(m.triggers, TriggerDef::None));
    }

    #[test]
    fn parse_error_missing_channel() {
        let json = r#"{
            "GRP:bad": {
                "val": {
                    "+type": "scalar"
                }
            }
        }"#;

        assert!(parse_group_config(json).is_err());
    }

    #[test]
    fn parse_multiple_groups() {
        let json = r#"{
            "GRP:b": {
                "x": { "+channel": "B:x" }
            },
            "GRP:a": {
                "y": { "+channel": "A:y" }
            }
        }"#;

        let groups = parse_group_config(json).unwrap();
        assert_eq!(groups.len(), 2);
        // Sorted by name
        assert_eq!(groups[0].name, "GRP:a");
        assert_eq!(groups[1].name, "GRP:b");
    }

    #[test]
    fn parse_member_id() {
        let json = r#"{
            "GRP:id": {
                "sensor": {
                    "+channel": "SENSOR:ai",
                    "+id": "epics:nt/NTScalar:1.0"
                }
            }
        }"#;

        let groups = parse_group_config(json).unwrap();
        let m = &groups[0].members[0];
        assert_eq!(m.struct_id.as_deref(), Some("epics:nt/NTScalar:1.0"));
    }

    #[test]
    fn parse_member_no_id() {
        let json = r#"{
            "GRP:noid": {
                "val": { "+channel": "REC:val" }
            }
        }"#;

        let groups = parse_group_config(json).unwrap();
        assert!(groups[0].members[0].struct_id.is_none());
    }

    #[test]
    fn parse_info_group_prefix() {
        let json = r#"{
            "TEMP:group": {
                "temperature": {
                    "+channel": "VAL",
                    "+type": "plain",
                    "+trigger": "*"
                }
            }
        }"#;

        let groups = parse_info_group("TEMP:sensor", json).unwrap();
        // Bare field "VAL" should become "TEMP:sensor.VAL"
        assert_eq!(groups[0].members[0].channel, "TEMP:sensor.VAL");
    }

    #[test]
    fn parse_info_group_absolute_channel() {
        let json = r#"{
            "TEMP:group": {
                "pressure": {
                    "+channel": "PRESS:ai",
                    "+type": "scalar"
                }
            }
        }"#;

        let groups = parse_info_group("TEMP:sensor", json).unwrap();
        // Absolute channel (contains ':') should be kept as-is
        assert_eq!(groups[0].members[0].channel, "PRESS:ai");
    }

    #[test]
    fn merge_groups() {
        let mut existing = HashMap::new();
        let defs1 = parse_group_config(
            r#"{
            "GRP:a": {
                "x": { "+channel": "R1:x" }
            }
        }"#,
        )
        .unwrap();
        merge_group_defs(&mut existing, defs1);

        let defs2 = parse_group_config(
            r#"{
            "GRP:a": {
                "y": { "+channel": "R2:y" }
            }
        }"#,
        )
        .unwrap();
        merge_group_defs(&mut existing, defs2);

        let grp = existing.get("GRP:a").unwrap();
        assert_eq!(grp.members.len(), 2);
    }

    #[test]
    fn trigger_validation_unknown_field() {
        let json = r#"{
            "GRP:bad": {
                "x": {
                    "+channel": "R:x",
                    "+trigger": "y,z"
                },
                "y": { "+channel": "R:y" }
            }
        }"#;

        // y exists but z doesn't — should fail
        let result = parse_group_config(json);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("'z'"), "expected error about 'z': {err}");
    }

    #[test]
    fn trigger_validation_self_reference() {
        let json = r#"{
            "GRP:ok": {
                "a": { "+channel": "R:a", "+trigger": "a,b" },
                "b": { "+channel": "R:b", "+trigger": "a" }
            }
        }"#;

        // Self-reference and cross-reference are both valid
        let result = parse_group_config(json);
        assert!(result.is_ok());
    }

    #[test]
    fn trigger_validation_star_passes() {
        let json = r#"{
            "GRP:ok": {
                "a": { "+channel": "R:a", "+trigger": "*" }
            }
        }"#;

        // "*" doesn't go through field validation
        assert!(parse_group_config(json).is_ok());
    }
}
