//! sniffnetWRT — Sniffnet-style WAN traffic monitor for OpenWrt.

mod bridge;
mod config;
mod model;
mod server;
mod sysinfo;
mod wan;
mod webhook;
mod ws;

use anyhow::Context;
use clap::Parser;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = config::Config::parse();

    let device = wan::detect_wan_device(config.interface.as_deref())
        .context("WAN detection failed")?;
    tracing::info!(device, "capturing on WAN interface");

    let lan_device = wan::detect_lan_device(config.lan_device.as_deref());
    if let Some(lan) = &lan_device {
        tracing::info!(lan, "capturing on LAN bridge for per-device accounting");
    }

    // Bind first so misconfiguration fails fast.
    let bind = format!("{}:{}", config.bind, config.port);
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("failed to bind {bind}"))?;
    tracing::info!(%bind, web_dir = %config.web_dir, "serving sniffnetWRT web UI");

    // Shared channel between the capture bridge and WebSocket sessions.
    let (tx, _rx) = broadcast::channel::<model::Snapshot>(256);

    // Run the engine + bridge on a tokio task.
    {
        let tx = tx.clone();
        let bpf = config.bpf.clone();
        let mmdb_dir = config.mmdb_dir.clone();
        let blacklist = config.blacklist.clone();
        let webhook = config.webhook.clone();
        let notify_threshold = config.notify_threshold;
        let pcap_out = config.pcap_out.clone();
        tokio::spawn(async move {
            if let Err(e) = bridge::run(
                device,
                lan_device,
                bpf,
                mmdb_dir,
                blacklist,
                webhook,
                notify_threshold,
                pcap_out,
                tx,
            )
            .await
            {
                tracing::error!("capture bridge failed: {e}");
            }
        });
    }

    let state = server::AppState {
        tx,
        last: Arc::new(RwLock::new(None)),
        token: config.token.clone(),
        pcap_path: config.pcap_out.clone(),
    };

    axum::serve(listener, server::build(state, &config.web_dir)).await?;
    Ok(())
}
