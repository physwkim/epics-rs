//! `.pvlist` configuration file parser.
//!
//! Corresponds to C++ `gateAs::readPvList` (`gateAs.cc`).
//!
//! ## File format
//!
//! ```text
//! EVALUATION ORDER ALLOW, DENY
//!
//! # comments start with #
//! Beam:.*          ALLOW Beam 1
//! PS.*             ALLOW PowerSupply 1
//! ps([0-9])        ALIAS PSCurrent\1.ai PowerSupply 1
//! test.*           DENY
//! ```
//!
//! Each non-comment line is one of:
//! - `EVALUATION ORDER ALLOW, DENY` (or `DENY, ALLOW`) — sets match order
//! - `pattern ALLOW [asg [asl]]` — allow access, optional access security group/level
//! - `pattern DENY [FROM host1 host2 ...]` — deny access (optional host list)
//! - `pattern ALIAS target [asg [asl]]` — alias to a different upstream PV.
//!   Target may contain backreferences `\0`–`\9` to capture groups.
//!
//! ## Notes
//!
//! - Patterns are full regex (Rust `regex` crate). C++ uses POSIX regex
//!   or PCRE optionally — most simple patterns are compatible.
//! - Backreference substitution is implemented manually because Rust
//!   `regex` doesn't support backreferences in the pattern, but
//!   capture groups are available for replacement.
//! - The DENY `FROM host` clause is parsed but not enforced in the
//!   skeleton phase (host filtering is a future addition).

use std::path::Path;

use regex::Regex;

use crate::error::{BridgeError, BridgeResult};

/// How to combine ALLOW and DENY rules.
///
/// `AllowDeny` (default): match ALLOW rules first; if any matches, DENY rules
/// can override. `DenyAllow`: match DENY rules first; if any matches, ALLOW
/// rules can override.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EvaluationOrder {
    /// `EVALUATION ORDER ALLOW, DENY` (default)
    #[default]
    AllowDeny,
    /// `EVALUATION ORDER DENY, ALLOW`
    DenyAllow,
}

/// One rule in a `.pvlist` file.
#[derive(Debug, Clone)]
pub enum PvListEntry {
    /// `pattern ALLOW [asg [asl]]`
    Allow {
        pattern: Regex,
        asg: Option<String>,
        asl: Option<i32>,
    },
    /// `pattern DENY [FROM host ...]`
    Deny {
        pattern: Regex,
        from_hosts: Vec<String>,
    },
    /// `pattern ALIAS target [asg [asl]]`
    Alias {
        pattern: Regex,
        target_template: String,
        asg: Option<String>,
        asl: Option<i32>,
    },
}

impl PvListEntry {
    fn pattern(&self) -> &Regex {
        match self {
            Self::Allow { pattern, .. } => pattern,
            Self::Deny { pattern, .. } => pattern,
            Self::Alias { pattern, .. } => pattern,
        }
    }

    fn is_allow(&self) -> bool {
        matches!(self, Self::Allow { .. } | Self::Alias { .. })
    }

    fn is_deny(&self) -> bool {
        matches!(self, Self::Deny { .. })
    }
}

/// Result of matching a PV name against the rule list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PvListMatch {
    /// Resolved upstream PV name (after alias substitution if applicable).
    /// Equal to the input name unless an `ALIAS` rule matched.
    pub resolved_name: String,
    /// Access security group (from rule), if specified.
    pub asg: Option<String>,
    /// Access security level (from rule), if specified.
    pub asl: Option<i32>,
    /// Whether this came from an Alias rule.
    pub is_alias: bool,
}

/// A parsed `.pvlist` file.
#[derive(Debug)]
pub struct PvList {
    pub order: EvaluationOrder,
    pub entries: Vec<PvListEntry>,
}

impl PvList {
    pub fn new() -> Self {
        Self {
            order: EvaluationOrder::default(),
            entries: Vec::new(),
        }
    }

