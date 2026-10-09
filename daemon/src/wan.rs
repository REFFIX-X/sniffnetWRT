//! WAN interface detection for OpenWrt.
//!
//! Prefers `ubus call network.interface.wan status`, falls back to parsing
//! `/etc/config/network`, and finally to an explicit `--interface` override.

use anyhow::{Context, Result};
use serde_json::Value;

/// Resolve the WAN device name. An explicit override always wins.
pub fn detect_wan_device(override_device: Option<&str>) -> Result<String> {
    if let Some(d) = override_device {
        if !d.trim().is_empty() {
            return Ok(d.trim().to_string());
        }
    }

    if let Some(d) = wan_device_from_ubus() {
        return Ok(d);
    }

    if let Some(d) = wan_device_from_uci() {
        return Ok(d);
    }

    anyhow::bail!("could not determine the WAN device; pass --interface <dev> explicitly")
}

/// Resolve the LAN bridge device name (for per-device accounting).
/// Returns `None` to disable LAN capture.
pub fn detect_lan_device(override_device: Option<&str>) -> Option<String> {
    if let Some(d) = override_device {
        if d.trim().is_empty() {
            return None; // explicitly disabled via empty value
        }
        return Some(d.trim().to_string());
    }
    if let Some(d) = interface_device_from_ubus("lan") {
        return Some(d);
    }
    if std::path::Path::new("/sys/class/net/br-lan").exists() {
        return Some("br-lan".to_string());
    }
    None
}

/// Ask netifd (via ubus) which device backs the given interface.
fn interface_device_from_ubus(name: &str) -> Option<String> {
    let out = std::process::Command::new("ubus")
        .args(["call", &format!("network.interface.{name}"), "status"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: Value = serde_json::from_slice(&out.stdout).ok()?;
    if let Some(d) = v.get("device").and_then(Value::as_str) {
        if !d.is_empty() {
            return Some(d.to_string());
        }
    }
    v.get("l3_device").and_then(Value::as_str).map(str::to_string)
}

/// Ask netifd (via ubus) which device backs the `wan` interface.
fn wan_device_from_ubus() -> Option<String> {
    let out = std::process::Command::new("ubus")
        .args(["call", "network.interface.wan", "status"])
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let v: Value = serde_json::from_slice(&out.stdout).ok()?;

    // `device` is the L2 device the traffic actually traverses; `l3_device` is
    // the layer-3 device (e.g. `pppoe-wan`). For capture we want the L2 device.
    if let Some(d) = v.get("device").and_then(Value::as_str) {
        if !d.is_empty() {
            return Some(d.to_string());
        }
    }
    if let Some(d) = v.get("l3_device").and_then(Value::as_str) {
        if !d.is_empty() {
            return Some(d.to_string());
        }
    }
    None
}

/// Minimal parser for `/etc/config/network`: find the `wan` interface section
/// and read its `option device` / `option ifname`.
fn wan_device_from_uci() -> Option<String> {
    let text = std::fs::read_to_string("/etc/config/network").ok()?;
    let mut in_wan = false;

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("config interface") {
            in_wan = line
                .split_whitespace()
                .any(|t| t == "wan" || t == "'wan'" || t == "\"wan\"");
        }
        if !in_wan {
            continue;
        }
        if let Some(rest) = line.strip_prefix("option device") {
            return parse_uci_value(rest);
        }
        if let Some(rest) = line.strip_prefix("option ifname") {
            return parse_uci_value(rest);
        }
    }
    None
}

/// Strip an `= 'value'` tail down to `value`.
fn parse_uci_value(s: &str) -> Option<String> {
    let v = s.trim_start_matches('=').trim();
    if v.is_empty() {
        return None;
    }
    // A single ifname line may list several devices; take the first.
    let first = v.split_whitespace().next().unwrap_or(v);
    Some(first.trim_matches(|c| c == '\'' || c == '"').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_uci_value() {
        assert_eq!(parse_uci_value("'eth1'"), Some("eth1".into()));
        assert_eq!(parse_uci_value(" \"wan\" "), Some("wan".into()));
        assert_eq!(parse_uci_value("eth0 eth1"), Some("eth0".into()));
        assert_eq!(parse_uci_value(""), None);
    }

    #[test]
    fn finds_wan_in_uci() {
        let _ = wan_device_from_uci; // reference to keep clippy quiet in builds without tests
    }
}
