//! HTTP server: static assets + `/ws` upgrade + `/healthz` + `/pcap`.

use crate::model::Snapshot;
use crate::security::EventBuffer;
use crate::ws;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Query, State};
use axum::body::{Body, Bytes};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::sync::{broadcast, RwLock};
use tower_http::services::ServeDir;

#[derive(Clone)]
pub struct AppState {
    /// Broadcast channel the capture thread publishes snapshots to.
    pub tx: broadcast::Sender<Snapshot>,
    /// Latest snapshot, serialized as JSON, for instant delivery on connect.
    pub last: Arc<RwLock<Option<String>>>,
    /// Optional bearer token required on the `/ws` connection.
    pub token: Option<String>,
    /// Optional path of the live PCAP export, served at `/pcap`.
    pub pcap_path: Option<String>,
    /// In-memory firewall event history, clearable from the dashboard.
    pub security_events: EventBuffer,
}

pub fn build(state: AppState, web_dir: &str) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .route("/healthz", get(|| async { "ok\n" }))
        .route("/pcap", get(pcap_handler))
        .route("/security/clear", post(clear_security_handler))
        .nest_service("/", ServeDir::new(web_dir))
        .with_state(state)
}

async fn clear_security_handler(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Response {
    if let Some(token) = &state.token {
        if params.get("token").map(String::as_str) != Some(token.as_str()) {
            return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
        }
    }
    state.security_events.write().await.clear();
    StatusCode::NO_CONTENT.into_response()
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Response {
    if let Some(token) = &state.token {
        if params.get("token").map(String::as_str) != Some(token.as_str()) {
            return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
        }
    }
    ws.on_upgrade(move |socket| ws::handle(socket, state))
}

async fn pcap_handler(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Response {
    if let Some(token) = &state.token {
        if params.get("token").map(String::as_str) != Some(token.as_str()) {
            return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
        }
    }
    let Some(path) = state.pcap_path.clone() else {
        return (StatusCode::NOT_FOUND, "PCAP export is disabled (set --pcap-out)").into_response();
    };
    match tokio::fs::File::open(&path).await {
        Ok(file) => {
            let length = match file.metadata().await {
                Ok(metadata) => metadata.len(),
                Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Could not inspect PCAP export").into_response(),
            };
            // Stream captures in bounded chunks instead of loading the complete
            // PCAP into router memory. Bound to the current file length so a
            // live capture that keeps growing does not create an endless download.
            let stream = futures_util::stream::try_unfold((file, length), |(mut file, remaining)| async move {
                if remaining == 0 {
                    return Ok::<_, std::io::Error>(None);
                }
                let mut chunk = vec![0; remaining.min(64 * 1024) as usize];
                match file.read(&mut chunk).await? {
                    0 => Ok(None),
                    n => {
                        chunk.truncate(n);
                        Ok(Some((Bytes::from(chunk), (file, remaining - n as u64))))
                    }
                }
            });
            let mut resp = Body::from_stream(stream).into_response();
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("application/vnd.tcpdump.pcap"),
            );
            resp
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            (StatusCode::NOT_FOUND, "PCAP not available yet").into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Could not open PCAP export").into_response(),
    }
}
