//! Public `PvaClient` facade.

use std::net::SocketAddr;
use std::time::Duration;

use crate::error::{PvaError, PvaResult};
use crate::pvdata::{FieldDesc, PvField};

use super::conn::Connection;
use super::ops::{create_channel, op_get, op_get_field, op_monitor, op_put};
use super::search::{default_server_port, search};

/// Result of a successful GET — value plus introspection used by the
/// formatter.
#[derive(Debug, Clone)]
pub struct PvGetResult {
    pub pv_name: String,
    pub value: PvField,
    pub introspection: FieldDesc,
    pub server_addr: SocketAddr,
}

/// Builder for [`PvaClient`].
pub struct PvaClientBuilder {
    timeout: Duration,
    server_addr: Option<SocketAddr>,
    user: Option<String>,
    host: Option<String>,
}

impl PvaClientBuilder {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            server_addr: None,
            user: None,
            host: None,
        }
    }

    pub fn timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }

    pub fn server_addr(mut self, addr: SocketAddr) -> Self {
        self.server_addr = Some(addr);
        self
    }

    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }

    pub fn build(self) -> PvaClient {
        PvaClient {
            timeout: self.timeout,
            server_addr: self.server_addr,
            user: self.user.unwrap_or_else(default_user),
            host: self.host.unwrap_or_else(default_host),
        }
    }
}

impl Default for PvaClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Pure-Rust pvAccess client (no `spvirit_client` dependency).
#[derive(Clone, Debug)]
pub struct PvaClient {
    timeout: Duration,
    server_addr: Option<SocketAddr>,
    user: String,
    host: String,
}

impl PvaClient {
    pub fn builder() -> PvaClientBuilder {
        PvaClientBuilder::new()
    }

    pub fn new() -> PvaResult<Self> {
        Ok(Self::builder().build())
    }

    /// Construct targeting an explicit `(udp_port, tcp_port)` pair.
    /// Provided for backwards compatibility with the spvirit-shaped API; the
    /// `udp_port` is ignored when `server_addr` is set, but otherwise feeds
    /// into the search subsystem (via env vars, since per-instance UDP port
    /// isn't carried — set `EPICS_PVA_BROADCAST_PORT`).
    pub fn with_ports(_udp_port: u16, tcp_port: u16) -> Self {
        let server_addr =
            SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), tcp_port);
        Self::builder().server_addr(server_addr).build()
    }

    async fn resolve(&self, pv_name: &str) -> PvaResult<SocketAddr> {
        if let Some(addr) = self.server_addr {
            // Override path: skip UDP search.
            return Ok(SocketAddr::new(addr.ip(), addr.port().max(default_server_port())));
        }
        search(pv_name, self.timeout).await
    }

    async fn open(&self, pv_name: &str) -> PvaResult<(Connection, super::ops::ChannelIds)> {
        let target = self.resolve(pv_name).await?;
        let mut conn = Connection::connect(target, &self.user, &self.host, self.timeout).await?;
        let ids = create_channel(&mut conn, pv_name).await?;
        Ok((conn, ids))
    }

    pub async fn pvget(&self, pv_name: &str) -> PvaResult<PvField> {
        let (mut conn, ids) = self.open(pv_name).await?;
        let (_intro, value) = op_get(&mut conn, ids, &[]).await?;
        Ok(value)
    }

    pub async fn pvget_full(&self, pv_name: &str) -> PvaResult<PvGetResult> {
        let (mut conn, ids) = self.open(pv_name).await?;
        let (intro, value) = op_get(&mut conn, ids, &[]).await?;
        Ok(PvGetResult {
            pv_name: pv_name.to_string(),
            value,
            introspection: intro,
            server_addr: conn.server_addr,
        })
    }

    pub async fn pvget_fields(&self, pv_name: &str, fields: &[&str]) -> PvaResult<PvGetResult> {
        let (mut conn, ids) = self.open(pv_name).await?;
        let (intro, value) = op_get(&mut conn, ids, fields).await?;
        Ok(PvGetResult {
            pv_name: pv_name.to_string(),
            value,
            introspection: intro,
            server_addr: conn.server_addr,
        })
    }

    pub async fn pvput(&self, pv_name: &str, value_str: &str) -> PvaResult<()> {
        let (mut conn, ids) = self.open(pv_name).await?;
        op_put(&mut conn, ids, value_str).await
    }

    pub async fn pvmonitor<F>(&self, pv_name: &str, mut callback: F) -> PvaResult<()>
    where
        F: FnMut(&PvField),
    {
        let (mut conn, ids) = self.open(pv_name).await?;
        op_monitor(&mut conn, ids, &[], |_desc, value| callback(value)).await
    }

    pub async fn pvmonitor_typed<F>(&self, pv_name: &str, mut callback: F) -> PvaResult<()>
    where
        F: FnMut(&FieldDesc, &PvField),
    {
        let (mut conn, ids) = self.open(pv_name).await?;
        op_monitor(&mut conn, ids, &[], |desc, value| callback(desc, value)).await
    }

    pub async fn pvinfo(&self, pv_name: &str) -> PvaResult<FieldDesc> {
        let (mut conn, ids) = self.open(pv_name).await?;
        // Try GET_FIELD first; some servers don't support it — fall back to a
        // GET INIT which also returns the descriptor.
        match op_get_field(&mut conn, ids, "").await {
            Ok(d) => Ok(d),
            Err(_) => {
                let (intro, _value) = op_get(&mut conn, ids, &[]).await?;
                Ok(intro)
            }
        }
    }

    pub async fn pvinfo_full(&self, pv_name: &str) -> PvaResult<(FieldDesc, SocketAddr)> {
        let (mut conn, ids) = self.open(pv_name).await?;
        let intro = match op_get_field(&mut conn, ids, "").await {
            Ok(d) => d,
            Err(_) => op_get(&mut conn, ids, &[]).await?.0,
        };
        Ok((intro, conn.server_addr))
    }
}

fn default_user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "anonymous".to_string())
}

fn default_host() -> String {
    hostname::get()
        .ok()
        .and_then(|s| s.into_string().ok())
        .unwrap_or_else(|| "localhost".to_string())
}
