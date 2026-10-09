//! Pure string helpers used by the engine (translations decoupled out).

use std::cmp::min;
use std::net::IpAddr;

pub fn get_domain_from_r_dns(r_dns: String) -> String {
    if r_dns.parse::<IpAddr>().is_ok() || r_dns.is_empty() {
        r_dns
    } else {
        let parts: Vec<&str> = r_dns.split('.').collect();
        let len = parts.len();
        if len >= 2 {
            let last = parts.get(len - 1).unwrap_or(&"");
            let second_last = parts.get(len - 2).unwrap_or(&"");
            if last.len() > 3 || second_last.len() > 3 {
                format!("{second_last}.{last}")
            } else {
                let third_last_opt = len.checked_sub(3).and_then(|i| parts.get(i));
                match third_last_opt {
                    Some(third_last) => format!("{third_last}.{second_last}.{last}"),
                    None => format!("{second_last}.{last}"),
                }
            }
        } else {
            r_dns
        }
    }
}

pub fn clip_text(text: &str, max_chars: usize) -> String {
    let text = text.trim();
    let chars = text.chars().collect::<Vec<char>>();
    let tot_len = chars.len();
    let slice_len = min(max_chars, tot_len);

    let suspensions = if tot_len > max_chars { "…" } else { "" };
    let slice = if tot_len > max_chars {
        &chars[..slice_len.saturating_sub(2)]
    } else {
        &chars[..slice_len]
    }
    .iter()
    .collect::<String>();

    [slice.trim(), suspensions].concat()
}
