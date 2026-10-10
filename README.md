# sniffnetWRT

A **headless, Sniffnet-style real-time traffic monitor for OpenWrt** that captures
the router's **WAN** interface and serves a live dark dashboard over **HTTP/WebSocket
on port 9999**.

Built by forking [GyulyVGC/sniffnet](https://github.com/GyulyVGC/sniffnet)
(MIT OR Apache-2.0): we keep its packet engine and replace the `iced` desktop GUI
with a web frontend.

## Layout

```
sniffnetWRT/
├── PLAN.md                 # the full porting plan
├── PORTING.md              # how to wire the vendored Sniffnet engine into the daemon
├── daemon/                 # Rust daemon (capture + HTTP/WebSocket server)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         # entry point
│       ├── config.rs       # CLI/env config
│       ├── wan.rs          # WAN interface auto-detection (ubus + UCI)
│       ├── bridge.rs       # engine → WebSocket bridge (+ health/WAN/devices)
│       ├── sysinfo.rs      # CPU/RAM/temp/uptime + WAN + LAN-client discovery
│       ├── webhook.rs      # remote webhook notifications
│       ├── model.rs        # JSON snapshot schema (mirrors Sniffnet's InfoTraffic)
│       ├── server.rs       # axum router (static assets + /ws + /healthz)
│       └── ws.rs           # WebSocket session relay
├── web/                    # frontend (vanilla HTML/CSS/JS, no build step)
│   ├── index.html
│   ├── style.css
│   └── app.js              # dashboard + live charts + demo fallback
├── openwrt/                # packaging
│   ├── sniffnetwrt.init    # procd init script
│   ├── sniffnetwrt.config  # UCI default config
│   ├── cross-build.sh      # cross-compile with the OpenWrt SDK
│   └── Makefile            # OpenWrt package template
└── vendor/                 # vendored upstream Sniffnet source (reference fork)
    ├── sniffnet-src/       #   upstream src/
    └── sniffnet-lib/       #   upstream lib/sniffnet-packet-parser
```

## Quick start (local dev, desktop Linux/macOS)

The daemon needs raw-socket access to an interface, so run it as root (or grant
`cap_net_raw`).

```sh
cd daemon
cargo build --release

# serve the dashboard, capturing eth0 (or your WAN device)
sudo ./target/release/sniffnetwrt --interface eth0 --web-dir ../web --port 9999
```

Open http://127.0.0.1:9999 — you'll see the live dashboard. Without the daemon
(or if the WebSocket can't connect within 2.5 s), the page falls back to a **demo
mode** so you can preview the look.

> The `pcap` crate links the system libpcap. On Debian/Ubuntu:
> `sudo apt install libpcap-dev`. On OpenWrt 25.12: `apk -U add libpcap`.

## Install on a Flint 4 (OpenWrt 25.12)

The ready-to-install release targets the GL.iNet Flint 4 (MT7988, aarch64)
running OpenWrt 25.12.x. From a computer with SSH access, connect as `root`:

```sh
ssh root@192.168.1.1
```

Then run these commands in the router's shell:

```sh
wget -O /tmp/sniffnetwrt-install.sh https://raw.githubusercontent.com/REFFIX-X/sniffnetWRT/main/scripts/install.sh
sh /tmp/sniffnetwrt-install.sh
```

The installer downloads the latest release, installs `libpcap`, puts the app
files in place, and enables and starts the service. It preserves an existing
`/etc/config/sniffnetwrt`; on a fresh install, it binds the dashboard to the
router's LAN IP when available. Open `http://192.168.1.1:9999` (replace the
address with your router's LAN IP if it differs).

To upgrade, run the same two commands again. To install a specific tarball
instead of the latest release, pass its path or URL to the installer:

```sh
sh /tmp/sniffnetwrt-install.sh https://github.com/REFFIX-X/sniffnetWRT/releases/download/v1.0.0/sniffnetwrt-aarch64.tar.gz
```

Traffic statistics stay in memory. PCAP recording is off by default, so the
installer does not set up continuous writes of traffic captures to router flash.

If the dashboard does not load, check the service log with
`logread | grep sniffnetwrt` and confirm your LAN address with
`uci get network.lan.ipaddr`.

## Build with zero local toolchain (GitHub Actions)

No Rust, no SDK, no Linux VM needed. Push this repo to GitHub and the included
[`.github/workflows/build.yml`](.github/workflows/build.yml) cross-compiles the
aarch64 binary on GitHub's free Ubuntu runners and uploads
`sniffnetwrt-aarch64.tar.gz` as an artifact.

```sh
git init && git add -A && git commit -m "sniffnetWRT"
git remote add origin https://github.com/<you>/sniffnetWRT.git
git push -u origin main
```

Then: **repo → Actions → the run → Artifacts → download `sniffnetwrt-aarch64`**.
It bundles `sniffnetwrt`, `web/`, the init/config files, and `install.sh` for a
self-contained install.

To publish a GitHub Release with that tarball attached, push a version tag after
the commit has built successfully:

```sh
git tag v1.0.0
git push origin v1.0.0
```

The tag workflow builds the same OpenWrt package and publishes
`sniffnetwrt-aarch64.tar.gz` on the release. The installer can download the
latest release directly:
`https://github.com/REFFIX-X/sniffnetWRT/releases/latest/download/sniffnetwrt-aarch64.tar.gz`.

> First run may need a tiny tweak (e.g. the `SDK_URL` in `build.yml`). The
> workflow log shows the exact failure; the fix is usually editing that one URL.

## Configuration

| Flag | Env | Default | Meaning |
|---|---|---|---|
| `--interface` | `SNIFFNETWRT_INTERFACE` | auto-detect | WAN capture device |
| `--lan-device` | `SNIFFNETWRT_LAN_DEVICE` | auto-detect | LAN bridge for per-device bandwidth; empty disables it |
| `--bind` | `SNIFFNETWRT_BIND` | `0.0.0.0` | listen address (use LAN IP) |
| `--port` | `SNIFFNETWRT_PORT` | `9999` | web/WS port |
| `--web-dir` | `SNIFFNETWRT_WEB_DIR` | `web` | static assets directory |
| `--mmdb-dir` | `SNIFFNETWRT_MMDB_DIR` | `db` | dir with GeoLite2-{Country,ASN}.mmdb |
| `--token` | `SNIFFNETWRT_TOKEN` | — | required `?token=` on `/ws` |
| `--bpf` | `SNIFFNETWRT_BPF` | — | optional BPF filter |
| `--webhook` | `SNIFFNETWRT_WEBHOOK` | — | remote-notification webhook URL |
| `--notify-threshold` | `SNIFFNETWRT_NOTIFY_THRESHOLD` | — | alert above this many B/s |
| `--pcap-out` | `SNIFFNETWRT_PCAP_OUT` | — | write captured packets to a PCAP file served at `/pcap` |
| `--interval-ms` | `SNIFFNETWRT_INTERVAL_MS` | `1000` | snapshot interval |

Traffic statistics stay in memory by default. PCAP recording is disabled unless
`--pcap-out` (or the UCI `pcap_out` option) is explicitly set; avoid pointing it
at internal flash for continuous captures.

When `--token` is configured, both `/ws?token=...` and `/pcap?token=...` require
that token.

## Status

The **vendored Sniffnet engine is wired in** (`engine/` = headless extraction of
upstream `networking` + `mmdb` + `countries`), and the web dashboard has grown
into a full router traffic monitor:

- header parsing via `sniffnet-packet-parser`,
- the 6000+ L7 **service signatures** (compiled from `engine/resources/services/`),
- **reverse-DNS** domains + **GeoLite2 country/ASN** enrichment (`--mmdb-dir db`),
- **remote webhook notifications** on rate thresholds (`--webhook`),
- router **health** (load/RAM/temp/uptime), **WAN status**, and **LAN clients**,
- dashboard tabs (Overview / Connections / Devices / Settings) with **search,
  filters, themes, service categories, GeoIP flags, and per-connection detail**.

Still desktop-only (not ported): `iced` GUI, IPFIX, program/process
identification, sound, and thumbnail mode. A flow diagram remains a possible
future feature. See [`PORTING.md`](PORTING.md).

## License

MIT OR Apache-2.0 (same as upstream Sniffnet).
