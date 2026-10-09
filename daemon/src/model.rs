//! JSON model for the WebSocket snapshot protocol.
//!
//! Field names mirror Sniffnet's `InfoTraffic` / `InfoAddressPortPair` data model.

use serde::Serialize;

/// Periodic snapshot broadcast to every connected browser (once per second).
#[derive(Clone, Serialize)]
pub struct Snapshot {
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// Capture interface name (e.g. `wan`, `eth2`).
    pub interface: String,
    /// Unix timestamp (seconds) of this snapshot.
    pub timestamp: u64,
    pub totals: Totals,
    pub connections: Vec<Connection>,
    pub services: Vec<ServiceAgg>,
    /// Router health (CPU/RAM/temp/uptime), if readable.
    pub health: Option<Health>,
    /// WAN link status, if readable.
    pub wan: Option<WanStatus>,
    /// LAN clients discovered via ARP + DHCP leases.
    pub devices: Vec<Device>,
}

/// Cumulative counters plus the instantaneous rate (bytes/second).
#[derive(Clone, Serialize, Default)]
pub struct Totals {
    pub incoming_bytes: u64,
    pub outgoing_bytes: u64,
    pub incoming_packets: u64,
    pub outgoing_packets: u64,
    pub incoming_rate: f64,
    pub outgoing_rate: f64,
}

/// A single aggregated flow (address:port pair), like Sniffnet's row.
#[derive(Clone, Serialize)]
pub struct Connection {
    pub id: String,
    pub src_ip: String,
    pub src_port: u16,
    pub dst_ip: String,
    pub dst_port: u16,
    pub protocol: String,
    /// `"incoming"` (remote → router) or `"outgoing"` (router → remote).
    pub direction: String,
    /// Upper-layer service name.
    pub service: String,
    /// Service category (Web, Email, Chat, ...).
    pub category: String,
    pub bytes: u64,
    pub packets: u64,
    pub incoming_bytes: u64,
    pub outgoing_bytes: u64,
    pub first_seen: u64,
    pub last_seen: u64,
    /// ISO country code (from GeoLite2), if available.
    pub country: Option<String>,
    /// Autonomous system name (from GeoLite2-ASN), if available.
    pub asn: Option<String>,
    /// Domain (from reverse-DNS), if available.
    pub domain: Option<String>,
    /// Whether the remote address is on an imported IP blacklist.
    pub is_blacklisted: bool,
}

/// Aggregated bytes per upper-layer service.
#[derive(Clone, Serialize)]
pub struct ServiceAgg {
    pub name: String,
    pub bytes: u64,
    pub connections: usize,
}

/// Router health snapshot.
#[derive(Clone, Serialize)]
pub struct Health {
    pub load1: f32,
    pub load5: f32,
    pub load15: f32,
    pub mem_total_kb: u64,
    pub mem_used_kb: u64,
    pub uptime_secs: u64,
    pub temp_c: Option<f32>,
}

/// WAN link status.
#[derive(Clone, Serialize)]
pub struct WanStatus {
    pub ip: String,
    pub device: String,
    pub uptime_secs: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

/// A LAN client discovered via ARP / DHCP leases.
#[derive(Clone, Serialize)]
pub struct Device {
    pub mac: String,
    pub ip: String,
    pub hostname: String,
    pub online: bool,
}
