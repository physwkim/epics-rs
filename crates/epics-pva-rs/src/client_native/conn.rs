//! TCP virtual circuit between this client and a single PVA server.
//!
//! Encapsulates:
//!
//! - `connect()` — open the socket, complete the handshake, return a
//!   ready-to-use connection
//! - `send()` — write a fully-built application frame
//! - `read_frame()` — read the next application frame, transparently
//!   discarding control messages (SET_BYTE_ORDER, SET_MARKER, ...)
//!
//! The handshake (pvxs `clientconn.cpp` ~line 80):
//!
//! 1. Read a control message — typically `SET_BYTE_ORDER` (cmd=2) — to
//!    pick up the negotiated wire byte order.
//! 2. Read `CONNECTION_VALIDATION` (cmd=1) request: server's buffer
//!    size, registry size, and supported auth methods.
//! 3. Write our own `CONNECTION_VALIDATION` reply: client's buffer
//!    size, registry size, QoS=0, the chosen authnz method ("ca" or
//!    "anonymous"), and (for "ca") a `user`+`host` AuthZ structure.
//! 4. Read `CONNECTION_VALIDATED` (cmd=9): final OK.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::debug;

use crate::error::{PvaError, PvaResult};
use crate::proto::{ByteOrder, Command, ControlCommand, PvaHeader, WriteExt, encode_string_into};

use super::decode::{
    Frame, decode_connection_validated, decode_connection_validation_request, try_parse_frame,
};

/// Maximum reasonable application-frame size we'll buffer (8 MB).
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

/// Default TCP receive buffer hint advertised in CONNECTION_VALIDATION.
pub const DEFAULT_BUFFER_SIZE: u32 = 87_040;
/// Default introspection registry size advertised in CONNECTION_VALIDATION.
pub const DEFAULT_REGISTRY_SIZE: u16 = 32_767;

/// An established server connection ready to carry channel ops.
pub struct Connection {
    stream: TcpStream,
    pub byte_order: ByteOrder,
    pub server_addr: SocketAddr,
    pub server_buffer_size: u32,
    pub server_registry_size: u16,
    pub auth_methods: Vec<String>,
    pub negotiated_auth: String,
    pub user: String,
    pub host: String,
    rx_buf: Vec<u8>,
    timeout: Duration,
}

impl Connection {
    /// Connect, handshake, and return a ready-to-use [`Connection`].
    pub async fn connect(
        target: SocketAddr,
        user: &str,
        host: &str,
        op_timeout: Duration,
    ) -> PvaResult<Self> {
        let stream = timeout(op_timeout, TcpStream::connect(target))
            .await
            .map_err(|_| PvaError::Timeout)?
            .map_err(PvaError::Io)?;
        stream.set_nodelay(true).ok();

        let mut conn = Connection {
            stream,
            byte_order: ByteOrder::Little,
            server_addr: target,
            server_buffer_size: 0,
            server_registry_size: 0,
            auth_methods: Vec::new(),
            negotiated_auth: String::new(),
            user: user.to_string(),
            host: host.to_string(),
            rx_buf: Vec::with_capacity(8192),
            timeout: op_timeout,
        };
        conn.handshake().await?;
        Ok(conn)
    }

    async fn handshake(&mut self) -> PvaResult<()> {
        // Step 1+2: read frames until we get CONNECTION_VALIDATION request.
        loop {
            let frame = self.read_any_frame().await?;
            if frame.header.flags.is_control() {
                if frame.header.command == ControlCommand::SetByteOrder.code() {
                    // Server's preferred wire byte order is encoded in the
                    // header flags of this control message itself.
                    self.byte_order = frame.header.flags.byte_order();
                    debug!(?self.byte_order, "set byte order");
                }
                continue;
            }
            if frame.header.command == Command::ConnectionValidation.code() {
                let req = decode_connection_validation_request(&frame)?;
                self.server_buffer_size = req.server_buffer_size;
                self.server_registry_size = req.server_registry_size;
                self.auth_methods = req.auth_methods.clone();
                // Prefer "ca" (named auth) when offered, else "anonymous".
                self.negotiated_auth = if req.auth_methods.iter().any(|m| m == "ca") {
                    "ca".to_string()
                } else {
                    "anonymous".to_string()
                };
                break;
            }
        }

        // Step 3: send our CONNECTION_VALIDATION reply.
        let reply = build_client_connection_validation(
            self.byte_order,
            DEFAULT_BUFFER_SIZE,
            DEFAULT_REGISTRY_SIZE,
            0,
            &self.negotiated_auth,
            &self.user,
            &self.host,
        );
        timeout(self.timeout, self.stream.write_all(&reply))
            .await
            .map_err(|_| PvaError::Timeout)?
            .map_err(PvaError::Io)?;

        // Step 4: wait for CONNECTION_VALIDATED.
        loop {
            let frame = self.read_any_frame().await?;
            if frame.header.flags.is_control() {
                continue;
            }
            if frame.header.command == Command::ConnectionValidated.code() {
                let st = decode_connection_validated(&frame)?;
                if !st.is_success() {
                    return Err(PvaError::Protocol(format!(
                        "connection validation failed: {:?}",
                        st
                    )));
                }
                return Ok(());
            }
        }
    }

