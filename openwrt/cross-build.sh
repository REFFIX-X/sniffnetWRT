#!/bin/sh
# Cross-compile the sniffnetwrt daemon for OpenWrt using the OpenWrt SDK.
#
# Prerequisites:
#   1. Extract the matching OpenWrt SDK, e.g.:
#      openwrt-sdk-25.12.0-mediatek-filogic_gcc-14.3.0_musl.Linux-x86_64.tar.zst
#   2. rustup with the aarch64 musl target.
#
# Usage:
#   SDK_DIR=/path/to/openwrt-sdk ./openwrt/cross-build.sh
#
# The binary is built *dynamically* linked against OpenWrt's musl + libpcap,
# so the target device needs the `libpcap` package installed (`apk add libpcap`).

set -eu

SDK_DIR="${SDK_DIR:?set SDK_DIR to the extracted OpenWrt SDK directory}"
TARGET="aarch64-unknown-linux-musl"

STAGING_DIR="$SDK_DIR/staging_dir"
TOOLCHAIN_DIR="$(ls -d "$STAGING_DIR"/toolchain-* 2>/dev/null | head -n1)"
TARGET_DIR="$(ls -d "$STAGING_DIR"/target-* 2>/dev/null | head -n1)"

if [ -z "$TOOLCHAIN_DIR" ] || [ -z "$TARGET_DIR" ]; then
	echo "error: could not locate toolchain/target dirs under $STAGING_DIR" >&2
	exit 1
fi

export STAGING_DIR
rustup target add "$TARGET"

# Use the OpenWrt musl cross-gcc as the linker so the pcap crate links against
# the SDK's libpcap; force dynamic CRT (matching OpenWrt's own binaries).
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER="$TOOLCHAIN_DIR/bin/aarch64-openwrt-linux-musl-gcc"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUSTFLAGS="-C target-feature=-crt-static"

# Point pkg-config at the SDK's target sysroot (libpcap headers + .pc files).
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_SYSROOT_DIR="$TARGET_DIR"
export PKG_CONFIG_LIBDIR="$TARGET_DIR/usr/lib/pkgconfig:$TARGET_DIR/usr/share/pkgconfig"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR/../daemon"

cargo build --release --target "$TARGET"

echo
echo "Built: $(pwd)/target/$TARGET/release/sniffnetwrt"
echo "Copy it to the router:  scp target/$TARGET/release/sniffnetwrt root@192.168.1.1:/tmp/"