    /// Match a PV name against the rule list.
    ///
    /// Returns `Some(PvListMatch)` if the name should be served (allowed,
    /// possibly via alias), or `None` if the name is denied.
    pub fn match_name(&self, name: &str) -> Option<PvListMatch> {
        // Find first matching ALLOW (or ALIAS) and first matching DENY
        let allow_match = self
            .entries
            .iter()
            .find(|e| e.is_allow() && e.pattern().is_match(name));
        let deny_match = self
            .entries
            .iter()
            .find(|e| e.is_deny() && e.pattern().is_match(name));

        let allow_decision: Option<PvListMatch> = allow_match.map(|e| match e {
            PvListEntry::Allow { asg, asl, .. } => PvListMatch {
                resolved_name: name.to_string(),
                asg: asg.clone(),
                asl: *asl,
                is_alias: false,
            },
            PvListEntry::Alias {
                pattern,
                target_template,
                asg,
                asl,
            } => {
                let resolved = expand_template(pattern, name, target_template);
                PvListMatch {
                    resolved_name: resolved,
                    asg: asg.clone(),
                    asl: *asl,
                    is_alias: true,
                }
            }
            _ => unreachable!(),
        });

        match self.order {
            EvaluationOrder::AllowDeny => {
                // ALLOW first, DENY can override
                if deny_match.is_some() {
                    None
                } else {
                    allow_decision
                }
            }
            EvaluationOrder::DenyAllow => {
                // DENY first, ALLOW can override.
                // - allow rule matches → grant (overrides any DENY)
                // - allow rule misses → deny (whether or not a DENY rule matched)
                allow_decision
            }
        }
    }
}

impl Default for PvList {
    fn default() -> Self {
        Self::new()
    }
}

/// Expand `\0`–`\9` backreferences in a template using regex captures.
///
/// `\0` refers to the entire match. `\1`–`\9` refer to capture groups.
fn expand_template(pattern: &Regex, input: &str, template: &str) -> String {
    let captures = match pattern.captures(input) {
        Some(c) => c,
        None => return template.to_string(),
    };

    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            let c = bytes[i + 1];
            if c.is_ascii_digit() {
                let group_idx = (c - b'0') as usize;
                if let Some(g) = captures.get(group_idx) {
                    out.push_str(g.as_str());
                }
                i += 2;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Parse a `.pvlist` file from string content.
pub fn parse_pvlist(content: &str) -> BridgeResult<PvList> {
    let mut list = PvList::new();

    for (lineno, raw) in content.lines().enumerate() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        // EVALUATION ORDER directive
        if let Some(rest) = line.strip_prefix("EVALUATION ORDER") {
            let rest = rest.trim();
            if rest.eq_ignore_ascii_case("ALLOW, DENY") || rest.eq_ignore_ascii_case("ALLOW,DENY") {
                list.order = EvaluationOrder::AllowDeny;
            } else if rest.eq_ignore_ascii_case("DENY, ALLOW")
                || rest.eq_ignore_ascii_case("DENY,ALLOW")
            {
                list.order = EvaluationOrder::DenyAllow;
            } else {
                return Err(BridgeError::GroupConfigError(format!(
                    "line {}: invalid EVALUATION ORDER '{}'",
                    lineno + 1,
                    rest
                )));
            }
            continue;
        }

        // Pattern rule: pattern KEYWORD [args...]
        let entry = parse_rule_line(line, lineno + 1)?;
        list.entries.push(entry);
    }

    Ok(list)
}

/// Parse a `.pvlist` file from disk.
pub fn parse_pvlist_file(path: &Path) -> BridgeResult<PvList> {
    let content = std::fs::read_to_string(path)?;
    parse_pvlist(&content)
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(i) => &line[..i],
        None => line,
    }
}

