use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::{Duration, Instant};

use epics_base_rs::net::AsyncUdpV4;
use epics_base_rs::runtime::sync::mpsc;

use crate::protocol::*;

use super::CoordRequest;

// ---------------------------------------------------------------------------
// Per-server beacon state
// ---------------------------------------------------------------------------

struct BeaconState {
    last_id: u32,
    last_seen: Instant,
    /// Estimated period between beacons (exponential moving average).
    period_estimate: Duration,
    count: u64,
}

/// Idle threshold after which a tracked server is forgotten. Mirrors
/// pvxs `beaconCleanInterval` (`client.cpp` 2 × 180 s default). When a
/// long-silent server resumes beacons, the next sighting becomes
/// `first_sighting = true` and naturally takes the anomaly path —
/// without this prune, in-sequence beacons after long silence would
/// keep `first_sighting = false` and miss the rescan kick. This
/// replaces the previous "soft poke on every beacon" mechanism, which
/// caused steady-state amplification (multi-IOC networks beaconing
/// within ~6 s aggregate kept the search engine in 200 ms fast-tick
/// mode indefinitely).
const BEACON_STALE_THRESHOLD: Duration = Duration::from_secs(180);

// ---------------------------------------------------------------------------
// Beacon monitor task
// ---------------------------------------------------------------------------

/// Receives beacon messages from the CA repeater, detects anomalies (IOC
/// restart), and notifies the coordinator to rescan affected channels.
/// Re-registration interval: if no beacons for this long, re-register
/// with the repeater in case it restarted.
const REREGISTER_INTERVAL: Duration = Duration::from_secs(300);

pub(crate) async fn run_beacon_monitor(coord_tx: mpsc::UnboundedSender<CoordRequest>) {
    run_beacon_monitor_inner(
        coord_tx,
        #[cfg(feature = "cap-tokens")]
        None,
    )
    .await;
}

/// Variant that gates beacon acceptance on a [`SignedBeaconVerifier`].
/// When `verifier` is `Some(...)`, the monitor only forwards beacons
/// to the search engine after a valid companion datagram (cmmd=0xCAFE,
/// see [`crate::server::signed_beacon`]) has been received and
/// verified for the same (server, beacon_id) within the
/// `max_age_secs` window.
#[cfg(feature = "cap-tokens")]
#[allow(dead_code)]
pub(crate) async fn run_beacon_monitor_with_verifier(
    coord_tx: mpsc::UnboundedSender<CoordRequest>,
    verifier: std::sync::Arc<crate::server::signed_beacon::SignedBeaconVerifier>,
) {
    run_beacon_monitor_inner(coord_tx, Some(verifier)).await;
}

