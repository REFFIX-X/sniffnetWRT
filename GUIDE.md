# sniffnetWRT — Guide

A Sniffnet-style, real-time **router traffic dashboard** for OpenWrt. It captures the
**WAN** interface (and the **LAN** bridge for per-device usage), and serves a live
dark dashboard over **HTTP/WebSocket on port 9999**.

---

## 1. Get the aarch64 binary

You don't need Rust or a Linux machine — GitHub Actions builds it for you.

1. Push to GitHub (the workflow in `.github/workflows/build.yml` runs automatically).
2. Open **Actions → the finished (green) run → Artifacts → `sniffnetwrt-aarch64`**.
3. Download it. The archive contains:
   - `sniffnetwrt` — the binary
   - `web/` — the dashboard
   - `sniffnetwrt.init` + `sniffnetwrt.config` — service files
   - `install.sh` — the installer

## 2. Install on the router

### Option A — one command (recommended)

Extract the installer from the archive, copy it with the archive to the router,
then run it there:

```sh
tar xzf sniffnetwrt-aarch64.tar.gz install.sh
scp install.sh sniffnetwrt-aarch64.tar.gz root@192.168.1.1:/tmp/
ssh root@192.168.1.1 "sh /tmp/install.sh /tmp/sniffnetwrt-aarch64.tar.gz"
```

The installer: installs `libpcap`, copies the binary + web + init + config, binds
the UI to your LAN IP, and starts the service.

### Option B — manual

```sh
ssh root@192.168.1.1
apk add libpcap                                     # opkg install libpcap on older OpenWrt

tar xzf sniffnetwrt-aarch64.tar.gz
mv sniffnetwrt /usr/bin/sniffnetwrt
mkdir -p /usr/share/sniffnetwrt && cp -r web/* /usr/share/sniffnetwrt/
cp sniffnetwrt.init /etc/init.d/sniffnetwrt && chmod +x /etc/init.d/sniffnetwrt
cp sniffnetwrt.config /etc/config/sniffnetwrt

/etc/init.d/sniffnetwrt enable
/etc/init.d/sniffnetwrt start
```

## 3. Open it

```
http://192.168.1.1:9999
```

(use your router's LAN IP — `uci get network.lan.ipaddr`). The WAN interface is
auto-detected via `ubus`; on the Flint 4 it's `eth2`.

## 4. Configure (`/etc/config/sniffnetwrt`)

| Option | Meaning |
|---|---|
| `device` | WAN capture device (empty = auto-detect) |
| `lan_device` | LAN bridge for per-device usage (default `br-lan`) |
| `port` | Web UI port (default 9999) |
| `bind` | Listen address — **set to your LAN IP** (installer does this) |
| `token` | optional `?token=` auth on the WebSocket and PCAP download |
| `bpf` | optional BPF filter (e.g. `tcp port 443`) |
| `mmdb_dir` | dir with `GeoLite2-{Country,ASN}.mmdb` |
| `webhook` | remote-notification webhook URL |
| `notify_threshold` | alert above this many B/s |
| `pcap_out` | path for a live PCAP export (download at `/pcap`) |

Apply with:

```sh
uci commit sniffnetwrt && /etc/init.d/sniffnetwrt restart
```

## 5. Features

- **Overview** — health (load/RAM/temp/uptime), WAN status, live throughput chart,
  totals, service breakdown.
- **Connections** — search + filter by direction or service category, GeoIP country
  flags, and expandable per-connection detail.
- **Devices** — LAN clients with live per-device **down/up** (from the `br-lan` capture).
- **Settings** — themes (Footstrap default), browser alerts + rate threshold.

Backend extras: remote webhook notifications for rate thresholds, live PCAP export,
reverse-DNS + GeoLite2
country/ASN, and Sniffnet's 6000+ service signatures.

## 6. Geo / ASN (optional)

Drop `GeoLite2-{Country,ASN}.mmdb` into `mmdb_dir` (a free [MaxMind](https://www.maxmind.com/en/geolite2/signup) account):

```sh
MAXMIND_LICENSE_KEY=xxxx ./scripts/fetch-geo.sh db
scp db/*.mmdb root@192.168.1.1:/usr/share/sniffnetwrt/
```

Without them the dashboard still works — you just won't see country flags / ASN names.

## 7. Troubleshooting

```sh
# is it running?
/etc/init.d/sniffnetwrt status
logread | grep sniffnetwrt          # logs

# test the endpoints
curl -s http://192.168.1.1:9999/healthz     # -> ok
curl -s 'http://192.168.1.1:9999/pcap?token=YOUR_TOKEN' -o x.pcap   # include token if configured

# run in the foreground to see errors
/etc/init.d/sniffnetwrt stop
/usr/bin/sniffnetwrt --web-dir /usr/share/sniffnetwrt --port 9999
```

**Common issues**
- **No traffic / blank dashboard** → check the WAN device: `ubus call network.interface.wan status` and set `option device`.
- **Can't reach :9999** → `bind` is set to the LAN IP and you're on the LAN; or a firewall rule blocks it.
- **No per-device bytes** → `lan_device` must be a real bridge (`br-lan`); check `ip link show br-lan`.
