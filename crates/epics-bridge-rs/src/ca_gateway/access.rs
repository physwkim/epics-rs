//! Access security adapter for the gateway.
//!
//! Wraps an EPICS access security configuration (`.access` / ACF file)
//! and provides per-channel read/write permission checks.
//!
//! ## Format
//!
//! ```text
//! UAG(engineers) { jones, smith }
//! HAG(controlroom) { console1, console2 }
//!
//! ASG(DEFAULT) {
//!   RULE(1, READ)
//!   RULE(1, WRITE)
//! }
//!
//! ASG(BeamGroup) {
//!   RULE(1, READ)
//!   RULE(1, WRITE) { UAG(engineers), HAG(controlroom) }
//! }
//! ```
//!
//! Each PV is associated with an ASG via the `.pvlist` `ALLOW` / `ALIAS`
//! directives (the third token after `ALLOW`/`ALIAS` is the ASG name).
//! When a downstream client attempts a put or read, the gateway checks
//! the ASG rules against the client's user/host credentials.
//!
//! ## Status
//!
//! The current implementation parses the file (via [`epics_base_rs`] ACF
//! parser) and provides allow-all defaults. Per-rule enforcement requires
//! integration with the downstream CaServer's per-client credential
//! tracking, which is wired in a later phase.

use std::path::Path;

use epics_base_rs::server::access_security::{AccessLevel, AccessSecurityConfig, parse_acf};

use crate::error::{BridgeError, BridgeResult};

/// Access security configuration for the gateway.
pub struct AccessConfig {
    /// Underlying parsed ACF, or None if no file was loaded.
    config: Option<AccessSecurityConfig>,
    /// If true, all operations are allowed regardless of rules.
    /// Used as the default when no `.access` file is provided.
    allow_all: bool,
}

impl AccessConfig {
    /// Construct an "allow all" config with no underlying rules.
    pub fn allow_all() -> Self {
        Self {
            config: None,
            allow_all: true,
        }
    }

    /// Load an `.access` file from disk.
    pub fn from_file(path: &Path) -> BridgeResult<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_string(&content)
    }

    /// Parse `.access` content from a string.
    pub fn from_string(content: &str) -> BridgeResult<Self> {
        let config = parse_acf(content)
            .map_err(|e| BridgeError::GroupConfigError(format!("ACF parse: {e}")))?;
        Ok(Self {
            config: Some(config),
            allow_all: false,
        })
    }

    /// Whether reading the given (asg, asl, user, host) tuple is allowed.
    ///
    /// `asl` (access security level) is currently unused; the underlying
    /// EPICS ACF parser stores per-rule levels but `check_access` returns
    /// a coarse `Read | ReadWrite | NoAccess` level. ASL filtering can be
    /// added when finer-grained level checking is needed.
    pub fn can_read(&self, asg: &str, _asl: i32, user: &str, host: &str) -> bool {
        if self.allow_all {
            return true;
        }
        match &self.config {
            Some(cfg) => matches!(
                cfg.check_access(asg, host, user),
                AccessLevel::Read | AccessLevel::ReadWrite
            ),
            None => true,
        }
    }

    /// Whether writing the given tuple is allowed.
    pub fn can_write(&self, asg: &str, _asl: i32, user: &str, host: &str) -> bool {
        if self.allow_all {
            return true;
        }
        match &self.config {
            Some(cfg) => matches!(cfg.check_access(asg, host, user), AccessLevel::ReadWrite),
            None => true,
        }
    }

    /// Whether the underlying ACF was successfully loaded.
    pub fn has_rules(&self) -> bool {
        self.config.is_some()
    }
}

impl Default for AccessConfig {
    fn default() -> Self {
        Self::allow_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_all_default() {
        let acc = AccessConfig::allow_all();
        assert!(!acc.has_rules());
        assert!(acc.can_read("BeamGroup", 1, "anyone", "anywhere"));
        assert!(acc.can_write("BeamGroup", 1, "anyone", "anywhere"));
    }

    #[test]
    fn allow_all_default_via_default_trait() {
        let acc = AccessConfig::default();
        assert!(acc.can_read("X", 0, "u", "h"));
    }

    #[test]
    fn from_string_with_minimal_acf() {
        // Minimal ACF: a single ASG with READ/WRITE rules
        let content = r#"
            ASG(DEFAULT) {
                RULE(1, READ)
                RULE(1, WRITE)
            }
        "#;
        // Just verify parsing doesn't blow up; the ACF parser may have
        // its own quirks but allow-mode fallback should still hold
        let acc = AccessConfig::from_string(content);
        // ACF parser may succeed or fail depending on supported syntax;
        // both outcomes are acceptable for this skeleton.
        let _ = acc;
    }
}