    /// Send a fully-built frame.
    pub async fn send(&mut self, frame_bytes: &[u8]) -> PvaResult<()> {
        timeout(self.timeout, self.stream.write_all(frame_bytes))
            .await
            .map_err(|_| PvaError::Timeout)?
            .map_err(PvaError::Io)?;
        Ok(())
    }

    /// Read the next *application* frame, transparently consuming control
    /// frames in between.
    pub async fn read_app_frame(&mut self) -> PvaResult<Frame> {
        loop {
            let frame = self.read_any_frame().await?;
            if !frame.header.flags.is_control() {
                return Ok(frame);
            }
            // Server may send SET_MARKER / ECHO_REQUEST control msgs — auto-ack.
            if frame.header.command == ControlCommand::EchoRequest.code() {
                let resp = PvaHeader::control(
                    false,
                    self.byte_order,
                    ControlCommand::EchoResponse.code(),
                    frame.header.payload_length,
                );
                let mut out = Vec::with_capacity(8);
                resp.write_into(&mut out);
                self.send(&out).await?;
            }
        }
    }

    /// Read any frame (control or application).
    async fn read_any_frame(&mut self) -> PvaResult<Frame> {
        loop {
            if let Some((frame, n)) = try_parse_frame(&self.rx_buf)? {
                self.rx_buf.drain(..n);
                return Ok(frame);
            }
            // Need more bytes.
            let mut chunk = [0u8; 4096];
            let n = match timeout(self.timeout, self.stream.read(&mut chunk)).await {
                Ok(Ok(n)) => n,
                Ok(Err(e)) => return Err(PvaError::Io(e)),
                Err(_) => return Err(PvaError::Timeout),
            };
            if n == 0 {
                return Err(PvaError::Protocol("connection closed".into()));
            }
            if self.rx_buf.len() + n > MAX_FRAME_BYTES {
                return Err(PvaError::Protocol("frame buffer overflow".into()));
            }
            self.rx_buf.extend_from_slice(&chunk[..n]);
        }
    }
}

/// Build a client-side CONNECTION_VALIDATION (cmd=1) reply per pvxs
/// `clientconn.cpp::handle_validation`.
///
/// Wire layout (after the standard 8-byte PVA header):
///
/// ```text
/// u32 buffer_size
/// u16 introspection_registry_size
/// u16 qos                        (0 for default)
/// String authnz_method           ("ca" or "anonymous")
/// AuthZ payload                  (only for "ca": variant carrying user+host)
/// ```
pub fn build_client_connection_validation(
    order: ByteOrder,
    buffer_size: u32,
    registry_size: u16,
    qos: u16,
    auth: &str,
    user: &str,
    host: &str,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.put_u32(buffer_size, order);
    payload.put_u16(registry_size, order);
    payload.put_u16(qos, order);
    encode_string_into(auth, order, &mut payload);

    if auth == "ca" {
        // Variant tag (0xFD) followed by an inline structure descriptor for a
        // 2-field record { user: string, host: string }, then values.
        payload.put_u8(0xFD);
        payload.put_u16(1, order); // type id (constant 1 — pvData inline-id slot)
        payload.put_u8(0x80); // structure tag
        payload.put_u8(0x00); // empty struct_id
        payload.put_u8(0x02); // 2 fields
        payload.put_u8(0x04);
        payload.extend_from_slice(b"user");
        payload.put_u8(0x60); // string
        payload.put_u8(0x04);
        payload.extend_from_slice(b"host");
        payload.put_u8(0x60); // string
        encode_string_into(user, order, &mut payload);
        encode_string_into(host, order, &mut payload);
    }

    let header = PvaHeader::application(
        false,
        order,
        Command::ConnectionValidation.code(),
        payload.len() as u32,
    );
    let mut out = Vec::with_capacity(PvaHeader::SIZE + payload.len());
    header.write_into(&mut out);
    out.extend_from_slice(&payload);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_reply_layout_smoke() {
        let bytes = build_client_connection_validation(
            ByteOrder::Little,
            87_040,
            32_767,
            0,
            "ca",
            "user",
            "host",
        );
        // Header (8) + payload — at minimum 8+4+2+2+1+1 = 18 (auth empty)
        assert!(bytes.len() > 18);
        assert_eq!(bytes[0], 0xCA);
        assert_eq!(bytes[3], Command::ConnectionValidation.code());
    }

    #[test]
    fn validation_reply_anonymous_no_authnz_block() {
        let with_ca = build_client_connection_validation(
            ByteOrder::Little,
            87_040,
            32_767,
            0,
            "ca",
            "u",
            "h",
        );
        let with_anon = build_client_connection_validation(
            ByteOrder::Little,
            87_040,
            32_767,
            0,
            "anonymous",
            "u",
            "h",
        );
        assert!(with_ca.len() > with_anon.len());
    }
}
