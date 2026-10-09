//! Bridge between the vendored Sniffnet engine and the WebSocket broadcaster.
//!
//! Spawns the engine's capture thread, consumes its `TickRun` snapshots,
//! accumulates them (`InfoTraffic::refresh`), enriches with router health/WAN/
//! LAN-client data, and broadcasts a `Snapshot`. Also fires remote webhook
//! notifications for blacklisted traffic and rate thresholds.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sniffnetwrt_engine::engine_config::Filters;
use sniffnetwrt_engine::mmdb::types::mmdb_reader::{MmdbReader, MmdbReaders};
use sniffnetwrt_engine::networking::capture::{spawn_capture_thread, BackendTrafficMessage};
use sniffnetwrt_engine::networking::types::capture_context::{CaptureContext, CaptureSource};
use sniffnetwrt_engine::networking::types::data_representation::DataRepr;
use sniffnetwrt_engine::networking::types::host::Host;
use sniffnetwrt_engine::networking::types::info_traffic::InfoTraffic;
use sniffnetwrt_engine::networking::types::ip_blacklist::IpBlacklist;
use sniffnetwrt_engine::networking::types::traffic_direction::TrafficDirection;
use tokio::sync::broadcast;

use crate::model::{Connection, ServiceAgg, Snapshot, Totals};
use crate::{sysinfo, webhook};

const MAX_CONNECTIONS: usize = 500;
const ALERT_COOLDOWN: Duration = Duration::from_secs(60);

/// Run the engine + bridge loop. Call once from a tokio task.
pub async fn run(
    device: String,
    bpf: Option<String>,
    mmdb_dir: String,
    blacklist_path: Option<String>,
    webhook_url: Option<String>,
    notify_threshold: Option<f64>,
    tx: broadcast::Sender<Snapshot>,
) -> Result<(), String> {
    let mut filters = Filters::default();
    if let Some(bpf) = bpf {
        if !bpf.trim().is_empty() {
            filters.expanded = true;
            filters.set_bpf(bpf);
        }
    }

    let source = CaptureSource::device_from_name(&device);
    let context = CaptureContext::new(&source, None, &filters);
    if let Some(e) = context.error() {
        return Err(e.to_string());
    }

    // GeoLite2 readers with a graceful empty fallback when the .mmdb files are
    // absent (geo/ASN enrichment is then simply skipped).
    let country_path = format!("{mmdb_dir}/GeoLite2-Country.mmdb");
    let asn_path = format!("{mmdb_dir}/GeoLite2-ASN.mmdb");
    let mmdb_readers = MmdbReaders {
        country: Arc::new(MmdbReader::from(&country_path, &[])),
        asn: Arc::new(MmdbReader::from(&asn_path, &[])),
    };

    // Load the IP blacklist (if configured).
    let ip_blacklist = match &blacklist_path {
        Some(p) => IpBlacklist::from_file(p.clone()).await,
        None => IpBlacklist::default(),
    };

    // Backend channel (engine → bridge) and freeze control (unused in headless mode).
    let (backend_tx, backend_rx) = async_channel::unbounded();
    let (freeze_tx, _) = tokio::sync::broadcast::channel(1_048_575);
    let freeze_rx = freeze_tx.subscribe();
    let freeze_rx2 = freeze_tx.subscribe();

    spawn_capture_thread(
        0,
        source,
        context,
        mmdb_readers,
        ip_blacklist,
        filters,
        backend_tx,
        (freeze_rx, freeze_rx2),
    );

    let mut accumulated = InfoTraffic::default();
    let mut ip_to_host: HashMap<IpAddr, Host> = HashMap::new();
    let mut last_alert: HashMap<String, Instant> = HashMap::new();

    while let Ok(msg) = backend_rx.recv().await {
        if let BackendTrafficMessage::TickRun(_, mut delta, host_msgs, _) = msg {
            for hm in host_msgs {
                ip_to_host.insert(hm.address_to_lookup, hm.host);
            }

            // The per-second `TickRun` delta is the instantaneous rate.
            let incoming_rate = delta.tot_data_info.incoming_data(DataRepr::Bytes) as f64;
            let outgoing_rate = delta.tot_data_info.outgoing_data(DataRepr::Bytes) as f64;

            accumulated.refresh(&mut delta);

            let snap = build_snapshot(
                &device,
                &accumulated,
                &ip_to_host,
                incoming_rate,
                outgoing_rate,
            );

            // Remote notifications.
            fire_alerts(&snap, webhook_url.as_deref(), notify_threshold, &mut last_alert);

            let _ = tx.send(snap);
        }
    }

    Ok(())
}

