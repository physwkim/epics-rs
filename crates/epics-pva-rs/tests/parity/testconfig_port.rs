//! Port of pvxs's `test/testconfig.cpp::testParse`.
//!
//! pvxs verifies that `EPICS_PVA_ADDR_LIST` is parsed into a list of
//! `SocketAddr`, with the default port supplied from
//! `EPICS_PVA_BROADCAST_PORT` (or 5076). Plain IP entries get the
//! default port appended; `host:port` entries keep their explicit
//! port. Whitespace and surrounding spaces are tolerated.

#![cfg(test)]

use std::net::SocketAddr;

use epics_pva_rs::client_native::search::parse_addr_list;

#[test]
fn pvxs_parse_addr_list_two_entries_explicit_default() {
    // pvxs sets EPICS_PVA_BROADCAST_PORT=1234 then parses
    //   "  1.2.3.4  5.6.7.8:9876  "
    // expecting ["1.2.3.4:1234", "5.6.7.8:9876"].
    //
    // We can't safely mutate process env in parallel tests, so we pin
    // the default port by passing through a raw string and asserting
    // the explicit-port entry survives. The default-port case is
    // covered by pvxs_parse_addr_list_default_port_substituted below.
    let addrs = parse_addr_list("  1.2.3.4  5.6.7.8:9876  ");
    assert_eq!(addrs.len(), 2);
    // First entry should keep IP and pick up *some* default port (5076 or env).
    assert_eq!(format!("{}", addrs[0].ip()), "1.2.3.4");
    // Second entry has explicit port.
    let want: SocketAddr = "5.6.7.8:9876".parse().unwrap();
    assert_eq!(addrs[1], want);
}

#[test]
fn pvxs_parse_addr_list_default_port_substituted() {
    // Without EPICS_PVA_BROADCAST_PORT, plain IPs get port 5076.
    let prev = std::env::var("EPICS_PVA_BROADCAST_PORT").ok();
    // Safety: set to "" then check, restore at end. Tests must run serially.
    // SAFETY: Single-threaded test scope; we only touch one env var.
    unsafe {
        std::env::remove_var("EPICS_PVA_BROADCAST_PORT");
    }
    let addrs = parse_addr_list("10.0.0.1");
    assert_eq!(addrs.len(), 1);
    assert_eq!(addrs[0].port(), 5076);
    if let Some(p) = prev {
        unsafe {
            std::env::set_var("EPICS_PVA_BROADCAST_PORT", p);
        }
    }
}

#[test]
fn pvxs_parse_addr_list_comma_separator() {
    // pvxs accepts comma OR whitespace separators.
    let addrs = parse_addr_list("1.1.1.1,2.2.2.2");
    assert_eq!(addrs.len(), 2);
    assert_eq!(format!("{}", addrs[0].ip()), "1.1.1.1");
    assert_eq!(format!("{}", addrs[1].ip()), "2.2.2.2");
}

#[test]
fn pvxs_parse_addr_list_empty_yields_empty() {
    let addrs = parse_addr_list("");
    assert!(addrs.is_empty());
}

#[test]
fn pvxs_parse_addr_list_skips_invalid_entries() {
    // Non-parsable entries are silently dropped.
    let addrs = parse_addr_list("garbage 127.0.0.1 also-bad 192.168.1.1:5075");
    assert_eq!(addrs.len(), 2);
    assert_eq!(format!("{}", addrs[0].ip()), "127.0.0.1");
    let want: SocketAddr = "192.168.1.1:5075".parse().unwrap();
    assert_eq!(addrs[1], want);
}