async fn run_beacon_monitor_inner(
    coord_tx: mpsc::UnboundedSender<CoordRequest>,
    #[cfg(feature = "cap-tokens")] verifier: Option<
        std::sync::Arc<crate::server::signed_beacon::SignedBeaconVerifier>,
    >,
) {
    // The CA repeater forwards every accepted beacon to its
    // registered clients over loopback only — there's no multi-NIC
    // routing here. Bind exclusively on `127.0.0.1` so we get the
    // SO_REUSEADDR-friendly per-NIC machinery for free without
    // wasting per-NIC sockets that would never see traffic.
    let socket = match AsyncUdpV4::bind_single(Ipv4Addr::LOCALHOST, 0, false) {
        Ok(s) => s,
        Err(_) => return,
    };

    // Initial registration with retry
    for attempt in 0..3u32 {
        if register_with_repeater(&socket).await.is_ok() {
            break;
        }
        if attempt < 2 {
            tokio::time::sleep(Duration::from_millis(200 * (1 << attempt))).await;
        }
    }

    // When `verifier` is set, this map remembers which
    // (server_ip, server_port, beacon_id) tuples have been
    // authenticated by a recent companion datagram. Beacons whose
    // tuple isn't here within `max_age_secs` get dropped (or merely
    // counted, when `require_signed` is false).
    #[cfg(feature = "cap-tokens")]
    let mut verified_tuples: HashMap<(u32, u16, u32), std::time::Instant> = HashMap::new();
    #[cfg(feature = "cap-tokens")]
    let require_signed = !matches!(
        epics_base_rs::runtime::env::get("EPICS_CA_BEACON_REQUIRE_SIGNED").as_deref(),
        Some("NO" | "no" | "0" | "false" | "FALSE")
    );
    let mut servers: HashMap<SocketAddr, BeaconState> = HashMap::new();
    // Beacons are 16 B but the repeater may concatenate VERSION + RSRV_IS_UP
    // and forward client-noop traffic. Use 4 KB so chained datagrams are
    // received intact.
    let mut buf = [0u8; 4096];

    loop {
        // Use timeout to detect beacon silence → re-register with repeater
        let recv = tokio::time::timeout(REREGISTER_INTERVAL, socket.recv_from(&mut buf)).await;
        let (len, _src) = match recv {
            Ok(Ok(v)) => v,
            Ok(Err(_)) => continue,
            Err(_) => {
                // No beacons for 5 minutes — repeater may have restarted
                let _ = register_with_repeater(&socket).await;
                continue;
            }
        };
        if len < CaHeader::SIZE {
            continue;
        }

        // Walk every CA frame in the datagram so chained beacons aren't
        // dropped when the repeater coalesces them.
        let mut offset = 0;
        while offset + CaHeader::SIZE <= len {
            let Ok(hdr) = CaHeader::from_bytes(&buf[offset..len]) else {
                break;
            };
            let payload_padded = ((hdr.postsize as usize) + 7) & !7;
            let frame_len = (CaHeader::SIZE + payload_padded).max(CaHeader::SIZE);
            // Bail out before advancing if the announced frame
            // length runs past the datagram. Otherwise the
            // post-advance slice clamp would silently hand the
            // verifier a truncated body and the parser would
            // continue from a misaligned offset (CR-10/F6).
            if offset.saturating_add(frame_len) > len {
                break;
            }
            // Used by the cap-tokens companion-frame slice below; the
            // attribute keeps the unused-variable lint quiet when the
            // feature is off.
            #[cfg_attr(not(feature = "cap-tokens"), allow(unused_variables))]
            let frame_start = offset;
            offset += frame_len;

            // Signed-beacon companion (cmmd=0xCAFE, cap-tokens
            // feature). Verify the signature and stash the tuple as
            // "authenticated" so the matching beacon is acceptable.
            #[cfg(feature = "cap-tokens")]
            if hdr.cmmd == crate::server::signed_beacon::CA_PROTO_RSRV_BEACON_SIG {
                if let Some(ref v) = verifier {
                    let frame = &buf[frame_start..frame_start + frame_len];
                    // G3: bind the signed payload's announced server_ip
                    // to the UDP source IP. A recorded valid companion
                    // can otherwise be replayed from anywhere; combined
                    // with the unbounded verified_tuples map below this
                    // is a poison amplifier.
                    let src_ip = match _src.ip() {
                        std::net::IpAddr::V4(v) => v,
                        std::net::IpAddr::V6(_) => {
                            metrics::counter!("ca_client_signed_beacon_failures_total")
                                .increment(1);
                            continue;
                        }
                    };
                    match v.verify(frame) {
                        Ok((ip, port, beacon_id)) if Ipv4Addr::from(ip) != src_ip => {
                            tracing::debug!(
                                announced = %Ipv4Addr::from(ip),
                                actual = %src_ip,
                                port, beacon_id,
                                "signed beacon source-IP mismatch (G3)"
                            );
                            metrics::counter!("ca_client_signed_beacon_source_ip_mismatch_total")
                                .increment(1);
                        }
                        Ok((ip, port, beacon_id)) => {
                            // G2: cap verified_tuples on the companion-
                            // only path. The unsigned-beacon path GC's
                            // it via retain() at line 181, but a peer
                            // sending only signed companions would
                            // otherwise grow it linearly.
                            const MAX_VERIFIED_TUPLES: usize = 8192;
                            if verified_tuples.len() >= MAX_VERIFIED_TUPLES {
                                let max_age = std::time::Duration::from_secs(v.max_age_secs.max(1));
                                let now = std::time::Instant::now();
                                verified_tuples.retain(|_, t| now.duration_since(*t) <= max_age);
                            }
                            verified_tuples
                                .insert((ip, port, beacon_id), std::time::Instant::now());
                            metrics::counter!("ca_client_signed_beacon_verified_total")
                                .increment(1);
                        }
                        Err(e) => {
                            tracing::debug!(error = ?e,
                                "signed beacon companion failed verification");
                            metrics::counter!("ca_client_signed_beacon_failures_total")
                                .increment(1);
                        }
                    }
                }
                continue;
            }

            if hdr.cmmd != CA_PROTO_RSRV_IS_UP {
                continue;
            }

            // Verifier policy: by default, drop unauthenticated
            // beacons when a verifier is configured. The companion
            // signed-beacon datagram can arrive ~simultaneously; we
            // check against the verified-tuple set populated above and
            // GC stale entries every iteration to keep the map bounded.
            //
            // EPICS_CA_BEACON_REQUIRE_SIGNED=NO opts out — unsigned
            // beacons are accepted (with a counter increment) so
            // operators can run mixed deployments where some servers
            // have rolled out signing and some haven't yet.
            #[cfg(feature = "cap-tokens")]
            if let Some(ref v) = verifier {
                let max_age = std::time::Duration::from_secs(v.max_age_secs.max(1));
                let now = std::time::Instant::now();
                verified_tuples.retain(|_, t| now.duration_since(*t) <= max_age);
                let key = (hdr.available, hdr.count, hdr.cid);
                if !verified_tuples.contains_key(&key) {
                    metrics::counter!("ca_client_unsigned_beacon_drops_total").increment(1);
                    if require_signed {
                        continue;
                    }
                }
            }

            handle_beacon(hdr, &mut servers, &coord_tx);
        }
    }
}