fn fire_alerts(
    snap: &Snapshot,
    webhook_url: Option<&str>,
    threshold: Option<f64>,
    last_alert: &mut HashMap<String, Instant>,
) {
    let Some(url) = webhook_url else { return };

    // Rate threshold.
    if let Some(t) = threshold {
        let total = snap.totals.incoming_rate + snap.totals.outgoing_rate;
        if total > t && cooldown_ok(last_alert, "threshold") {
            let payload = webhook::threshold_payload(
                &format!("{t:.0} B/s"),
                &format!("{total:.0} B/s"),
            );
            spawn_webhook(url, payload);
        }
    }

    // Blacklisted connections.
    for c in &snap.connections {
        if c.is_blacklisted && cooldown_ok(last_alert, &c.src_ip) {
            let payload = webhook::blacklist_payload(
                &c.src_ip,
                c.country.as_deref(),
                c.domain.as_deref(),
                c.asn.as_deref(),
                &format!("{} B", c.bytes),
            );
            spawn_webhook(url, payload);
        }
    }
}

fn cooldown_ok(last_alert: &mut HashMap<String, Instant>, key: &str) -> bool {
    let now = Instant::now();
    match last_alert.get(key) {
        Some(t) if now.duration_since(*t) < ALERT_COOLDOWN => false,
        _ => {
            last_alert.insert(key.to_string(), now);
            true
        }
    }
}

fn spawn_webhook(url: &str, payload: serde_json::Value) {
    let url = url.to_string();
    tokio::task::spawn_blocking(move || {
        if let Err(e) = webhook::notify(&url, payload) {
            tracing::warn!("webhook failed: {e}");
        }
    });
}

fn build_snapshot(
    device: &str,
    info: &InfoTraffic,
    ip_to_host: &HashMap<IpAddr, Host>,
    incoming_rate: f64,
    outgoing_rate: f64,
) -> Snapshot {
    let tot = &info.tot_data_info;
    let totals = Totals {
        incoming_bytes: tot.incoming_data(DataRepr::Bytes) as u64,
        outgoing_bytes: tot.outgoing_data(DataRepr::Bytes) as u64,
        incoming_packets: tot.incoming_data(DataRepr::Packets) as u64,
        outgoing_packets: tot.outgoing_data(DataRepr::Packets) as u64,
        incoming_rate,
        outgoing_rate,
    };

    let mut connections: Vec<Connection> = info
        .map
        .iter()
        .map(|(key, value)| {
            let remote_ip = match value.traffic_direction {
                TrafficDirection::Outgoing => key.dst_ip,
                TrafficDirection::Incoming => key.src_ip,
            };
            let host = ip_to_host.get(&remote_ip);
            let outgoing = value.traffic_direction == TrafficDirection::Outgoing;
            Connection {
                id: format!(
                    "{}:{}->{}:{}/{}",
                    key.src_ip,
                    key.src_port.unwrap_or(0),
                    key.dst_ip,
                    key.dst_port.unwrap_or(0),
                    key.protocol
                ),
                src_ip: key.src_ip.to_string(),
                src_port: key.src_port.unwrap_or(0),
                dst_ip: key.dst_ip.to_string(),
                dst_port: key.dst_port.unwrap_or(0),
                protocol: key.protocol.to_string().to_lowercase(),
                direction: if outgoing { "outgoing" } else { "incoming" }.to_string(),
                service: value.service.to_string(),
                category: value.service.category().to_string(),
                bytes: value.bytes as u64,
                packets: value.packets as u64,
                incoming_bytes: if outgoing { 0 } else { value.bytes as u64 },
                outgoing_bytes: if outgoing { value.bytes as u64 } else { 0 },
                first_seen: value.initial_timestamp.secs().max(0) as u64,
                last_seen: value.final_timestamp.secs().max(0) as u64,
                country: host.map(|h| h.country.to_string()),
                asn: host.map(|h| h.asn.name.clone()),
                domain: host.map(|h| h.domain.clone()),
                is_blacklisted: value.is_blacklisted,
            }
        })
        .collect();
    connections.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    connections.truncate(MAX_CONNECTIONS);

    let mut services: Vec<ServiceAgg> = info
        .services
        .iter()
        .map(|(service, data)| ServiceAgg {
            name: service.to_string(),
            bytes: data.tot_data(DataRepr::Bytes) as u64,
            connections: 0,
        })
        .collect();
    services.sort_by(|a, b| b.bytes.cmp(&a.bytes));

    Snapshot {
        kind: "tick",
        interface: device.to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        totals,
        connections,
        services,
        health: Some(sysinfo::health()),
        wan: sysinfo::wan_status(device),
        devices: sysinfo::devices(),
    }
}