fn parse_rule_line(line: &str, lineno: usize) -> BridgeResult<PvListEntry> {
    let mut tokens = line.split_whitespace();

    let pattern_str = tokens
        .next()
        .ok_or_else(|| BridgeError::GroupConfigError(format!("line {lineno}: missing pattern")))?;
    let keyword = tokens
        .next()
        .ok_or_else(|| BridgeError::GroupConfigError(format!("line {lineno}: missing keyword")))?;

    let pattern = build_pattern(pattern_str, lineno)?;

    match keyword.to_ascii_uppercase().as_str() {
        "ALLOW" => {
            let asg = tokens.next().map(String::from);
            let asl = tokens
                .next()
                .map(|s| {
                    s.parse::<i32>().map_err(|e| {
                        BridgeError::GroupConfigError(format!(
                            "line {lineno}: invalid asl '{s}': {e}"
                        ))
                    })
                })
                .transpose()?;
            Ok(PvListEntry::Allow { pattern, asg, asl })
        }
        "DENY" => {
            // Optional FROM host1 host2 ...
            let mut from_hosts = Vec::new();
            if let Some(t) = tokens.next() {
                if t.eq_ignore_ascii_case("FROM") {
                    for h in tokens {
                        from_hosts.push(h.to_string());
                    }
                } else {
                    return Err(BridgeError::GroupConfigError(format!(
                        "line {lineno}: expected FROM after DENY, got '{t}'"
                    )));
                }
            }
            Ok(PvListEntry::Deny {
                pattern,
                from_hosts,
            })
        }
        "ALIAS" => {
            let target = tokens.next().ok_or_else(|| {
                BridgeError::GroupConfigError(format!(
                    "line {lineno}: ALIAS requires a target name"
                ))
            })?;
            let asg = tokens.next().map(String::from);
            let asl = tokens
                .next()
                .map(|s| {
                    s.parse::<i32>().map_err(|e| {
                        BridgeError::GroupConfigError(format!(
                            "line {lineno}: invalid asl '{s}': {e}"
                        ))
                    })
                })
                .transpose()?;
            Ok(PvListEntry::Alias {
                pattern,
                target_template: target.to_string(),
                asg,
                asl,
            })
        }
        other => Err(BridgeError::GroupConfigError(format!(
            "line {lineno}: unknown keyword '{other}', expected ALLOW/DENY/ALIAS"
        ))),
    }
}

