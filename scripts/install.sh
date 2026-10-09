#!/bin/sh
# sniffnetWRT installer for OpenWrt (works with apk and opkg).
#
# Usage:
#   ./install.sh ./sniffnetwrt-aarch64.tar.gz
#   ./install.sh https://github.com/REFFIX-X/sniffnetWRT/releases/latest/download/sniffnetwrt-aarch64.tar.gz
#
# The tarball must contain: sniffnetwrt (binary), web/, sniffnetwrt.init,
# sniffnetwrt.config — exactly what the GitHub Actions build produces.

set -eu

SRC="${1:-}"
WEB_DIR=/usr/share/sniffnetwrt
BIN=/usr/bin/sniffnetwrt
INIT=/etc/init.d/sniffnetwrt
CFG=/etc/config/sniffnetwrt

if [ -z "$SRC" ]; then
	echo "usage: $0 <tarball-or-url>" >&2
	exit 1
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

case "$SRC" in
	http://*|https://*)
		echo "Downloading $SRC ..."
		wget -qO "$TMP/sniffnetwrt.tar.gz" "$SRC" \
			|| { echo "download failed (try curl -L "$SRC" -o s.tar.gz)"; exit 1; }
		;;
	*)
		cp "$SRC" "$TMP/sniffnetwrt.tar.gz"
		;;
esac

echo "Extracting..."
tar -xzf "$TMP/sniffnetwrt.tar.gz" -C "$TMP"

# Locate the extracted files (the tarball may or may not have a top-level dir).
EXTRACT="$(find "$TMP" -maxdepth 3 -name sniffnetwrt -type f | head -n1)"
[ -n "$EXTRACT" ] || { echo "error: 'sniffnetwrt' binary not found in the tarball" >&2; exit 1; }
BASE="$(dirname "$EXTRACT")"

# Install libpcap (the binary's only runtime dependency).
if command -v apk >/dev/null 2>&1; then
	apk add libpcap
elif command -v opkg >/dev/null 2>&1; then
	opkg update >/dev/null 2>&1 || true
	opkg install libpcap
else
	echo "warning: no package manager found; install libpcap manually"
fi

echo "Installing files..."
install -Dm755 "$BASE/sniffnetwrt" "$BIN"
mkdir -p "$WEB_DIR"
cp -r "$BASE"/web/. "$WEB_DIR"/
[ -f "$BASE/sniffnetwrt.init" ] && install -Dm755 "$BASE/sniffnetwrt.init" "$INIT"
# Don't clobber an existing config.
[ -f "$BASE/sniffnetwrt.config" ] && [ ! -f "$CFG" ] && install -Dm644 "$BASE/sniffnetwrt.config" "$CFG"

# Bind to the LAN IP by default so the dashboard isn't reachable from the WAN.
if command -v uci >/dev/null 2>&1 && [ -f "$CFG" ]; then
	LAN_IP="$(uci get network.lan.ipaddr 2>/dev/null || true)"
	[ -n "$LAN_IP" ] && uci set sniffnetwrt.wan.bind="$LAN_IP" && uci commit sniffnetwrt
fi

if [ -x "$INIT" ]; then
	echo "Enabling + starting service..."
	"$INIT" enable
	"$INIT" restart
else
	echo "warning: init script not installed; start manually:"
	echo "  $BIN --web-dir $WEB_DIR --port 9999"
fi

echo
echo "✓ sniffnetWRT installed"
echo "  Dashboard:  http://<router-lan-ip>:9999"
echo "  Configure:  uci show sniffnetwrt   (token, blacklist, webhook, pcap, ...)"
echo "  Restart:    /etc/init.d/sniffnetwrt restart"
echo "  Logs:       logread | grep sniffnetwrt"
