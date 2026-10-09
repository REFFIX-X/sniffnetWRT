//! WebSocket session: relay capture snapshots to one connected browser.

use crate::server::AppState;
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;

pub async fn handle(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.tx.subscribe();

    // Deliver the most recent snapshot immediately so the UI isn't empty.
    if let Some(last) = state.last.read().await.clone() {
        if sender.send(Message::Text(last.into())).await.is_err() {
            return;
        }
    }

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(snap) => {
                        if let Ok(json) = serde_json::to_string(&snap) {
                            let _ = state.last.write().await.replace(json.clone());
                            if sender.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(p))) => {
                        let _ = sender.send(Message::Pong(p)).await;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
        }
    }
}