fn build_pattern(pat: &str, lineno: usize) -> BridgeResult<Regex> {
    // Anchor the pattern to match the full PV name (C++ ca-gateway behavior).
    let anchored = format!("^{pat}$");
    Regex::new(&anchored).map_err(|e| {
        BridgeError::GroupConfigError(format!("line {lineno}: invalid regex '{pat}': {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty() {
        let list = parse_pvlist("").unwrap();
        assert_eq!(list.order, EvaluationOrder::AllowDeny);
        assert!(list.entries.is_empty());
    }

    #[test]
    fn parse_comments_and_blanks() {
        let content = r#"
            # This is a comment

            # Another one

        "#;
        let list = parse_pvlist(content).unwrap();
        assert!(list.entries.is_empty());
    }

    #[test]
    fn parse_evaluation_order() {
        let list = parse_pvlist("EVALUATION ORDER DENY, ALLOW").unwrap();
        assert_eq!(list.order, EvaluationOrder::DenyAllow);

        let list = parse_pvlist("EVALUATION ORDER ALLOW, DENY").unwrap();
        assert_eq!(list.order, EvaluationOrder::AllowDeny);
    }

    #[test]
    fn parse_simple_allow() {
        let list = parse_pvlist("Beam:.* ALLOW").unwrap();
        assert_eq!(list.entries.len(), 1);
        assert!(matches!(list.entries[0], PvListEntry::Allow { .. }));
    }

    #[test]
    fn parse_allow_with_asg_asl() {
        let list = parse_pvlist("Beam:.* ALLOW BeamGroup 2").unwrap();
        if let PvListEntry::Allow { asg, asl, .. } = &list.entries[0] {
            assert_eq!(asg.as_deref(), Some("BeamGroup"));
            assert_eq!(*asl, Some(2));
        } else {
            panic!("expected Allow");
        }
    }

    #[test]
    fn parse_deny() {
        let list = parse_pvlist("test.* DENY").unwrap();
        assert!(matches!(list.entries[0], PvListEntry::Deny { .. }));
    }

    #[test]
    fn parse_deny_from_hosts() {
        let list = parse_pvlist("test.* DENY FROM bad.host evil.host").unwrap();
        if let PvListEntry::Deny { from_hosts, .. } = &list.entries[0] {
            assert_eq!(from_hosts, &["bad.host", "evil.host"]);
        } else {
            panic!("expected Deny");
        }
    }

    #[test]
    fn parse_alias() {
        let list = parse_pvlist(r"ps([0-9]) ALIAS PSCurrent\1.ai PSGroup 1").unwrap();
        if let PvListEntry::Alias {
            target_template,
            asg,
            asl,
            ..
        } = &list.entries[0]
        {
            assert_eq!(target_template, r"PSCurrent\1.ai");
            assert_eq!(asg.as_deref(), Some("PSGroup"));
            assert_eq!(*asl, Some(1));
        } else {
            panic!("expected Alias");
        }
    }

    #[test]
    fn parse_full_example() {
        let content = r#"
            EVALUATION ORDER ALLOW, DENY

            # Beam line PVs
            Beam:.*       ALLOW BeamGroup 1

            # Power supplies via alias
            ps([0-9])     ALIAS PSCurrent\1.ai PSGroup 1

            # Block test PVs
            test.*        DENY
        "#;
        let list = parse_pvlist(content).unwrap();
        assert_eq!(list.entries.len(), 3);
    }

    #[test]
    fn parse_invalid_keyword() {
        assert!(parse_pvlist("foo BAD").is_err());
    }

    #[test]
    fn parse_invalid_regex() {
        assert!(parse_pvlist("[invalid ALLOW").is_err());
    }

    #[test]
    fn parse_alias_missing_target() {
        assert!(parse_pvlist("foo ALIAS").is_err());
    }

    #[test]
    fn match_simple_allow() {
        let list = parse_pvlist("Beam:.* ALLOW").unwrap();
        let m = list.match_name("Beam:current").unwrap();
        assert_eq!(m.resolved_name, "Beam:current");
        assert!(!m.is_alias);

        assert!(list.match_name("Other:pv").is_none());
    }

    #[test]
    fn match_deny_overrides_allow() {
        // ALLOW, DENY order: DENY overrides
        let list = parse_pvlist(
            r#"
                EVALUATION ORDER ALLOW, DENY
                .*  ALLOW
                bad.* DENY
            "#,
        )
        .unwrap();
        assert!(list.match_name("good:pv").is_some());
        assert!(list.match_name("bad:pv").is_none());
    }

    #[test]
    fn match_allow_overrides_deny() {
        // DENY, ALLOW order: ALLOW overrides
        let list = parse_pvlist(
            r#"
                EVALUATION ORDER DENY, ALLOW
                .*    DENY
                Beam:.* ALLOW
            "#,
        )
        .unwrap();
        assert!(list.match_name("Beam:current").is_some());
        assert!(list.match_name("Other:pv").is_none());
    }

    #[test]
    fn match_alias_with_backreference() {
        let list = parse_pvlist(r"ps([0-9]) ALIAS PSCurrent\1.ai PSGroup 1").unwrap();
        let m = list.match_name("ps3").unwrap();
        assert!(m.is_alias);
        assert_eq!(m.resolved_name, "PSCurrent3.ai");
        assert_eq!(m.asg.as_deref(), Some("PSGroup"));
        assert_eq!(m.asl, Some(1));
    }

    #[test]
    fn match_alias_multiple_groups() {
        let list = parse_pvlist(r"(\w+):(\d+) ALIAS \1_record\2.VAL").unwrap();
        let m = list.match_name("temp:7").unwrap();
        assert_eq!(m.resolved_name, "temp_record7.VAL");
    }

    #[test]
    fn pattern_anchored() {
        // Pattern is implicitly anchored — partial matches should fail
        let list = parse_pvlist("foo ALLOW").unwrap();
        assert!(list.match_name("foo").is_some());
        assert!(list.match_name("foobar").is_none());
        assert!(list.match_name("xfoo").is_none());
    }

    #[test]
    fn expand_template_zero_group() {
        let pat = Regex::new(r"^(\w+)$").unwrap();
        // \0 is the whole match
        let result = expand_template(&pat, "hello", r"prefix_\0_suffix");
        assert_eq!(result, "prefix_hello_suffix");
    }
}
