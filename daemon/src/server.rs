//! HTTP server: static assets + `/ws` upgrade + `/healthz`.

use crate::model::Snapshot;
use crate::ws;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::collections::HashMap;
use std::sync::Arc;
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
}

pub fn build(state: AppState, web_dir: &str) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .route("/healthz", get(|| async { "ok\n" }))
        .nest_service("/", ServeDir::new(web_dir))
        .with_state(state)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Response {
    if let Some(token) = &state.token {
        if params.get("token").map(String::as_str) != Some(token.as_str()) {
            return (axum::http::StatusCode::UNAUTHORIZED, "invalid token").into_response();
        }
    }
    ws.on_upgrade(move |socket| ws::handle(socket, state))
}
