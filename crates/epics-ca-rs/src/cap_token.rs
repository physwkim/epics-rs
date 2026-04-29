//! Ed25519-signed capability tokens for CA identity.
//!
//! ACF was designed around UNIX hostnames and usernames — the model
//! breaks for clients behind NAT, in Kubernetes, or otherwise unable
//! to present a stable host identity. This module provides an
//! orthogonal identity layer: a small signed JSON token the client
//! ships in CLIENT_NAME, the server verifies, and the resolved
//! `subject` becomes the username for ACF matching.
//!
//! Token shape (the payload):
//!
//! ```json
//! {"sub":"alice","groups":["BEAM"],"exp":1714200000,"iss":"ops-1"}
//! ```
//!
//! Encoded form: `cap:<base64url(payload)>.<base64url(signature)>`
//!
//! - signature is Ed25519 over the base64url-encoded payload bytes
//! - issuer key is identified by `iss` and looked up in the
//!   verifier's keyring
//! - `exp` is unix seconds; tokens past expiry are rejected
//! - `groups` are surfaced through ACF UAG matching as
//!   `cap-token-group:<NAME>` virtual entries
//!
//! The format is intentionally not JWT — JWT carries a lot of
//! historical baggage we don't need. Custom but compact suits the
//! single-protocol scope here.

#![cfg(feature = "cap-tokens")]

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenClaims {
    /// Subject — the identifier ACF matches as the username.
    pub sub: String,
    /// Optional group memberships; surfaced through ACF UAG matching.
    #[serde(default)]
    pub groups: Vec<String>,
    /// Expiration in unix seconds.
    pub exp: u64,
    /// Issuer key id — looked up in the verifier's keyring.
    pub iss: String,
}

impl TokenClaims {
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.exp <= now
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("missing 'cap:' prefix")]
    MissingPrefix,
    #[error("malformed token (no '.' separator)")]
    Malformed,
    #[error("base64 decode failed: {0}")]
    Base64(String),
    #[error("json decode failed: {0}")]
    Json(String),
    #[error("unknown issuer key id: {0}")]
    UnknownIssuer(String),
    #[error("invalid signature")]
    BadSignature,
    #[error("token expired")]
    Expired,
    #[error("token revoked")]
    Revoked,
}

/// Issues tokens. One signing key per server / role.
pub struct TokenIssuer {
    iss_id: String,
    key: SigningKey,
}

impl TokenIssuer {
    pub fn new(iss_id: impl Into<String>, key: SigningKey) -> Self {
        Self {
            iss_id: iss_id.into(),
            key,
        }
    }

    /// Generate a fresh signing keypair. Caller is responsible for
    /// persistence.
    pub fn generate(iss_id: impl Into<String>) -> Self {
        use rand_core::OsRng;
        let mut csprng = OsRng;
        Self::new(iss_id, SigningKey::generate(&mut csprng))
    }

    pub fn issue(&self, sub: &str, groups: &[String], ttl_secs: u64) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let claims = TokenClaims {
            sub: sub.to_string(),
            groups: groups.to_vec(),
            exp: now + ttl_secs,
            iss: self.iss_id.clone(),
        };
        let payload = serde_json::to_vec(&claims).expect("TokenClaims serializes infallibly");
        let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&payload);
        let sig: Signature = self.key.sign(payload_b64.as_bytes());
        let sig_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sig.to_bytes());
        format!("cap:{payload_b64}.{sig_b64}")
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.key.verifying_key()
    }
}

/// Verifies tokens against a keyring of trusted issuers. Tracks a
/// revocation list of `(iss, sub)` pairs so an operator can blacklist
/// a stolen / leaked subject without rotating the issuer's key.
#[derive(Default)]
pub struct TokenVerifier {
    keys: HashMap<String, VerifyingKey>,
    /// Revocation entries keyed by `(iss, sub)`. Stateless tokens
    /// can't be "expired early" any other way; we accept the storage
    /// cost (small — typically a handful of entries).
    revoked: std::collections::HashSet<(String, String)>,
}

impl TokenVerifier {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn trust(&mut self, iss_id: impl Into<String>, key: VerifyingKey) {
        self.keys.insert(iss_id.into(), key);
    }

    /// Block a subject issued by a specific issuer. Future verifies
    /// of any token with the same `(iss, sub)` pair return
    /// [`TokenError::Revoked`]. Idempotent.
    pub fn revoke(&mut self, iss_id: impl Into<String>, sub: impl Into<String>) {
        self.revoked.insert((iss_id.into(), sub.into()));
    }

    /// Lift a previous revoke. Idempotent.
    pub fn unrevoke(&mut self, iss_id: &str, sub: &str) {
        self.revoked.remove(&(iss_id.to_string(), sub.to_string()));
    }

    /// Snapshot of the trusted keys for export / publishing
    /// (e.g. via a `/keys` introspection endpoint). Returned as
    /// `(issuer_id, public_key_bytes)` pairs.
    pub fn export_keys(&self) -> Vec<(String, [u8; 32])> {
        self.keys
            .iter()
            .map(|(iss, vk)| (iss.clone(), vk.to_bytes()))
            .collect()
    }

