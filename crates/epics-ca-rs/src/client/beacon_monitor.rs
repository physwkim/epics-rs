use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::{Duration, Instant};

use epics_base_rs::runtime::sync::mpsc;
use tokio::net::UdpSocket;

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

// ---------------------------------------------------------------------------
// Beacon monitor task
// ---------------------------------------------------------------------------

/// Receives beacon messages from the CA repeater, detects anomalies (IOC
/// restart), and notifies the coordinator to rescan affected channels.
/// Re-registration interval: if no beacons for this long, re-register
/// with the repeater in case it restarted.
const REREGISTER_INTERVAL: Duration = Duration::from_secs(300);

pub(crate) async fn run_beacon_monitor(coord_tx: mpsc::UnboundedSender<CoordRequest>) {
    // Bind a dedicated UDP socket for beacon reception.
    let socket = match UdpSocket::bind("0.0.0.0:0").await {
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

    let mut servers: HashMap<SocketAddr, BeaconState> = HashMap::new();
    let mut buf = [0u8; 256];

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
        let Ok(hdr) = CaHeader::from_bytes(&buf[..len]) else {
            continue;
        };

        if hdr.cmmd != CA_PROTO_RSRV_IS_UP {
            continue;
        }

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

        let entry = servers.entry(server_addr).or_insert_with(|| BeaconState {
            last_id: beacon_id.wrapping_sub(1),
            last_seen: now,
            period_estimate: Duration::from_secs(15),
            count: 0,
        });

        let actual_interval = now.duration_since(entry.last_seen);
        let expected_next_id = entry.last_id.wrapping_add(1);

        // Anomaly: beacon_id not monotonically increasing (IOC restarted
        // with a fresh sequence), OR period suddenly dropped below 1/3 of
        // the estimated steady-state period (IOC restarted and is in its
        // fast-beacon initial phase).
        let is_anomaly = beacon_id != expected_next_id
            || (entry.count > 3 && actual_interval < entry.period_estimate / 3);

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

        if is_anomaly {
            let _ = coord_tx.send(CoordRequest::ForceRescanServer { server_addr });
        }
    }
}

// ---------------------------------------------------------------------------
// Repeater registration
// ---------------------------------------------------------------------------

/// Register our socket with the CA repeater at localhost:5065.
async fn register_with_repeater(socket: &UdpSocket) -> Result<(), ()> {
    let local_ip = match socket.local_addr().ok() {
        Some(SocketAddr::V4(v4)) => *v4.ip(),
        _ => Ipv4Addr::LOCALHOST,
    };

    let mut hdr = CaHeader::new(CA_PROTO_REPEATER_REGISTER);
    hdr.available = u32::from_be_bytes(local_ip.octets());

    let repeater_addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, CA_REPEATER_PORT);
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
