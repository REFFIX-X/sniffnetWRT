# OpenWrt package Makefile for sniffnetwrt.
#
# NOTE: this is a template. Building Rust packages under the OpenWrt buildroot
# requires the `rust` host package (feeds/packages lang/rust) and vendored crates
# (`cargo vendor`) for offline builds. Validate this against the OpenWrt rust
# package conventions before relying on it — the fast path for development is
# `openwrt/cross-build.sh` + `scp` (see README).

include $(TOPDIR)/rules.mk

PKG_NAME:=sniffnetwrt
PKG_VERSION:=0.1.0
PKG_RELEASE:=1

PKG_LICENSE:=MIT OR Apache-2.0
PKG_MAINTAINER:=you <you@example.com>

PKG_BUILD_DEPENDS:=rust/host
PKG_BUILD_DIR:=$(BUILD_DIR)/$(PKG_NAME)-$(PKG_VERSION)

include $(INCLUDE_DIR)/package.mk

define Package/sniffnetwrt
  SECTION:=net
  CATEGORY:=Network
  TITLE:=Sniffnet-style WAN traffic monitor (web UI on port 9999)
  URL:=https://github.com/GyulyVGC/sniffnet
  DEPENDS:=+libpcap
endef

define Package/sniffnetwrt/description
  Headless, Sniffnet-style real-time traffic monitor that captures the WAN
  interface and serves a live dashboard over HTTP/WebSocket on port 9999.
endef

define Build/Prepare
	mkdir -p $(PKG_BUILD_DIR)
	$(CP) ./daemon/* $(PKG_BUILD_DIR)/
endef

define Build/Compile
	( cd $(PKG_BUILD_DIR) && \
	  CARGO_HOME=$(STAGING_DIR_HOST)/share/cargo \
	  cargo build --release --offline --target $(RUST_ARCH) )
endef

define Package/sniffnetwrt/install
	$(INSTALL_DIR) $(1)/usr/bin
	$(INSTALL_BIN) $(PKG_BUILD_DIR)/target/$(RUST_ARCH)/release/sniffnetwrt $(1)/usr/bin/sniffnetwrt
	$(INSTALL_DIR) $(1)/usr/share/sniffnetwrt
	$(CP) ./web/* $(1)/usr/share/sniffnetwrt/
	$(INSTALL_DIR) $(1)/etc/init.d
	$(INSTALL_BIN) ./files/sniffnetwrt.init $(1)/etc/init.d/sniffnetwrt
	$(INSTALL_DIR) $(1)/etc/config
	$(INSTALL_CONF) ./files/sniffnetwrt.config $(1)/etc/config/sniffnetwrt
endef

$(eval $(call BuildPackage,sniffnetwrt))
