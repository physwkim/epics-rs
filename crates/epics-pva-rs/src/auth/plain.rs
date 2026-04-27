//! Plain (non-TLS) authentication: "anonymous" or "ca" with user+host AuthZ.
//!
//! The user/host pair is part of the CONNECTION_VALIDATION reply per pvxs
//! `clientconn.cpp::handle_validation`. We default to `$USER`/`hostname()`
//! but callers can override via [`crate::client_native::PvaClientBuilder`]
//! or the `EPICS_PVA_AUTH_USER` / `EPICS_PVA_AUTH_HOST` environment variables.

/// Resolve the AuthZ user. Honours `EPICS_PVA_AUTH_USER`, then `USER`,
/// then `USERNAME`, then falls back to `"anonymous"`.
pub fn authnz_default_user() -> String {
    std::env::var("EPICS_PVA_AUTH_USER")
        .or_else(|_| std::env::var("USER"))
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "anonymous".to_string())
}

/// Resolve the AuthZ host. Honours `EPICS_PVA_AUTH_HOST`, then the OS
/// hostname, falling back to `"localhost"`.
pub fn authnz_default_host() -> String {
    if let Ok(h) = std::env::var("EPICS_PVA_AUTH_HOST") {
        return h;
    }
    hostname::get()
        .ok()
        .and_then(|s| s.into_string().ok())
        .unwrap_or_else(|| "localhost".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_falls_back() {
        // Just check it returns *something*.
        let u = authnz_default_user();
        assert!(!u.is_empty());
    }

    #[test]
    fn host_falls_back() {
        let h = authnz_default_host();
        assert!(!h.is_empty());
    }
}