    pub fn verify(&self, token: &str) -> Result<TokenClaims, TokenError> {
        let body = token
            .strip_prefix("cap:")
            .ok_or(TokenError::MissingPrefix)?;
        let (payload_b64, sig_b64) = body.split_once('.').ok_or(TokenError::Malformed)?;
        let sig_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(sig_b64)
            .map_err(|e| TokenError::Base64(e.to_string()))?;
        if sig_bytes.len() != 64 {
            return Err(TokenError::BadSignature);
        }
        let mut sig_arr = [0u8; 64];
        sig_arr.copy_from_slice(&sig_bytes);
        let signature = Signature::from_bytes(&sig_arr);
        let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload_b64)
            .map_err(|e| TokenError::Base64(e.to_string()))?;
        // G5: bound the JSON parse before verification. JSON parse
        // runs against attacker-controlled bytes; while we still
        // need claims.iss to look up the verification key (so we
        // can't fully verify-first), capping the payload length
        // prevents pathological deeply-nested JSON from burning CPU.
        // Real CA cap-tokens are <1 KiB; 4 KiB gives generous headroom.
        const MAX_PAYLOAD_BYTES: usize = 4096;
        if payload_bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(TokenError::Malformed);
        }
        let claims: TokenClaims =
            serde_json::from_slice(&payload_bytes).map_err(|e| TokenError::Json(e.to_string()))?;
        let key = self
            .keys
            .get(&claims.iss)
            .ok_or_else(|| TokenError::UnknownIssuer(claims.iss.clone()))?;
        key.verify(payload_b64.as_bytes(), &signature)
            .map_err(|_| TokenError::BadSignature)?;
        if claims.is_expired() {
            return Err(TokenError::Expired);
        }
        if self
            .revoked
            .contains(&(claims.iss.clone(), claims.sub.clone()))
        {
            return Err(TokenError::Revoked);
        }
        Ok(claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_valid() {
        let issuer = TokenIssuer::generate("ops-1");
        let mut verifier = TokenVerifier::new();
        verifier.trust("ops-1", issuer.verifying_key());
        let tok = issuer.issue("alice", &["BEAM".into(), "DIAG".into()], 3600);
        let claims = verifier.verify(&tok).expect("valid token");
        assert_eq!(claims.sub, "alice");
        assert_eq!(claims.groups, vec!["BEAM", "DIAG"]);
        assert_eq!(claims.iss, "ops-1");
    }

    #[test]
    fn rejects_unknown_issuer() {
        let issuer = TokenIssuer::generate("ops-1");
        let verifier = TokenVerifier::new();
        let tok = issuer.issue("alice", &[], 3600);
        let err = verifier.verify(&tok).unwrap_err();
        matches!(err, TokenError::UnknownIssuer(_))
            .then_some(())
            .expect("expected UnknownIssuer");
    }

    #[test]
    fn rejects_tampered_payload() {
        let issuer = TokenIssuer::generate("ops-1");
        let mut verifier = TokenVerifier::new();
        verifier.trust("ops-1", issuer.verifying_key());
        let tok = issuer.issue("alice", &[], 3600);
        // Flip a byte in the payload portion.
        let body = tok.strip_prefix("cap:").unwrap();
        let (p, s) = body.split_once('.').unwrap();
        let mut p_bytes = p.as_bytes().to_vec();
        p_bytes[0] ^= 0xFF;
        let tampered = format!("cap:{}.{s}", String::from_utf8_lossy(&p_bytes));
        assert!(verifier.verify(&tampered).is_err());
    }

    #[test]
    fn rejects_revoked_then_unrevoke_works() {
        let issuer = TokenIssuer::generate("ops-1");
        let mut verifier = TokenVerifier::new();
        verifier.trust("ops-1", issuer.verifying_key());
        let tok = issuer.issue("alice", &[], 3600);
        assert!(verifier.verify(&tok).is_ok());
        verifier.revoke("ops-1", "alice");
        let err = verifier.verify(&tok).unwrap_err();
        matches!(err, TokenError::Revoked)
            .then_some(())
            .expect("expected Revoked");
        // Lifting the revocation re-allows the token.
        verifier.unrevoke("ops-1", "alice");
        assert!(verifier.verify(&tok).is_ok());
    }

    #[test]
    fn export_keys_reflects_keyring() {
        let issuer = TokenIssuer::generate("ops-1");
        let mut verifier = TokenVerifier::new();
        verifier.trust("ops-1", issuer.verifying_key());
        let exported = verifier.export_keys();
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].0, "ops-1");
        assert_eq!(exported[0].1, issuer.verifying_key().to_bytes());
    }

    #[test]
    fn rejects_expired() {
        let issuer = TokenIssuer::generate("ops-1");
        let mut verifier = TokenVerifier::new();
        verifier.trust("ops-1", issuer.verifying_key());
        let tok = issuer.issue("alice", &[], 0); // exp = now
        std::thread::sleep(std::time::Duration::from_secs(1));
        let err = verifier.verify(&tok).unwrap_err();
        matches!(err, TokenError::Expired)
            .then_some(())
            .expect("expected Expired");
    }
}
