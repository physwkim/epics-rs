//! Parser for `@pva://...` link strings.
//!
//! Accepted forms (matches pvxs `pvalink_jlif.cpp`):
//!
//! ```text
//! pva://PV:NAME                              — bare PV name, default options
//! pva://PV:NAME?field=value                  — explicit value field
//! pva://PV:NAME?proc=PASSIVE&monitor=true    — multiple options
//! pva://PV:NAME pp                           — legacy "process passive" suffix
//! ```
//!
//! INP vs OUT direction is determined by the record field, not the link
//! string itself; callers pass [`LinkDirection`] when constructing a link.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkDirection {
    /// Record reads from the remote PV (INP-style).
    Inp,
    /// Record writes to the remote PV (OUT-style).
    Out,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PvaLinkConfig {
    pub pv_name: String,
    /// Sub-field selector inside the remote NT structure (default `"value"`).
    pub field: String,
    /// True iff the link should keep an active monitor open instead of
    /// re-reading on each access (INP only).
    pub monitor: bool,
    /// True iff PUT should call `process()` on the remote record (OUT only).
    pub process: bool,
    /// True iff the link reports DBE_VALUE notifications back to the local
    /// record (INP, monitor mode).
    pub notify: bool,
    /// Direction inferred from caller, not parsed.
    pub direction: LinkDirection,
}

#[derive(Debug, thiserror::Error)]
pub enum PvaLinkParseError {
    #[error("not a pva link: {0:?}")]
    NotPvaLink(String),
    #[error("empty PV name")]
    EmptyPv,
    #[error("invalid option: {0:?}")]
    BadOption(String),
}

impl PvaLinkConfig {
    /// Parse a link string into a config. The caller passes the direction
    /// explicitly — INP for record input fields, OUT for outputs.
    pub fn parse(s: &str, direction: LinkDirection) -> Result<Self, PvaLinkParseError> {
        // Strip leading `@` if present (DBD parsers strip this; tests may not).
        let s = s.trim();
        let s = s.strip_prefix('@').unwrap_or(s);
        // pvxs accepts both `pva://` and bare `pva:` prefixes.
        let body = s
            .strip_prefix("pva://")
            .or_else(|| s.strip_prefix("pva:"))
            .ok_or_else(|| PvaLinkParseError::NotPvaLink(s.to_string()))?;

        // Strip legacy "PP" / "NPP" / "MS" / "NMS" suffixes (DBD-style mods).
        let (body, legacy_mods) = strip_legacy_mods(body);

        // Split off ?key=value&key=value
        let (pv_name, opts) = match body.split_once('?') {
            Some((pv, q)) => (pv, parse_query(q)?),
            None => (body, HashMap::new()),
        };
        if pv_name.is_empty() {
            return Err(PvaLinkParseError::EmptyPv);
        }

        let mut cfg = PvaLinkConfig {
            pv_name: pv_name.to_string(),
            field: "value".to_string(),
            monitor: false,
            process: false,
            notify: false,
            direction,
        };

        if let Some(v) = opts.get("field") {
            cfg.field = v.clone();
        }
        if let Some(v) = opts.get("monitor") {
            cfg.monitor = parse_bool(v)?;
        }
        if let Some(v) = opts.get("proc") {
            cfg.process = matches!(v.as_str(), "TRUE" | "true" | "1" | "PASSIVE" | "passive");
        }
        if let Some(v) = opts.get("notify") {
            cfg.notify = parse_bool(v)?;
        }

        // Apply legacy bare modifiers
        for m in legacy_mods {
            match m.as_str() {
                "PP" | "pp" => cfg.process = true,
                "NPP" | "npp" => cfg.process = false,
                "MS" | "ms" | "NMS" | "nms" => {
                    // Maximize-severity flags don't affect the PVA path; ignore.
                }
                _ => {}
            }
        }

        Ok(cfg)
    }
}

fn strip_legacy_mods(body: &str) -> (&str, Vec<String>) {
    // Legacy DBD links can have whitespace-separated trailing tokens like
    // "PV:NAME PP MS". Detect and split those off.
    let mut parts: Vec<&str> = body.split_whitespace().collect();
    if parts.len() <= 1 {
        return (body, Vec::new());
    }
    let head = parts.remove(0);
    let mods: Vec<String> = parts.into_iter().map(|s| s.to_string()).collect();
    (head, mods)
}

fn parse_query(q: &str) -> Result<HashMap<String, String>, PvaLinkParseError> {
    let mut out = HashMap::new();
    for chunk in q.split('&').filter(|s| !s.is_empty()) {
        let (k, v) = chunk
            .split_once('=')
            .ok_or_else(|| PvaLinkParseError::BadOption(chunk.to_string()))?;
        out.insert(k.to_string(), v.to_string());
    }
    Ok(out)
}

fn parse_bool(v: &str) -> Result<bool, PvaLinkParseError> {
    match v {
        "true" | "TRUE" | "1" | "yes" | "YES" => Ok(true),
        "false" | "FALSE" | "0" | "no" | "NO" => Ok(false),
        other => Err(PvaLinkParseError::BadOption(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_pv_name() {
        let c = PvaLinkConfig::parse("pva://OTHER:PV", LinkDirection::Inp).unwrap();
        assert_eq!(c.pv_name, "OTHER:PV");
        assert_eq!(c.field, "value");
        assert!(!c.monitor);
        assert!(!c.process);
    }

    #[test]
    fn at_prefix_accepted() {
        let c = PvaLinkConfig::parse("@pva://X", LinkDirection::Out).unwrap();
        assert_eq!(c.pv_name, "X");
    }

    #[test]
    fn query_options() {
        let c = PvaLinkConfig::parse(
            "pva://A?field=alarm.severity&monitor=true&proc=PASSIVE",
            LinkDirection::Inp,
        )
        .unwrap();
        assert_eq!(c.field, "alarm.severity");
        assert!(c.monitor);
        assert!(c.process);
    }

    #[test]
    fn legacy_pp_modifier() {
        let c = PvaLinkConfig::parse("pva://X PP", LinkDirection::Out).unwrap();
        assert_eq!(c.pv_name, "X");
        assert!(c.process);
    }

    #[test]
    fn empty_pv_rejected() {
        assert!(matches!(
            PvaLinkConfig::parse("pva://", LinkDirection::Inp),
            Err(PvaLinkParseError::EmptyPv)
        ));
    }

    #[test]
    fn non_pva_rejected() {
        assert!(matches!(
            PvaLinkConfig::parse("ca://X", LinkDirection::Inp),
            Err(PvaLinkParseError::NotPvaLink(_))
        ));
    }
}
