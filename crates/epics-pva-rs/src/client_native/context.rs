//! Public `PvaClient` facade.
//!
//! Built on top of:
//!
//! - [`super::search_engine::SearchEngine`] — single background task,
//!   handles SEARCH retry backoff + beacon listening.
//! - [`super::channel::ConnectionPool`] — shared `ServerConn` per server
//!   address, with full handshake + heartbeat + auto-shutdown.
//! - [`super::channel::Channel`] — per-PV state machine (Searching →
//!   Connecting → Active → Reconnecting). Multiple ops share a single
//!   channel instance.
//! - [`super::ops_v2`] — GET / PUT / MONITOR / RPC; MONITOR transparently
//!   re-issues INIT + START on every reconnect.
//!
//! Public API stays compatible with the previous shape so existing callers
//! (pvget-rs, pvput-rs, pvmonitor-rs, pvinfo-rs) keep working.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use parking_lot::RwLock;

use crate::error::{PvaError, PvaResult};
use crate::pvdata::{FieldDesc, PvField};

use super::channel::{Channel, ConnectionPool};
use super::ops_v2::{op_get, op_monitor, op_put, op_rpc, DEFAULT_PIPELINE_SIZE};
use super::search_engine::SearchEngine;

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
    pipeline_size: u32,
}

impl PvaClientBuilder {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            server_addr: None,
            user: None,
            host: None,
            pipeline_size: DEFAULT_PIPELINE_SIZE,
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

    /// Override the monitor pipeline size (default 4 — one ack per 4 events).
    /// Set to 0 to disable pipelining.
    pub fn pipeline_size(mut self, n: u32) -> Self {
        self.pipeline_size = n;
        self
    }

    pub fn build(self) -> PvaClient {
        PvaClient {
            inner: Arc::new(ClientInner {
                timeout: self.timeout,
                server_addr: self.server_addr,
                user: self.user.unwrap_or_else(super::super::auth::authnz_default_user),
                host: self.host.unwrap_or_else(super::super::auth::authnz_default_host),
                pipeline_size: self.pipeline_size,
                pool: ConnectionPool::new(),
                channels: RwLock::new(HashMap::new()),
                search: OnceLock::new(),
            }),
        }
    }
}

impl Default for PvaClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

struct ClientInner {
    timeout: Duration,
    server_addr: Option<SocketAddr>,
    user: String,
    host: String,
    pipeline_size: u32,
    pool: Arc<ConnectionPool>,
    channels: RwLock<HashMap<String, Arc<Channel>>>,
    /// Lazy: only spawn the search engine when we actually need to resolve.
    search: OnceLock<SearchEngine>,
}

#[derive(Clone)]
pub struct PvaClient {
    inner: Arc<ClientInner>,
}

impl std::fmt::Debug for PvaClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PvaClient")
            .field("timeout", &self.inner.timeout)
            .field("user", &self.inner.user)
            .field("host", &self.inner.host)
            .finish()
    }
}

impl PvaClient {
    pub fn builder() -> PvaClientBuilder {
        PvaClientBuilder::new()
    }

    pub fn new() -> PvaResult<Self> {
        Ok(Self::builder().build())
    }

