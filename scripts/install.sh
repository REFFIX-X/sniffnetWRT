#!/bin/sh
# sniffnetWRT installer for OpenWrt (works with apk and opkg).
#
# Usage:
#   ./install.sh                                      # install latest release
#   ./install.sh ./sniffnetwrt-aarch64.tar.gz         # install a local package
#   ./install.sh https://example/sniffnetwrt.tar.gz   # install a package URL
#
# The tarball must contain: sniffnetwrt (binary), web/, sniffnetwrt.init,
# sniffnetwrt.config, and install.sh — exactly what the GitHub Actions build produces.

set -eu

RELEASE_URL="https://github.com/REFFIX-X/sniffnetWRT/releases/latest/download/sniffnetwrt-aarch64.tar.gz"
SRC="${1:-$RELEASE_URL}"
WEB_DIR=/usr/share/sniffnetwrt
BIN=/usr/bin/sniffnetwrt
INIT=/etc/init.d/sniffnetwrt
CFG=/etc/config/sniffnetwrt

if [ "$(id -u)" -ne 0 ]; then
	echo "error: run this installer as root (SSH to the router as root)" >&2
	exit 1
fi

ARCH="$(uname -m)"
case "$ARCH" in
	aarch64|arm64) ;;
	*)
		echo "error: this release is built for 64-bit ARM (aarch64); detected $ARCH" >&2
		exit 1
		;;
esac

if [ -r /etc/openwrt_release ]; then
	. /etc/openwrt_release
	case "${DISTRIB_RELEASE:-}" in
		25.12*) ;;
		'') echo "warning: could not identify the OpenWrt version" >&2 ;;
		*) echo "warning: this release is built for OpenWrt 25.12; detected ${DISTRIB_RELEASE}" >&2 ;;
	esac
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

case "$SRC" in
	http://*|https://*)
		echo "Downloading $SRC ..."
		wget -qO "$TMP/sniffnetwrt.tar.gz" "$SRC" \
			|| { echo "error: download failed: $SRC" >&2; exit 1; }
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
	apk -U add libpcap
elif command -v opkg >/dev/null 2>&1; then
	opkg update >/dev/null 2>&1 || true
	opkg install libpcap
else
	echo "error: neither apk nor opkg is available; cannot install libpcap" >&2
	exit 1
fi

echo "Installing files..."
install -Dm755 "$BASE/sniffnetwrt" "$BIN"
mkdir -p "$WEB_DIR"
cp -r "$BASE"/web/. "$WEB_DIR"/
[ -f "$BASE/sniffnetwrt.init" ] && install -Dm755 "$BASE/sniffnetwrt.init" "$INIT"
# Don't clobber an existing config.
NEW_CONFIG=0
if [ -f "$BASE/sniffnetwrt.config" ] && [ ! -f "$CFG" ]; then
	install -Dm644 "$BASE/sniffnetwrt.config" "$CFG"
	NEW_CONFIG=1
fi

# Bind to the LAN IP by default so the dashboard isn't reachable from the WAN.
if [ "$NEW_CONFIG" -eq 1 ] && command -v uci >/dev/null 2>&1 && [ -f "$CFG" ]; then
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
echo "  Configure:  uci show sniffnetwrt   (token, webhook, pcap, ...)"
echo "  Restart:    /etc/init.d/sniffnetwrt restart"
echo "  Logs:       logread | grep sniffnetwrt"
