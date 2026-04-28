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

/// Enumerate the current process's POSIX group names. Mirrors pvxs
/// `osdGetRoles()` (osgroups.cpp:54). On non-POSIX targets returns an
/// empty list. The `ca` auth method advertises these as the user's
/// "roles" claim so server-side ACF rules of the form
/// `R member group:engineers` can match against the actual group
/// membership of the requesting user.
///
/// Calls `getgrouplist(3)` directly via `nix` — same data libca
/// gathers on the C side. Result is sorted + deduped for stable
/// matching.
pub fn posix_groups() -> Vec<String> {
    #[cfg(unix)]
    {
        use std::ffi::CStr;
        // Step 1: get this process's effective uid + login name.
        // SAFETY: getuid is always safe; geteuid returns the
        // current effective uid.
        let uid = unsafe { libc::getuid() };
        // Look up login name via getpwuid_r (re-entrant). Falls
        // back to `$USER` env when the lookup misses.
        let name = unsafe {
            let pw = libc::getpwuid(uid);
            if pw.is_null() {
                None
            } else {
                let cs = CStr::from_ptr((*pw).pw_name);
                cs.to_str().ok().map(|s| s.to_string())
            }
        };
        let user = name
            .or_else(|| std::env::var("USER").ok())
            .unwrap_or_default();
        if user.is_empty() {
            return Vec::new();
        }
        // Step 2: getgrouplist(3) — first call probes the size.
        // SAFETY: We pass cstr-terminated `user` and a writable
        // groups slice big enough for the kernel's reply.
        let user_cstr = match std::ffi::CString::new(user) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        // Darwin's libc binding uses `*mut c_int` for the groups
        // pointer; Linux uses `*mut gid_t` (u32). Use a raw c_int
        // buffer and cast on the way out so the signature matches
        // both platforms.
        let primary = unsafe { libc::getgid() } as libc::c_int;
        let mut ngroups: libc::c_int = 64;
        let mut groups: Vec<libc::c_int> = vec![0; ngroups as usize];
        let rc = unsafe {
            libc::getgrouplist(
                user_cstr.as_ptr(),
                primary as _,
                groups.as_mut_ptr() as *mut _,
                &mut ngroups,
            )
        };
        if rc < 0 {
            groups.resize(ngroups as usize, 0);
            let rc2 = unsafe {
                libc::getgrouplist(
                    user_cstr.as_ptr(),
                    primary as _,
                    groups.as_mut_ptr() as *mut _,
                    &mut ngroups,
                )
            };
            if rc2 < 0 {
                return Vec::new();
            }
        }
        groups.truncate(ngroups as usize);
        // Step 3: gid → group name via getgrgid.
        let mut names: Vec<String> = Vec::with_capacity(groups.len());
        for gid in groups {
            unsafe {
                let gr = libc::getgrgid(gid as libc::gid_t);
                if gr.is_null() {
                    continue;
                }
                let cs = CStr::from_ptr((*gr).gr_name);
                if let Ok(s) = cs.to_str() {
                    names.push(s.to_string());
                }
            }
        }
        names.sort();
        names.dedup();
        names
    }
    #[cfg(not(unix))]
    {
        Vec::new()
    }
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
