//! Tower-style middleware for the PVA gateway.
//!
//! Wraps the underlying [`epics_pva_rs::server_native::ChannelSource`]
//! ([`super::source::GatewayChannelSource`] in practice) with
//! composable [`Layer`]s that add cross-cutting concerns:
//!
//! - [`AclLayer`] — refuse `has_pv` / `put_value` for PV names that
//!   match the deny list
//! - [`ReadOnlyLayer`] — fail every PUT, even if upstream allows
//!   it. Mirrors the existing `read_only` flag but as a composable
//!   layer so operators can stack it with audit / ACL.
//! - [`AuditLayer`] — emit a structured event for every PUT,
//!   reusing the existing audit pipeline shape
//!
//! Design constraint: the inner [`ChannelSource`] is the gateway's
//! own [`super::source::GatewayChannelSource`], and we want the
//! `Layer` chain to short-circuit BEFORE the call reaches it (so an
//! ACL deny doesn't trigger an upstream search). Each [`Layer`]
//! implementation forwards calls verbatim by default; override the
//! method to insert pre/post hooks.

use std::sync::Arc;

use epics_pva_rs::pvdata::{FieldDesc, PvField};
use epics_pva_rs::server_native::ChannelContext;
use epics_pva_rs::server_native::source::ChannelSource;
use tokio::sync::mpsc;

/// Wrap a [`ChannelSource`] and produce a new one with extra
/// behaviour. Implementations override only the methods they need;
/// the default forwards every call unchanged.
pub trait Layer<S: ChannelSource>: Send + Sync + 'static {
    type Wrapped: ChannelSource;
    fn layer(self, inner: S) -> Self::Wrapped;
}

// ── ReadOnlyLayer ────────────────────────────────────────────────

/// Reject every PUT regardless of upstream policy. Composable
/// with audit / ACL — stacking `ReadOnlyLayer` last in the chain
/// guarantees no PUT can reach the underlying source even if a
/// later layer would have allowed it.
pub struct ReadOnlyLayer;

pub struct ReadOnly<S> {
    inner: Arc<S>,
}

impl<S: ChannelSource> Layer<S> for ReadOnlyLayer {
    type Wrapped = ReadOnly<S>;
    fn layer(self, inner: S) -> ReadOnly<S> {
        ReadOnly {
            inner: Arc::new(inner),
        }
    }
}

impl<S: ChannelSource> ChannelSource for ReadOnly<S> {
    async fn list_pvs(&self) -> Vec<String> {
        self.inner.list_pvs().await
    }
    async fn has_pv(&self, name: &str) -> bool {
        self.inner.has_pv(name).await
    }
    async fn get_introspection(&self, name: &str) -> Option<FieldDesc> {
        self.inner.get_introspection(name).await
    }
    async fn get_value(&self, name: &str) -> Option<PvField> {
        self.inner.get_value(name).await
    }
    async fn put_value(&self, _name: &str, _value: PvField) -> Result<(), String> {
        Err("read-only mode: PUT rejected".into())
    }
    async fn put_value_ctx(
        &self,
        _name: &str,
        _value: PvField,
        _ctx: ChannelContext,
    ) -> Result<(), String> {
        Err("read-only mode: PUT rejected".into())
    }
    async fn is_writable(&self, _name: &str) -> bool {
        false
    }
    async fn subscribe(&self, name: &str) -> Option<mpsc::Receiver<PvField>> {
        self.inner.subscribe(name).await
    }
}

// ── AclLayer ─────────────────────────────────────────────────────

/// Pattern-matched access control. PV names matching any entry in
/// `deny` are rejected at the layer before reaching the upstream
/// proxy. `allow_only` (when non-empty) flips the policy — only
/// names matching one of these patterns get through.
///
/// Patterns are simple glob-style — `*` matches any chars, exact
/// match otherwise. For full regex see the per-rule `pvlist` system
/// in the CA gateway; here we keep the surface minimal.
#[derive(Clone, Default)]
pub struct AclConfig {
    pub deny: Vec<String>,
    pub allow_only: Vec<String>,
}

impl AclConfig {
    pub fn allowed(&self, name: &str) -> bool {
        if self.deny.iter().any(|p| matches_pattern(p, name)) {
            return false;
        }
        if !self.allow_only.is_empty()
            && !self
                .allow_only
                .iter()
                .any(|p| matches_pattern(p, name))
        {
            return false;
        }
        true
    }
}

fn matches_pattern(pattern: &str, name: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        return name.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return name.ends_with(suffix);
    }
    name == pattern
}

pub struct AclLayer {
    config: AclConfig,
}

impl AclLayer {
    pub fn new(config: AclConfig) -> Self {
        Self { config }
    }
}

pub struct Acl<S> {
    inner: Arc<S>,
    config: AclConfig,
}

impl<S: ChannelSource> Layer<S> for AclLayer {
    type Wrapped = Acl<S>;
    fn layer(self, inner: S) -> Acl<S> {
        Acl {
            inner: Arc::new(inner),
            config: self.config,
        }
    }
}

