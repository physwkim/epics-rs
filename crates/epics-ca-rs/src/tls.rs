//! TLS configuration for Channel Access over TCP.
//!
//! `epics-ca-rs` extends CA with optional TLS-encrypted TCP virtual
//! circuits — UDP search remains plaintext (PV names are not secret).
//! Enable with the `tls` cargo feature.
//!
//! Two modes:
//!
//! 1. **Server-auth** (TLS) — clients verify the server's certificate
//!    against a root CA. Equivalent to HTTPS without client certs.
//! 2. **mTLS** — both ends present certificates. The server's `ACF`
//!    rule matching uses the client cert's CN/SAN as the identity
//!    instead of the spoofable `CA_PROTO_HOST_NAME` message.
//!
//! Use cases:
//!
//! - Encrypt control traffic across an untrusted LAN segment
//! - Authenticate operators/services without trusting hostnames
//! - Comply with site policies (medical, nuclear, multi-tenant
//!   facilities) that mandate transport encryption
//!
//! See `doc/11-tls-design.md` for the wire-level negotiation,
//! coexistence with plaintext peers, and migration guidance.

#[cfg(feature = "tls")]
use std::io;
#[cfg(feature = "tls")]
use std::path::Path;
#[cfg(feature = "tls")]
use std::sync::Arc;

#[cfg(feature = "tls")]
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
#[cfg(feature = "tls")]
use tokio_rustls::rustls::{ClientConfig, RootCertStore, ServerConfig};

/// CA-over-TLS configuration. Wraps `rustls` ClientConfig/ServerConfig
/// with the conventions used by the CA TLS feature.
#[cfg(feature = "tls")]
#[derive(Clone)]
pub enum TlsConfig {
    /// Server-side TLS configuration. Used by `CaServer::with_tls`.
    Server(Arc<ServerConfig>),
    /// Client-side TLS configuration. Used by `CaClient::with_tls`.
    Client(Arc<ClientConfig>),
}

#[cfg(feature = "tls")]
impl TlsConfig {
    /// Build a server config for TLS-only (no client cert verification).
    /// `cert_chain_pem` should be the server certificate chain (leaf
    /// first), `key_pem` the corresponding private key.
    pub fn server_from_pem(
        cert_chain: Vec<CertificateDer<'static>>,
        key: PrivateKeyDer<'static>,
    ) -> Result<Self, TlsError> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let cfg = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, key)
            .map_err(|e| TlsError::Build(e.to_string()))?;
        Ok(TlsConfig::Server(Arc::new(cfg)))
    }

    /// Build a server config that **requires** a valid client cert
    /// (mTLS). The client's identity (CN or first SAN) becomes the
    /// effective hostname for ACF rule matching, replacing the
    /// `CA_PROTO_HOST_NAME` message.
    pub fn server_mtls_from_pem(
        cert_chain: Vec<CertificateDer<'static>>,
        key: PrivateKeyDer<'static>,
        client_ca_roots: RootCertStore,
    ) -> Result<Self, TlsError> {
        use tokio_rustls::rustls::server::WebPkiClientVerifier;
        let _ = rustls::crypto::ring::default_provider().install_default();
        let verifier = WebPkiClientVerifier::builder(Arc::new(client_ca_roots))
            .build()
            .map_err(|e| TlsError::Build(e.to_string()))?;
        let cfg = ServerConfig::builder()
            .with_client_cert_verifier(verifier)
            .with_single_cert(cert_chain, key)
            .map_err(|e| TlsError::Build(e.to_string()))?;
        Ok(TlsConfig::Server(Arc::new(cfg)))
    }

    /// Build a client config that verifies the server cert against the
    /// supplied roots and presents no client cert (server-auth only).
    pub fn client_from_roots(roots: RootCertStore) -> Self {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let cfg = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        TlsConfig::Client(Arc::new(cfg))
    }

    /// Build a client config that verifies the server cert AND
    /// presents the supplied client cert (mTLS).
    pub fn client_mtls(
        roots: RootCertStore,
        client_cert: Vec<CertificateDer<'static>>,
        client_key: PrivateKeyDer<'static>,
    ) -> Result<Self, TlsError> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let cfg = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_client_auth_cert(client_cert, client_key)
            .map_err(|e| TlsError::Build(e.to_string()))?;
        Ok(TlsConfig::Client(Arc::new(cfg)))
    }
}

/// Helper: load a PEM-encoded certificate chain from a file.
#[cfg(feature = "tls")]
pub fn load_certs(path: impl AsRef<Path>) -> io::Result<Vec<CertificateDer<'static>>> {
    let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
    rustls_pemfile::certs(&mut reader).collect::<io::Result<Vec<_>>>()
}

/// Helper: load a PEM-encoded private key from a file. Tries PKCS#8,
/// PKCS#1 (RSA), and SEC1 (EC) sequentially; returns the first match.
#[cfg(feature = "tls")]
pub fn load_private_key(path: impl AsRef<Path>) -> io::Result<PrivateKeyDer<'static>> {
    let mut reader = std::io::BufReader::new(std::fs::File::open(&path)?);
    if let Some(key) = rustls_pemfile::pkcs8_private_keys(&mut reader)
        .next()
        .transpose()?
    {
        return Ok(PrivateKeyDer::Pkcs8(key));
    }
    let mut reader = std::io::BufReader::new(std::fs::File::open(&path)?);
    if let Some(key) = rustls_pemfile::rsa_private_keys(&mut reader)
        .next()
        .transpose()?
    {
        return Ok(PrivateKeyDer::Pkcs1(key));
    }
    let mut reader = std::io::BufReader::new(std::fs::File::open(&path)?);
    if let Some(key) = rustls_pemfile::ec_private_keys(&mut reader)
        .next()
        .transpose()?
    {
        return Ok(PrivateKeyDer::Sec1(key));
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "no PKCS8/PKCS1/EC private key found in file",
    ))
}

/// Helper: build a `RootCertStore` from a PEM file containing one or
/// more CA certificates.
#[cfg(feature = "tls")]
pub fn load_root_store(path: impl AsRef<Path>) -> io::Result<RootCertStore> {
    let mut store = RootCertStore::empty();
    for cert in load_certs(path)? {
        store
            .add(cert)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    }
    Ok(store)
}

/// Errors returned by TLS configuration helpers.
#[derive(Debug)]
pub enum TlsError {
    Io(std::io::Error),
    Build(String),
}

impl std::fmt::Display for TlsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TlsError::Io(e) => write!(f, "TLS I/O: {e}"),
            TlsError::Build(s) => write!(f, "TLS build: {s}"),
        }
    }
}

impl std::error::Error for TlsError {}

impl From<std::io::Error> for TlsError {
    fn from(e: std::io::Error) -> Self {
        TlsError::Io(e)
    }
}

// Re-exports needed by the public API when the feature is enabled.
#[cfg(feature = "tls")]
pub use rustls_pki_types::CertificateDer as Cert;
#[cfg(feature = "tls")]
pub use rustls_pki_types::PrivateKeyDer as Key;
#[cfg(feature = "tls")]
pub use tokio_rustls::rustls::RootCertStore as Roots;

#[cfg(feature = "tls")]
mod rustls {
    pub use tokio_rustls::rustls::*;
}