    /// Backwards-compatible: targets a specific TCP port (UDP ignored —
    /// search uses the standard port machinery).
    pub fn with_ports(_udp_port: u16, tcp_port: u16) -> Self {
        let server_addr =
            SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), tcp_port);
        Self::builder().server_addr(server_addr).build()
    }

    async fn search_engine(&self) -> PvaResult<&SearchEngine> {
        if self.inner.search.get().is_none() {
            let engine = SearchEngine::spawn(Vec::new()).await?;
            let _ = self.inner.search.set(engine);
        }
        Ok(self.inner.search.get().unwrap())
    }

    async fn channel(&self, pv_name: &str) -> PvaResult<Arc<Channel>> {
        if let Some(c) = self.inner.channels.read().get(pv_name).cloned() {
            return Ok(c);
        }

        let ch = if let Some(direct) = self.inner.server_addr {
            // Direct-server mode: no UDP search at all. Channel will go
            // straight to Connecting → Active using `direct`.
            Arc::new(Channel::new_direct(
                pv_name.to_string(),
                self.inner.user.clone(),
                self.inner.host.clone(),
                self.inner.timeout,
                self.inner.pool.clone(),
                direct,
            ))
        } else {
            let search = self.search_engine().await?.clone();
            Arc::new(Channel::new(
                pv_name.to_string(),
                self.inner.user.clone(),
                self.inner.host.clone(),
                self.inner.timeout,
                self.inner.pool.clone(),
                search,
            ))
        };

        let mut map = self.inner.channels.write();
        if let Some(existing) = map.get(pv_name).cloned() {
            return Ok(existing);
        }
        map.insert(pv_name.to_string(), ch.clone());
        Ok(ch)
    }

    pub async fn pvget(&self, pv_name: &str) -> PvaResult<PvField> {
        let ch = self.channel(pv_name).await?;
        let (_, v) = op_get(&ch, &[], self.inner.timeout).await?;
        Ok(v)
    }

    pub async fn pvget_full(&self, pv_name: &str) -> PvaResult<PvGetResult> {
        let ch = self.channel(pv_name).await?;
        let (intro, value) = op_get(&ch, &[], self.inner.timeout).await?;
        let server_addr = match ch.current_state() {
            super::channel::ChannelState::Active { server, .. } => server.addr,
            _ => SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 0),
        };
        Ok(PvGetResult {
            pv_name: pv_name.to_string(),
            value,
            introspection: intro,
            server_addr,
        })
    }

    pub async fn pvget_fields(&self, pv_name: &str, fields: &[&str]) -> PvaResult<PvGetResult> {
        let ch = self.channel(pv_name).await?;
        let (intro, value) = op_get(&ch, fields, self.inner.timeout).await?;
        let server_addr = match ch.current_state() {
            super::channel::ChannelState::Active { server, .. } => server.addr,
            _ => SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 0),
        };
        Ok(PvGetResult {
            pv_name: pv_name.to_string(),
            value,
            introspection: intro,
            server_addr,
        })
    }

    pub async fn pvput(&self, pv_name: &str, value_str: &str) -> PvaResult<()> {
        let ch = self.channel(pv_name).await?;
        op_put(&ch, value_str, self.inner.timeout).await
    }

    pub async fn pvmonitor<F>(&self, pv_name: &str, mut callback: F) -> PvaResult<()>
    where
        F: FnMut(&PvField) + Send,
    {
        let ch = self.channel(pv_name).await?;
        op_monitor(&ch, &[], self.inner.pipeline_size, move |_desc, value| {
            callback(value)
        })
        .await
    }

    pub async fn pvmonitor_typed<F>(&self, pv_name: &str, callback: F) -> PvaResult<()>
    where
        F: FnMut(&FieldDesc, &PvField) + Send,
    {
        let ch = self.channel(pv_name).await?;
        op_monitor(&ch, &[], self.inner.pipeline_size, callback).await
    }

    pub async fn pvinfo(&self, pv_name: &str) -> PvaResult<FieldDesc> {
        let ch = self.channel(pv_name).await?;
        let (intro, _value) = op_get(&ch, &[], self.inner.timeout).await?;
        Ok(intro)
    }

    pub async fn pvinfo_full(&self, pv_name: &str) -> PvaResult<(FieldDesc, SocketAddr)> {
        let ch = self.channel(pv_name).await?;
        let (intro, _value) = op_get(&ch, &[], self.inner.timeout).await?;
        let server_addr = match ch.current_state() {
            super::channel::ChannelState::Active { server, .. } => server.addr,
            _ => SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 0),
        };
        Ok((intro, server_addr))
    }

    pub async fn pvrpc(
        &self,
        pv_name: &str,
        request_desc: &FieldDesc,
        request_value: &PvField,
    ) -> PvaResult<(FieldDesc, PvField)> {
        let ch = self.channel(pv_name).await?;
        op_rpc(&ch, request_desc, request_value, self.inner.timeout).await
    }
}
