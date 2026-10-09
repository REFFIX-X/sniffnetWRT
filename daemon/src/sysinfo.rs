//! Router system info: health, WAN status, and LAN-client discovery.
//!
//! Reads standard OpenWrt /proc + sysfs files and `ubus`/`arp`/DHCP leases.
//! Every function is defensive and returns a default/`None` on failure.

use crate::model::{Device, Health, WanStatus};
use serde_json::Value;
use std::collections::BTreeMap;

/// CPU load average, memory, uptime, and temperature.
pub fn health() -> Health {
    let (load1, load5, load15) = loadavg();
    let (mem_total, mem_used) = meminfo();
    Health {
        load1,
        load5,
        load15,
        mem_total_kb: mem_total,
        mem_used_kb: mem_used,
        uptime_secs: uptime(),
        temp_c: temperature(),
    }
}

/// WAN status via ubus + per-interface counters from sysfs.
pub fn wan_status(device: &str) -> Option<WanStatus> {
    let out = std::process::Command::new("ubus")
        .args(["call", "network.interface.wan", "status"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: Value = serde_json::from_slice(&out.stdout).ok()?;

    let ip = v
        .get("ipv4-address")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|e| e.get("address"))
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string();
    let dev = v
        .get("device")
        .and_then(Value::as_str)
        .unwrap_or(device)
        .to_string();
    let uptime_secs = v.get("uptime").and_then(Value::as_u64).unwrap_or(0);
    let (rx_bytes, tx_bytes) = iface_counters(&dev);

    Some(WanStatus {
        ip,
        device: dev,
        uptime_secs,
        rx_bytes,
        tx_bytes,
    })
}

/// LAN clients from ARP (online status) + DHCP leases (hostnames).
pub fn devices() -> Vec<Device> {
    let mut map: BTreeMap<String, Device> = BTreeMap::new();

    // ARP gives us online IPv4 devices (MAC → IP).
    if let Ok(text) = std::fs::read_to_string("/proc/net/arp") {
        for line in text.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let ip = parts[0].to_string();
                let mac = parts[3].to_uppercase();
                if mac != "00:00:00:00:00:00" && ip != "0.0.0.0" {
                    map.entry(mac.clone()).or_insert_with(|| Device {
                        mac,
                        ip,
                        hostname: String::new(),
                        online: true,
                    });
                }
            }
        }
    }

    // DHCP leases add hostnames (and clients that aren't in ARP yet).
    if let Ok(text) = std::fs::read_to_string("/tmp/dhcp.leases") {
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let mac = parts[1].to_uppercase();
                let ip = parts[2].to_string();
                let hostname = parts[3].to_string();
                let e = map.entry(mac.clone()).or_insert_with(|| Device {
                    mac,
                    ip,
                    hostname,
                    online: false,
                });
                if !hostname.is_empty() && hostname != "*" {
                    e.hostname = hostname;
                }
            }
        }
    }

    map.into_values().collect()
}

fn loadavg() -> (f32, f32, f32) {
    let Ok(text) = std::fs::read_to_string("/proc/loadavg") else {
        return (0.0, 0.0, 0.0);
    };
    let parts: Vec<&str> = text.split_whitespace().collect();
    (
        parts.first().and_then(|s| s.parse().ok()).unwrap_or(0.0),
        parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0),
    )
}

fn meminfo() -> (u64, u64) {
    let Ok(text) = std::fs::read_to_string("/proc/meminfo") else {
        return (0, 0);
    };
    let mut total = 0u64;
    let mut available = 0u64;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total = rest.trim().trim_end_matches(" kB").trim().parse().unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            available = rest.trim().trim_end_matches(" kB").trim().parse().unwrap_or(0);
        }
    }
    (total, total.saturating_sub(available))
}

fn uptime() -> u64 {
    std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|v| v.parse().ok()))
        .unwrap_or(0)
}

fn temperature() -> Option<f32> {
    // First thermal zone is usually the SoC temperature.
    let raw = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp").ok()?;
    let milli: i64 = raw.trim().parse().ok()?;
    Some(milli as f32 / 1000.0)
}

fn iface_counters(dev: &str) -> (u64, u64) {
    let read = |f: &str| {
        std::fs::read_to_string(format!("/sys/class/net/{dev}/statistics/{f}"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    };
    (read("rx_bytes"), read("tx_bytes"))
}