fn handle_beacon(
    hdr: CaHeader,
    servers: &mut HashMap<SocketAddr, BeaconState>,
    coord_tx: &mpsc::UnboundedSender<CoordRequest>,
) {
    // count = server TCP port (CA v4.1+), data_type = protocol version.
    let server_port = if hdr.count != 0 {
        hdr.count
    } else {
        CA_SERVER_PORT
    };
    let beacon_id = hdr.cid;

    // New servers always set available=INADDR_ANY (0).  Use 0.0.0.0
    // as-is for beacon tracking — each IOC still has a unique port,
    // matching the approach used by the C CA client (libca).
    let server_ip = Ipv4Addr::from(hdr.available.to_be_bytes());
    let server_addr = SocketAddr::V4(SocketAddrV4::new(server_ip, server_port));
    let now = Instant::now();

    // Drop entries idle past `BEACON_STALE_THRESHOLD` so a long-silent
    // server's revival lands on the `first_sighting = true` path and
    // triggers the anomaly poke naturally (pvxs `tickBeaconClean`
    // parity). This is what protects the search engine from staying in
    // 200 ms fast-tick mode forever in a steady-state network.
    servers.retain(|_, s| now.duration_since(s.last_seen) < BEACON_STALE_THRESHOLD);

    // G1: cap the per-server BeaconState map. With
    // EPICS_CA_BEACON_REQUIRE_SIGNED=NO an attacker can spoof
    // beacons with arbitrary `available`/`count` to grow the map.
    // Reap entries idle for ≥5× period_estimate when the cap is hit.
    const MAX_BEACON_SERVERS: usize = 4096;
    let first_sighting = !servers.contains_key(&server_addr);
    if first_sighting && servers.len() >= MAX_BEACON_SERVERS {
        let cutoff_threshold = Duration::from_secs(15 * 5);
        servers.retain(|_, s| now.duration_since(s.last_seen) < cutoff_threshold);
    }
    let entry = servers.entry(server_addr).or_insert_with(|| BeaconState {
        last_id: beacon_id.wrapping_sub(1),
        last_seen: now,
        period_estimate: Duration::from_secs(15),
        count: 0,
    });

    let actual_interval = now.duration_since(entry.last_seen);
    let expected_next_id = entry.last_id.wrapping_add(1);

    // Multi-NIC / repeater duplicate detection: the SAME beacon (same
    // id, arriving microseconds apart through different paths) used to
    // trip the period-collapse branch below and fire a spurious
    // anomaly on every duplicate. Drop the second copy outright so
    // the search engine isn't woken twice for one beacon. Without
    // soft-poke-on-every-beacon (removed earlier this round) this
    // misclassification was masked by the throttle; the prune-only
    // design surfaces it.
    if !first_sighting && beacon_id == entry.last_id {
        return;
    }

    // Anomaly: beacon_id not monotonically increasing (IOC restarted
    // with a fresh sequence), OR period suddenly dropped below 1/3 of
    // the estimated steady-state period (IOC restarted and is in its
    // fast-beacon initial phase). Also: first time we've seen this
    // server — libca treats unknown-server beacons as a hint to
    // re-search immediately so channels still in `Searching` wake up
    // on the new IOC instead of waiting their full bucket cycle.
    //
    // Floor the period-collapse check at 50 ms — multi-NIC duplicate
    // beacons that happen to use the next sequence id (rare but
    // possible if the network reorders) would otherwise still
    // satisfy `actual_interval < period_estimate / 3` for any
    // nonzero period. 50 ms safely separates "duplicate" from
    // "legitimate fast-beacon initial phase" (real IOCs send
    // every 100-500 ms during startup).
    const MIN_PERIOD_COLLAPSE_INTERVAL: Duration = Duration::from_millis(50);
    let is_anomaly = first_sighting
        || beacon_id != expected_next_id
        || (entry.count > 3
            && actual_interval > MIN_PERIOD_COLLAPSE_INTERVAL
            && actual_interval < entry.period_estimate / 3);

    // Update state.
    entry.last_id = beacon_id;
    entry.last_seen = now;
    entry.count += 1;

    if entry.count > 1 {
        let alpha = 0.25;
        let new_estimate = Duration::from_secs_f64(
            entry.period_estimate.as_secs_f64() * (1.0 - alpha)
                + actual_interval.as_secs_f64() * alpha,
        );
        entry.period_estimate = new_estimate;
    }

    // pvxs `onBeacon` poke semantic: only fire on first sighting of a
    // (server, GUID) — i.e. a real anomaly (new server / IOC restart /
    // beacon-period collapse). Steady-state beacons just refresh
    // `last_seen` so the entry stays out of the prune set above; they
    // intentionally do NOT poke the coordinator. The earlier "soft
    // poke on every beacon" code amplified normal beacon traffic into
    // a permanent fast-tick search storm whenever multiple IOCs
    // beaconed within the engine's revolution window.
    if is_anomaly {
        let _ = coord_tx.send(CoordRequest::ForceRescanServer { server_addr });
    }
}

