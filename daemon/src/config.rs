//! CLI / environment configuration for the daemon.

use clap::Parser;
use std::net::IpAddr;

#[derive(Parser, Clone, Debug)]
#[command(name = "sniffnetwrt", version, about = "Sniffnet-style WAN monitor for OpenWrt")]
pub struct Config {
    /// Network interface to capture (WAN). Auto-detected via ubus when omitted.
    #[arg(long, env = "SNIFFNETWRT_INTERFACE")]
    pub interface: Option<String>,

    /// LAN bridge to capture for per-device accounting (e.g. br-lan).
    /// Auto-detected via ubus when omitted; pass empty to disable.
    #[arg(long, env = "SNIFFNETWRT_LAN_DEVICE")]
    pub lan_device: Option<String>,

    /// Optional path to write a live PCAP export (disabled by default; download at `/pcap`).
    #[arg(long, env = "SNIFFNETWRT_PCAP_OUT")]
    pub pcap_out: Option<String>,

    /// Address to bind the web server to.
    #[arg(long, env = "SNIFFNETWRT_BIND", default_value = "0.0.0.0")]
    pub bind: IpAddr,

    /// Port for the web UI / WebSocket.
    #[arg(long, env = "SNIFFNETWRT_PORT", default_value_t = 9999)]
    pub port: u16,

    /// Directory containing the static web assets.
    #[arg(long, env = "SNIFFNETWRT_WEB_DIR", default_value = "web")]
    pub web_dir: String,

    /// Directory containing GeoLite2-{Country,ASN}.mmdb (optional).
    #[arg(long, env = "SNIFFNETWRT_MMDB_DIR", default_value = "db")]
    pub mmdb_dir: String,

    /// Optional webhook URL for remote notifications.
    #[arg(long, env = "SNIFFNETWRT_WEBHOOK")]
    pub webhook: Option<String>,

    /// Notify when total throughput exceeds this many bytes/second.
    #[arg(long, env = "SNIFFNETWRT_NOTIFY_THRESHOLD")]
    pub notify_threshold: Option<f64>,

    /// Optional token required as `?token=...` on the `/ws` connection.
    #[arg(long, env = "SNIFFNETWRT_TOKEN")]
    pub token: Option<String>,

    /// Optional BPF filter to apply to the capture.
    #[arg(long, env = "SNIFFNETWRT_BPF")]
    pub bpf: Option<String>,

    /// Snapshot interval in milliseconds (informational; the engine ticks at 1 Hz).
    #[arg(long, env = "SNIFFNETWRT_INTERVAL_MS", default_value_t = 1000)]
    pub interval_ms: u64,
}
