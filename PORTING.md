# PORTING — wiring the vendored Sniffnet engine into the daemon

The `daemon/` crate ships with a self-contained MVP capture loop. This document
explains how to replace it with the real Sniffnet engine (vendored under
`vendor/`) so we get reverse-DNS, GeoLite2 country/ASN, and the 6000+ L7 service
signatures.

## What upstream already gives us for free

Sniffnet's engine is cleanly separated from its GUI and communicates only through
typed channels:

- `vendor/sniffnet-src/networking/capture.rs` exposes
  `spawn_capture_thread(cap_id, capture_source, capture_context, mmdb_readers,
  ip_blacklist, filters, tx, freeze_rxs)` — it spawns the parse thread.
- The parse thread emits `BackendTrafficMessage::TickRun(usize, InfoTraffic,
  Vec<HostMessage>, bool)` once per second via an `async_channel::Sender`.
- `vendor/sniffnet-src/networking/types/info_traffic.rs` is the aggregation state
  (totals, `map: HashMap<AddressPortPair, InfoAddressPortPair>`, `services`,
  `hosts`).

So the port is: call `spawn_capture_thread`, consume `TickRun`, and serialize
`InfoTraffic` → JSON → broadcast (replacing the GUI's `Task::run` consumer).
Our `daemon/src/ws.rs` + `daemon/src/model.rs` already define that side.

## Engine crate structure

Create a new crate (e.g. `engine/`, or make `daemon/` depend on it) containing:

```
engine/
├── Cargo.toml
├── src/
│   ├── lib.rs                 # crate root (re-exports, see below)
│   ├── networking/            # vendored: networking/**  (minus ipfix if unused)
│   ├── mmdb/                  # vendored: mmdb/**
│   ├── countries/             # vendored: countries/** (Country type only)
│   ├── utils/                 # vendored: utils/** (error_logger, formatted_strings, types)
│   └── engine_config.rs       # NEW: minimal config replacing Conf
└── resources/DB/*.mmdb        # GeoLite2 files (download separately — see below)
```

`lib/sniffnet-packet-parser` (`vendor/sniffnet-lib/`) is added as a path
dependency.

## Crate-root re-exports

Upstream relies on crate-root imports. Replicate them in the engine's `lib.rs`:

```rust
pub use networking::types::service::Service;
pub use sniffnet_packet_parser::Protocol;
// plus the `location!` macro from utils::error_logger, and any other `crate::X`
// items the copied modules reference (grep for `use crate::` after copying).
```

## Decoupling GUI-coupled types

The engine imports a handful of types from `gui/` that must be moved or replaced:

| Import | Where used | Action |
|---|---|---|
| `crate::gui::types::filters::Filters` | `capture.rs` | Copy `filters.rs` into the engine (it is standalone; only `deserialize_or_default` comes from `conf.rs` — inline it). |
| `crate::gui::types::conf::Conf` | `capture_context.rs` (`CaptureSource::from_conf`) | Replace with a minimal `EngineConfig { device: String, ... }`. |
| `crate::gui::types::ipfix_socket::MyIpfixSocket` | `capture_context.rs` | Drop if IPFIX isn't needed, or copy `ipfix_socket.rs`. |
| `crate::translations::*` | `capture_context.rs` (`CaptureSource::title`) | Replace with plain string literals. |
| `crate::report::types::sort_type::SortType` | `data_info.rs`, `info_address_port_pair.rs` | Copy the small `SortType` enum, or strip `compare()` methods. |
| `crate::countries::types::country::Country` | `host.rs` | Keep `Country`; drop `flags_pictures` (iced flag rendering). |
| `crate::mmdb::*` | `capture.rs` | Keep the whole `mmdb/` module. |
| `crate::utils::error_logger::{ErrorLogger, Location}` + `location!` | widely | Keep `utils/error_logger.rs` + the macro. |

## Drop entirely

`gui/`, `chart/`, `translations/`, `report/` (most of it), `notifications/`,
`cli/`, and the desktop-only `networking/types/program.rs` +
`program_lookup.rs` (process identification). On a router, replace program
lookup with LAN-client identification via the ARP/neighbor table.

## Data files

- `resources/services/services.txt` + `categories.txt` → L7 service signatures
  (compiled by `build.rs` via `phf_codegen`). Keep them and upstream's `build.rs`.
- `resources/DB/*.mmdb` → GeoLite2-ASN + GeoLite2-Country. These are **not**
  freely redistributable; download them with a free MaxMind key (or switch to
  DB-IP Lite, CC-BY-4.0).

## Serialization bridge

`InfoTraffic` / `DataInfo` / `InfoAddressPortPair` are `pub` structs but not
`serde::Serialize`. Rather than adding derives to upstream types, keep a mapping
layer — extend `daemon/src/model.rs` to convert `InfoTraffic` → `Snapshot`:

```rust
fn snapshot_from_tick(device: &str, info: &InfoTraffic, hosts: &[HostMessage]) -> Snapshot {
    // totals        ← info.tot_data_info (use .incoming_data/.outgoing_data)
    // connections   ← info.map  (AddressPortPair + InfoAddressPortPair)
    // services      ← info.services
    // hosts (geo/asn/rDNS) ← hosts
    // ...
}
```

`Host` is already `Serialize`, so country/ASN/domain arrive for free.

## Wiring

In `daemon/src/main.rs`, replace `capture::run(...)` with:

1. Build `CaptureSource::Device(...)` from the detected WAN device.
2. `CaptureContext::new(&source, None, &filters)`.
3. `MmdbReaders` (loaded once), `IpBlacklist`, `Filters`.
4. `let (tx, rx) = async_channel::unbounded();`
5. `spawn_capture_thread(cap_id, source, ctx, mmdb, blacklist, filters, tx.clone(), freeze_rxs);`
6. A bridge task: `while let Ok(msg) = rx.recv().await { if let TickRun(_, info, hosts, _) = msg { let snap = snapshot_from_tick(...); tx_broadcast.send(snap); } }`

This preserves Sniffnet's thread model (capture / reverse-DNS / lookup threads)
while swapping only the final consumer.