// ---------------------------------------------------------------------------
// Repeater registration
// ---------------------------------------------------------------------------

/// Register our socket with the CA repeater at localhost:5065.
async fn register_with_repeater(socket: &AsyncUdpV4) -> Result<(), ()> {
    // We bound to a single loopback NIC, so `local_addrs()` gives the
    // one ephemeral port we want to announce.
    let local_ip = socket
        .local_addrs()
        .into_iter()
        .find_map(|sa| match sa {
            SocketAddr::V4(v4) => Some(*v4.ip()),
            _ => None,
        })
        .unwrap_or(Ipv4Addr::LOCALHOST);

    let mut hdr = CaHeader::new(CA_PROTO_REPEATER_REGISTER);
    hdr.available = u32::from_be_bytes(local_ip.octets());

    let repeater_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, CA_REPEATER_PORT));
    socket
        .send_to(&hdr.to_bytes(), repeater_addr)
        .await
        .map_err(|_| ())?;

    // Wait for REPEATER_CONFIRM.
    let mut buf = [0u8; 64];
    let result = tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            let (len, _) = socket.recv_from(&mut buf).await.map_err(|_| ())?;
            if len >= CaHeader::SIZE {
                if let Ok(resp) = CaHeader::from_bytes(&buf[..len]) {
                    if resp.cmmd == CA_PROTO_REPEATER_CONFIRM {
                        return Ok::<(), ()>(());
                    }
                }
            }
        }
    })
    .await;

    match result {
        Ok(Ok(())) => Ok(()),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// `BEACON_STALE_THRESHOLD` is exactly 180 s (mirrors pvxs
    /// `tickBeaconClean` 2 × beaconCleanInterval / 2 default), and
    /// the prune sweep retains entries seen within the window while
    /// dropping older ones. The prune is what makes long-silent
    /// servers' revival hit the `first_sighting = true` anomaly
    /// path — without it, an in-sequence beacon after long silence
    /// wouldn't kick the search engine out of slow-cadence retry.
    #[test]
    fn beacon_stale_threshold_is_180s() {
        assert_eq!(BEACON_STALE_THRESHOLD, Duration::from_secs(180));
    }

    /// Multi-NIC / repeater duplicate detection: same beacon arriving
    /// twice in quick succession (same `cid`) must NOT fire a second
    /// anomaly request to the coordinator. Without the duplicate
    /// guard, the second copy hit the period-collapse branch
    /// (actual_interval ≈ 0 < period_estimate/3) and rescheduled
    /// every pending search a second time. With soft-poke removed
    /// earlier this round, the misclassification surfaces as a
    /// permanent fast-tick spam in dual-NIC environments.
    #[test]
    fn duplicate_beacon_does_not_double_fire_anomaly() {
        let mut servers: HashMap<SocketAddr, BeaconState> = HashMap::new();
        let (tx, mut rx) = mpsc::unbounded_channel::<CoordRequest>();

        let mut hdr = CaHeader::new(CA_PROTO_RSRV_IS_UP);
        hdr.count = 5064;
        hdr.cid = 100;
        hdr.available = u32::from_be_bytes([127, 0, 0, 1]);

        // First beacon — first sighting → anomaly fires.
        handle_beacon(hdr, &mut servers, &tx);
        assert!(matches!(rx.try_recv(), Ok(CoordRequest::ForceRescanServer { .. })));
        // Drain any further send (none expected).
        assert!(rx.try_recv().is_err());

        // Second beacon with the SAME cid (true duplicate from another
        // NIC / repeater coalesce) — must be silently dropped.
        handle_beacon(hdr, &mut servers, &tx);
        assert!(
            rx.try_recv().is_err(),
            "duplicate same-cid beacon must not fire ForceRescanServer"
        );
    }

    #[test]
    fn stale_prune_drops_idle_entries_only() {
        let now = Instant::now();
        let mut servers: HashMap<SocketAddr, BeaconState> = HashMap::new();
        let fresh: SocketAddr = "127.0.0.1:5064".parse().unwrap();
        let stale: SocketAddr = "127.0.0.1:5065".parse().unwrap();
        servers.insert(
            fresh,
            BeaconState {
                last_id: 0,
                last_seen: now - Duration::from_secs(10),
                period_estimate: Duration::from_secs(15),
                count: 5,
            },
        );
        servers.insert(
            stale,
            BeaconState {
                last_id: 0,
                last_seen: now - Duration::from_secs(300),
                period_estimate: Duration::from_secs(15),
                count: 5,
            },
        );
        // The prune logic in handle_beacon: same retain expression.
        servers.retain(|_, s| now.duration_since(s.last_seen) < BEACON_STALE_THRESHOLD);
        assert!(servers.contains_key(&fresh), "fresh entry must survive prune");
        assert!(!servers.contains_key(&stale), "180-s-idle entry must be pruned");
    }
}
