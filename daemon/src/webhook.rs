//! Remote webhook notifications (Sniffnet's "remote notifications" ported).
//!
//! POSTs a small JSON payload to a user-configured endpoint (SIGNL4, IFTTT,
//! Svix, ntfy, a custom server, ...). Blocking; call via `spawn_blocking`.

use serde_json::json;

/// Send a webhook notification. Returns `Err` with a message on failure.
pub fn notify(url: &str, payload: serde_json::Value) -> Result<(), String> {
    let resp = ureq::post(url)
        .set("Content-Type", "application/json")
        .send_json(payload)
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(format!("webhook returned {status}"))
    }
}

/// Build the "data threshold exceeded" payload.
pub fn threshold_payload(threshold: &str, data: &str) -> serde_json::Value {
    json!({
        "info": "Bytes threshold exceeded",
        "timestamp": now_string(),
        "threshold": threshold,
        "data": data,
    })
}

fn now_string() -> String {
    // jiff-free: format a unix timestamp as YYYY/MM/DD HH:MM:SS (UTC).
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3_600, (rem % 3_600) / 60, rem % 60);
    // days since epoch → civil date (approximation is fine for a notification).
    let (y, mo, d) = civil_from_days(days as i64);
    format!("{y:04}/{mo:02}/{d:02} {h:02}:{m:02}:{s:02}")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    // Howard Hinnant's algorithm (public domain).
    let z = z + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
