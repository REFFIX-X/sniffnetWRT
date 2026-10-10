//! Bounded, RAM-only reader for WAN firewall rejection messages.

use std::collections::VecDeque;
use std::net::IpAddr;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use sniffnetwrt_engine::mmdb::types::mmdb_country_entry::MmdbCountryEntry;
use sniffnetwrt_engine::mmdb::types::mmdb_reader::MmdbReader;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::RwLock;

use crate::model::SecurityEvent;

const MAX_EVENTS: usize = 250;

pub type EventBuffer = Arc<RwLock<VecDeque<SecurityEvent>>>;

pub fn new_buffer() -> EventBuffer {
    Arc::new(RwLock::new(VecDeque::with_capacity(MAX_EVENTS)))
}

/// Read logd's RAM stream. Restart logread if it exits; never write events to disk.
pub async fn follow(buffer: EventBuffer, country_db: Arc<MmdbReader>) {
    loop {
        let child = tokio::process::Command::new("logread")
            .args(["-f"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = child else {
            tracing::warn!("WAN firewall events enabled, but logread could not start");
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            continue;
        };

        if let Some(stdout) = child.stdout.take() {
            let mut lines = BufReader::new(stdout).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        if let Some(event) = parse_rejection(&line, &country_db) {
                            let mut events = buffer.write().await;
                            if events.len() == MAX_EVENTS {
                                events.pop_front();
                            }
                            events.push_back(event);
                        }
                    }
                    Ok(None) | Err(_) => break,
                }
            }
        }
        let _ = child.kill().await;
        let _ = child.wait().await;
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

fn parse_rejection(line: &str, country_db: &MmdbReader) -> Option<SecurityEvent> {
    // fw4's zone logging emits rejected packets. Ignore all other system log lines.
    if !line.to_ascii_uppercase().contains("REJECT") {
        return None;
    }
    let source_ip = log_field(line, "SRC=")?.to_owned();
    let destination_ip = log_field(line, "DST=")?.to_owned();
    let source_port = log_field(line, "SPT=").and_then(|v| v.parse().ok());
    let destination_port = log_field(line, "DPT=").and_then(|v| v.parse().ok());
    let protocol = log_field(line, "PROTO=")?.to_ascii_lowercase();
    let source = source_ip.parse::<IpAddr>().ok();
    let country = source
        .and_then(|ip| country_db.lookup::<MmdbCountryEntry>(ip))
        .map(|entry| entry.get_country().to_string())
        .filter(|code| code != "ZZ");
    let marker = line.to_ascii_lowercase();
    let target = if marker.contains("input") {
        "router"
    } else {
        "forwarded"
    };
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();

    Some(SecurityEvent {
        timestamp,
        source_ip,
        source_port,
        destination_ip,
        destination_port,
        protocol,
        target: target.to_owned(),
        country,
    })
}

fn log_field<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    line.split_ascii_whitespace()
        .find_map(|word| word.strip_prefix(name))
}