impl<S: ChannelSource> ChannelSource for Acl<S> {
    async fn list_pvs(&self) -> Vec<String> {
        // Filter the underlying list so introspection sweeps don't
        // leak the names of denied PVs.
        let mut names = self.inner.list_pvs().await;
        names.retain(|n| self.config.allowed(n));
        names
    }
    async fn has_pv(&self, name: &str) -> bool {
        if !self.config.allowed(name) {
            return false;
        }
        self.inner.has_pv(name).await
    }
    async fn get_introspection(&self, name: &str) -> Option<FieldDesc> {
        if !self.config.allowed(name) {
            return None;
        }
        self.inner.get_introspection(name).await
    }
    async fn get_value(&self, name: &str) -> Option<PvField> {
        if !self.config.allowed(name) {
            return None;
        }
        self.inner.get_value(name).await
    }
    async fn put_value(&self, name: &str, value: PvField) -> Result<(), String> {
        if !self.config.allowed(name) {
            return Err(format!("ACL: PV '{name}' denied"));
        }
        self.inner.put_value(name, value).await
    }
    async fn put_value_ctx(
        &self,
        name: &str,
        value: PvField,
        ctx: ChannelContext,
    ) -> Result<(), String> {
        if !self.config.allowed(name) {
            return Err(format!("ACL: PV '{name}' denied"));
        }
        self.inner.put_value_ctx(name, value, ctx).await
    }
    async fn is_writable(&self, name: &str) -> bool {
        self.config.allowed(name) && self.inner.is_writable(name).await
    }
    async fn subscribe(&self, name: &str) -> Option<mpsc::Receiver<PvField>> {
        if !self.config.allowed(name) {
            return None;
        }
        self.inner.subscribe(name).await
    }
}

// ── AuditLayer ───────────────────────────────────────────────────

/// Hook fired on every PUT. Ops typically wire this into a file
/// sink (line-based JSON, libca-asLib text, etc.) — the layer
/// stays format-agnostic.
pub trait AuditSink: Send + Sync + 'static {
    fn record(&self, event: AuditEvent);
}

/// Default no-op sink — useful when wiring a layer chain in tests.
pub struct NoopAudit;

impl AuditSink for NoopAudit {
    fn record(&self, _event: AuditEvent) {}
}

/// Boxed-closure audit sink — convenient for inline tests +
/// custom integrations without a dedicated trait impl.
pub struct ClosureAudit<F: Fn(AuditEvent) + Send + Sync + 'static>(pub F);

impl<F: Fn(AuditEvent) + Send + Sync + 'static> AuditSink for ClosureAudit<F> {
    fn record(&self, event: AuditEvent) {
        (self.0)(event);
    }
}

#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub pv: String,
    pub event: AuditEventKind,
    pub user: String,
    pub host: String,
    pub result: AuditResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditEventKind {
    Put,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditResult {
    Ok,
    Denied,
    Failed,
}

pub struct AuditLayer<A: AuditSink> {
    sink: Arc<A>,
}

impl<A: AuditSink> AuditLayer<A> {
    pub fn new(sink: A) -> Self {
        Self {
            sink: Arc::new(sink),
        }
    }
}

pub struct Audited<S, A> {
    inner: Arc<S>,
    sink: Arc<A>,
}

impl<S: ChannelSource, A: AuditSink> Layer<S> for AuditLayer<A> {
    type Wrapped = Audited<S, A>;
    fn layer(self, inner: S) -> Audited<S, A> {
        Audited {
            inner: Arc::new(inner),
            sink: self.sink,
        }
    }
}

impl<S: ChannelSource, A: AuditSink> ChannelSource for Audited<S, A> {
    async fn list_pvs(&self) -> Vec<String> {
        self.inner.list_pvs().await
    }
    async fn has_pv(&self, name: &str) -> bool {
        self.inner.has_pv(name).await
    }
    async fn get_introspection(&self, name: &str) -> Option<FieldDesc> {
        self.inner.get_introspection(name).await
    }
    async fn get_value(&self, name: &str) -> Option<PvField> {
        self.inner.get_value(name).await
    }
    async fn put_value(&self, name: &str, value: PvField) -> Result<(), String> {
        let result = self.inner.put_value(name, value).await;
        self.sink.record(AuditEvent {
            pv: name.to_string(),
            event: AuditEventKind::Put,
            user: String::new(),
            host: String::new(),
            result: match &result {
                Ok(_) => AuditResult::Ok,
                Err(_) => AuditResult::Failed,
            },
        });
        result
    }
    async fn put_value_ctx(
        &self,
        name: &str,
        value: PvField,
        ctx: ChannelContext,
    ) -> Result<(), String> {
        let user = ctx.account.clone();
        let host = ctx.host.clone();
        let result = self.inner.put_value_ctx(name, value, ctx).await;
        self.sink.record(AuditEvent {
            pv: name.to_string(),
            event: AuditEventKind::Put,
            user,
            host,
            result: match &result {
                Ok(_) => AuditResult::Ok,
                Err(_) => AuditResult::Failed,
            },
        });
        result
    }
    async fn is_writable(&self, name: &str) -> bool {
        self.inner.is_writable(name).await
    }
    async fn subscribe(&self, name: &str) -> Option<mpsc::Receiver<PvField>> {
        self.inner.subscribe(name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_matching() {
        assert!(matches_pattern("MOTOR:*", "MOTOR:VAL"));
        assert!(matches_pattern("*VAL", "MOTOR:VAL"));
        assert!(matches_pattern("EXACT", "EXACT"));
        assert!(!matches_pattern("EXACT", "EXACT2"));
        assert!(!matches_pattern("MOTOR:*", "OTHER:VAL"));
    }

    #[test]
    fn acl_allow_only() {
        let cfg = AclConfig {
            allow_only: vec!["BL10C:*".into()],
            ..Default::default()
        };
        assert!(cfg.allowed("BL10C:VG-01:PRESSURE"));
        assert!(!cfg.allowed("RFP:HV"));
    }

    #[test]
    fn acl_deny_overrides_allow() {
        let cfg = AclConfig {
            allow_only: vec!["MOTOR:*".into()],
            deny: vec!["MOTOR:JOG:*".into()],
        };
        assert!(cfg.allowed("MOTOR:VAL"));
        assert!(!cfg.allowed("MOTOR:JOG:UP"));
        assert!(!cfg.allowed("OTHER:PV"));
    }
}
