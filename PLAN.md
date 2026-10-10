# sniffnetWRT — Porting Sniffnet to OpenWrt (Flint 4) — Plan

## 1. Goal

Run a **Sniffnet-style real-time traffic monitor on the GL.iNet Flint 4 router**
(OpenWrt 25.12), capturing the **WAN** interface, and expose a **web UI on port 9999**
that looks like Sniffnet's dark dashboard (Overview + Inspect pages).

## 2. Key facts (verified)

| Item | Value |
|---|---|
| Upstream | [GyulyVGC/sniffnet](https://github.com/GyulyVGC/sniffnet) v1.5.1 |
| License | **MIT OR Apache-2.0** (free to fork/vendor) |
| Language / edition | Rust, edition 2024 (needs Rust ≥ 1.85) |
| GUI stack | `iced` 0.14 + `plotters`/`plotters-iced2` (desktop only) |
| Capture | `pcap` crate (links system libpcap) |
| Packet parsing | `sniffnet-packet-parser` (already a separate crate) + `etherparse` |
| Geo / ASN | `maxminddb` (GeoLite2 `resources/DB/*.mmdb`) |
| Service ID | `phf` compiled tables from `resources/services/*` |
| Router (Flint 4) | MediaTek **Filogic 880 (MT7988)**, quad **Cortex-A73**, 2 GB DDR4, **64 GB eMMC** ([specs](https://privacygear.nl/en/reviews/glinet-flint-4-preview/)) |
| OpenWrt target | `mediatek/filogic`, `aarch64` (cortex-a73) |
| OpenWrt 25.12 | Released 2026-03; **opkg → apk**, musl 1.2.5, gcc 14.3, kernel 6.12 ([release notes](https://lists.openwrt.org/pipermail/openwrt-announce/2026-March/000081.html)) |

## 3. The core insight

Sniffnet's desktop GUI cannot run on a headless router, but the authors already
separated the **capture/analysis engine** from the GUI:

- Engine: `src/networking/*` — capture loop (`parse_packets`), header analysis,
  aggregation (`InfoTraffic`), reverse-DNS pool, MMDB lookup, L7 service detection.
- Communication: the engine emits `BackendTrafficMessage::TickRun(usize, InfoTraffic, Vec<HostMessage>, bool)`
  once per second through an `async_channel` ([architecture breakdown](https://instagit.com/GyulyVGC/sniffnet/how-does-the-multi-threaded-packet-capture-backend-interact-with-the-gui.md)).

**So the port = keep the engine, replace `iced` with a web frontend, and bridge them
with a WebSocket carrying the same `TickRun` snapshots as JSON.**

## 4. Architecture

```
              OpenWrt 25.12 — Flint 4 (MT7988 / aarch64)
 ┌───────────────────────────────────────────────────────────────┐
 │  sniffnetwrt daemon (Rust)                                    │
 │  ┌──────────────────────────────┐   ┌────────────────────────┐ │
 │  │ Capture engine (Sniffnet's)  │   │ Web layer (new)        │ │
 │  │  • pcap on WAN iface         │──▶│  • HTTP :9999          │ │
 │  │  • header parse + aggregate  │   │  • WebSocket (JSON     │ │
 │  │  • reverse DNS / MMDB / L7   │   │    TickRun snapshots)  │ │
 │  │  • InfoTraffic state         │   │  • static web assets   │ │
 │  └──────────────────────────────┘   └────────────────────────┘ │
 └───────────────────────────────────────────────────────────────┘
        │ WebSocket (1 Hz snapshots + incremental updates)
        ▼
   Browser  →  static HTML/CSS/JS recreating the Sniffnet look
```

## 5. UI strategy (chosen: Option 1)

| # | Approach | Verdict |
|---|---|---|
| 1 | **Rebuild UI in web (HTML/CSS/JS + uPlot/Chart.js)** | ✅ **Chosen** — lowest CPU/RAM, full control of look |
| 2 | Compile `iced` UI to WASM and serve it | ⚠️ Pixel-perfect but risky; `plotters-iced2`+`wgpu` WASM is not first-class |
| 3 | Run GUI headless under Xvfb + stream (noVNC/WebRTC) | ❌ Heavy, janky on 2 GB router |

## 6. What we reuse vs. replace

**Reuse (from the vendored fork in `vendor/`):**
- `lib/sniffnet-packet-parser/**` — pure packet parsing (self-contained).
- `src/networking/**` — capture loop, `InfoTraffic`, `Host`, `HostMessage`,
  reverse-DNS pool, L7 service tables, `AddressPortPair`, `DataInfo`.
- `src/mmdb/**` + `src/countries/**` — geo/ASN + country types.
- `resources/services/*`, `resources/DB/*.mmdb` — service signatures + geo DBs.

**Replace / drop:**
- `iced`, `plotters`, `plotters-iced2`, `winit`, `wgpu`, `rodio`, `rfd`, `picon` → dropped.
- `src/gui/**`, `src/translations/**`, `src/chart/**`, `src/report/**`, `src/notifications/**`, `src/cli/**` → replaced by web frontend + daemon.
- Program/process identification (`src/networking/types/program*.rs`) → desktop-only; on the
  router, repurpose to **LAN-client identification** via ARP/neighbor table.

## 7. Feature triage

| Sniffnet feature | On router |
|---|---|
| Choose network adapter | Fixed to WAN (auto-detected) + UCI override |
| Filters (BPF) | Keep — pass BPF to `pcap` |
| Import/export PCAP | Keep export (ring buffer on eMMC); import optional |
| Overall statistics | Keep |
| Real-time charts | Keep (web charts) |
| Local network hosts | Keep (WAN side local = LAN clients) |
| Geo location / ASN / reverse DNS | Keep (GeoLite2 + `dns_lookup`) |
| 6000+ service identification | Keep |
| **Which programs generate bandwidth** | **Drop** → replace with LAN client/MAC view |
| Notifications | Keep (browser Notification/WebAudio for sound) |
| Custom themes / thumbnail mode | Keep (CSS themes; compact view) |

## 8. WAN capture on Flint 4

- OpenWrt WAN is logical: `wan`/`eth1`/`pppoe-wan` etc. Auto-detect at runtime:
  `ubus call network.interface.wan status` → read `device` / `l3_device`.
- For PPPoE, capture on the underlying **L2 device**, not `pppoe-wan`.
- Run as root (or `CAP_NET_RAW,cap_net_admin`) — procd handles this.
- Requires OpenWrt `libpcap` package.

## 9. Build & packaging (OpenWrt 25.12)

1. Cross-compile for `aarch64-unknown-linux-musl` against the OpenWrt SDK
   `STAGING_DIR` (so `pcap`/libpcap headers resolve). See `openwrt/cross-build.sh`.
2. Two shipping options:
   - **OpenWrt package Makefile** (`openwrt/Makefile`) — cleanest; integrates with `apk` + ASU.
   - **Static musl binary + `scp`** — fastest iteration (64 GB eMMC leaves room).
3. `procd` init script (`openwrt/sniffnetwrt.init`) + UCI config (`openwrt/sniffnetwrt.config`).
4. Bind to LAN (`192.168.x.1:9999`) with token/basic auth (it exposes LAN traffic data).
5. GeoLite2 needs a free MaxMind key; DB-IP Lite (CC-BY-4.0) is a no-license alternative.

## 10. Milestones

1. **Spike (1–2 d):** daemon captures WAN, prints `TickRun` JSON to stdout over SSH.
2. **Daemon (2–3 d):** `axum` + `tokio-tungstenite` on :9999, WAN auto-detect, UCI config.
3. **Frontend (3–5 d):** HTML/CSS/JS pages matching the screenshot (cards, live charts, table).
4. **Packaging (1–2 d):** procd init + apk package, MMDB download, auth.
5. **Polish:** notifications, themes, compact view.

## 11. Risks

- **Line-rate capture** on 10G SFP+ WAN would spike CPU; realistic WAN ≤ 2.5 Gbps and
  Sniffnet's header-only aggregation scales fine. Cap ring size if needed.
- **Vanilla vs. stock firmware** — engine works on both (both have ubus/netifd), but
  packaging differs (`apk` vs GL.iNet's `opkg`).
- **Rust edition 2024** requires a recent toolchain (rustup 1.85+).
- **libpcap cross-linking** is the main build friction; the OpenWrt package Makefile sidesteps it.
