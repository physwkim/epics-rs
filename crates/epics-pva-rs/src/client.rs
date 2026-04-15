use std::ops::ControlFlow;

use spvirit_client::PvGetError;
use spvirit_codec::spvd_decode::{DecodedValue, StructureDesc};

use crate::error::{PvaError, PvaResult};

// ─── Error conversion ────────────────────────────────────────────────────────

fn pva_err(e: PvGetError) -> PvaError {
    match e {
        PvGetError::Io(e) => PvaError::Io(e),
        PvGetError::Timeout(_) => PvaError::Timeout,
        PvGetError::Search(s) => PvaError::ChannelNotFound(s.to_string()),
        PvGetError::Protocol(s) => PvaError::Protocol(s),
        PvGetError::Decode(s) => PvaError::Decode(s),
    }
}

// ─── JSON value conversion for PUT ───────────────────────────────────────────

/// Convert a string value to a serde_json::Value, attempting numeric/boolean
/// parsing first, falling back to a plain string.
fn str_to_json_value(s: &str) -> serde_json::Value {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
        return v;
    }
    serde_json::Value::String(s.to_string())
}

/// pvAccess client — delegates to [`spvirit_client::PvaClient`].
pub struct PvaClient {
    inner: spvirit_client::PvaClient,
}

impl PvaClient {
    pub fn new() -> PvaResult<Self> {
        let udp_port = epics_base_rs::runtime::net::pva_broadcast_port();
        let tcp_port = epics_base_rs::runtime::net::pva_server_port();
        Ok(Self {
            inner: spvirit_client::PvaClient::builder()
                .udp_port(udp_port)
                .port(tcp_port)
                .build(),
        })
    }

    /// Create a client targeting specific ports (useful for testing).
    pub fn with_ports(udp_port: u16, tcp_port: u16) -> Self {
        Self {
            inner: spvirit_client::PvaClient::builder()
                .udp_port(udp_port)
                .port(tcp_port)
                .build(),
        }
    }

    // ─── pvget ──────────────────────────────────────────────────────────

    pub async fn pvget(&self, pv_name: &str) -> PvaResult<DecodedValue> {
        let result = self.inner.pvget(pv_name).await.map_err(pva_err)?;
        Ok(result.value)
    }

    /// Get full result including introspection (needed for typed formatting).
    pub async fn pvget_full(&self, pv_name: &str) -> PvaResult<spvirit_client::PvGetResult> {
        self.inner.pvget(pv_name).await.map_err(pva_err)
    }

    /// Get with field filtering (equivalent to `pvget -r "field(value,alarm)"`).
    pub async fn pvget_fields(
        &self,
        pv_name: &str,
        fields: &[&str],
    ) -> PvaResult<spvirit_client::PvGetResult> {
        self.inner
            .pvget_fields(pv_name, fields)
            .await
            .map_err(pva_err)
    }

    // ─── pvput ──────────────────────────────────────────────────────────

    pub async fn pvput(&self, pv_name: &str, value_str: &str) -> PvaResult<()> {
        let json_val = str_to_json_value(value_str);
        self.inner.pvput(pv_name, json_val).await.map_err(pva_err)
    }

    // ─── pvmonitor ──────────────────────────────────────────────────────

    pub async fn pvmonitor<F>(&self, pv_name: &str, mut callback: F) -> PvaResult<()>
    where
        F: FnMut(&DecodedValue),
    {
        self.inner
            .pvmonitor(pv_name, |val| {
                callback(val);
                ControlFlow::Continue(())
            })
            .await
            .map_err(pva_err)
    }

    // ─── pvinfo ─────────────────────────────────────────────────────────

    /// Get type descriptor via GET_FIELD, falling back to pvget introspection
    /// if the server doesn't support GET_FIELD.
    pub async fn pvinfo(&self, pv_name: &str) -> PvaResult<StructureDesc> {
        match self.inner.pvinfo(pv_name).await {
            Ok(desc) => Ok(desc),
            Err(_) => {
                // Fallback: use pvget introspection
                let result = self.inner.pvget(pv_name).await.map_err(pva_err)?;
                Ok(result.introspection)
            }
        }
    }

    /// Get type descriptor and server address.
    pub async fn pvinfo_full(
        &self,
        pv_name: &str,
    ) -> PvaResult<(StructureDesc, std::net::SocketAddr)> {
        match self.inner.pvinfo_full(pv_name).await {
            Ok(result) => Ok(result),
            Err(_) => {
                // Fallback: use pvget introspection (no server addr available)
                let result = self.inner.pvget(pv_name).await.map_err(pva_err)?;
                // Use a placeholder — pvget doesn't expose server_addr yet
                let addr = "0.0.0.0:0".parse().unwrap();
                Ok((result.introspection, addr))
            }
        }
    }
}
