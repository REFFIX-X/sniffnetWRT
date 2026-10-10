# sniffnetWRT install and quick guide

sniffnetWRT is a live traffic dashboard for OpenWrt 25.12 on aarch64 routers,
including the GL.iNet Flint 4. The installer downloads the latest GitHub Release,
installs the `libpcap` runtime dependency, sets up the procd service, and starts
the dashboard. No Rust toolchain or package archive handling is needed on the router.

## Install or upgrade

SSH to the router as root and run:

```sh
wget -O /tmp/sniffnetwrt-install.sh https://raw.githubusercontent.com/REFFIX-X/sniffnetWRT/main/scripts/install.sh
sh /tmp/sniffnetwrt-install.sh
```

Open `http://192.168.1.1:9999` (replace the IP with the router's LAN address).
The installer preserves an existing `/etc/config/sniffnetwrt`. For a fresh install
it sets the bind address to the LAN IP when UCI can read it. Keep the web port
blocked from untrusted networks; use the OpenWrt firewall and a LAN-only bind.

To install a specific release, pass its tarball URL:

```sh
sh /tmp/sniffnetwrt-install.sh https://github.com/REFFIX-X/sniffnetWRT/releases/download/v1.2.0/sniffnetwrt-aarch64.tar.gz
```

To opt into rate-limited WAN firewall rejection events during install:

```sh
sh /tmp/sniffnetwrt-install.sh --enable-firewall-logging
```

This is optional. It enables WAN-zone logging at 10 messages per minute; recent
events are kept in RAM and are cleared when the service restarts.

## Configure and operate

```sh
uci show sniffnetwrt
uci set sniffnetwrt.wan.interval_ms='5000'  # WebSocket updates at most every 5 seconds
uci commit sniffnetwrt
/etc/init.d/sniffnetwrt restart
```

The capture engine continues sampling once per second. The `interval_ms` option
controls the minimum interval at which snapshots are sent to connected browsers.
The dashboard also has a browser-local refresh setting. Traffic totals and
firewall event history are held in memory by default. PCAP writing is disabled
unless `pcap_out` is explicitly configured; avoid setting it to router flash for
continuous capture.

Useful checks:

```sh
/etc/init.d/sniffnetwrt status
logread | grep sniffnetwrt
curl -s http://127.0.0.1:9999/healthz
ubus call network.interface.wan status
```

For interface overrides, authentication token configuration, GeoIP databases,
firewall event details, and development builds, see the [README](README.md).
