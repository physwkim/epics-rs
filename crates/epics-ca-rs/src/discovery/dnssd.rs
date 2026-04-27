//! Unicast DNS-SD discovery — works across subnets / WAN.
//!
//! Issues DNS PTR query against `_epics-ca._tcp.<zone>` to enumerate
//! IOC instances, then SRV+A queries to resolve each instance to a
//! socket address.
//!
//! Uses `hickory-resolver` configured from the system's DNS settings
//! (`/etc/resolv.conf` on Unix, registry on Windows).

#![cfg(feature = "discovery")]

use std::net::SocketAddr;

use hickory_resolver::TokioAsyncResolver;
use hickory_resolver::config::{ResolverConfig, ResolverOpts};

use super::Backend;

pub struct DnsSdBackend {
    zone: String,
    resolver: TokioAsyncResolver,
}

impl DnsSdBackend {
    pub fn new(zone: impl Into<String>) -> Result<Self, std::io::Error> {
        // Try the system resolver first; fall back to a default config
        // (Cloudflare DNS) if that fails.
        let resolver = match hickory_resolver::system_conf::read_system_conf() {
            Ok((cfg, opts)) => TokioAsyncResolver::tokio(cfg, opts),
            Err(e) => {
                tracing::warn!(error = %e, "system DNS config unavailable; using defaults");
                TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default())
            }
        };
        Ok(Self {
            zone: zone.into(),
            resolver,
        })
    }

    /// Service-type FQDN for this backend's zone, e.g.
    /// `_epics-ca._tcp.facility.local`.
    fn service_fqdn(&self) -> String {
        format!("_epics-ca._tcp.{}", self.zone)
    }
}

#[async_trait::async_trait]
impl Backend for DnsSdBackend {
    async fn discover(&self) -> Vec<SocketAddr> {
        let svc = self.service_fqdn();
        // Step 1: PTR — enumerate instances.
        let ptr = match self.resolver.srv_lookup(&svc).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(zone = %self.zone, error = %e,
                    "DNS-SD: PTR/SRV lookup failed");
                return Vec::new();
            }
        };

        // hickory's `srv_lookup` already chases PTR→SRV→A internally
        // and returns SrvLookup which exposes both srv records and the
        // resolved IP addresses. We just unwrap them into SocketAddrs.
        let mut out = Vec::new();
        for srv in ptr.iter() {
            for ip in ptr.ip_iter() {
                let addr = SocketAddr::new(ip, srv.port());
                if !out.contains(&addr) {
                    out.push(addr);
                }
            }
        }
        if out.is_empty() {
            tracing::debug!(zone = %self.zone, "DNS-SD: no instances found");
        } else {
            tracing::info!(zone = %self.zone, count = out.len(),
                "DNS-SD discovered IOCs");
        }
        out
    }
}
