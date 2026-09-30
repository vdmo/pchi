//! Web server for PCHI Conductor dashboard

use crate::{GovernanceLog, SceneState};
use axum::{
    extract::{State, ws::WebSocket, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Json},
    routing::get,
    Router,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use futures_util::StreamExt;
use serde_json::json;

/// Web server state
#[derive(Clone)]
pub struct WebServerState {
    /// Current scene state
    pub scene_state: Arc<RwLock<SceneState>>,
    /// Signed, hash-chained governance receipt log. `None` if this server
    /// was built without governance wiring (kept `Option` so the struct
    /// stays constructible from just a scene-state arc, as it always has
    /// been, for anyone already embedding it that way).
    pub governance: Option<Arc<GovernanceLog>>,
}

/// Create web server router
pub fn create_router(state: WebServerState) -> Router {
    Router::new()
        .route("/", get(dashboard_handler))
        .route("/ws", get(websocket_handler))
        .route("/governance/export", get(governance_export_handler))
        .with_state(state)
}

/// GET /governance/export — the signed, hash-chained batch of every rule
/// firing this conductor has recorded. Guarded by `PCHI_GOVERNANCE_KEY`
/// (sent as `X-Governance-Key`) when that env var is set; open otherwise,
/// since this conductor is meant to be self-hosted on a machine the
/// operator already controls — unlike DGV's `/decisions/export` (a
/// multi-tenant service other people's agents call), there's no default
/// audience this needs to be closed against. Set the key anyway before
/// exposing this conductor beyond localhost.
async fn governance_export_handler(
    State(state): State<WebServerState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let Some(gov) = &state.governance else {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(json!({"error": "governance_not_configured"})),
        )
            .into_response();
    };
    if let Ok(required) = std::env::var("PCHI_GOVERNANCE_KEY") {
        if !required.is_empty() {
            let provided = headers.get("X-Governance-Key").and_then(|v| v.to_str().ok());
            if provided != Some(required.as_str()) {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": "governance_key_required", "hint": "set X-Governance-Key"})),
                )
                    .into_response();
            }
        }
    }
    (StatusCode::OK, Json(gov.export())).into_response()
}

/// Serve the dashboard HTML
async fn dashboard_handler() -> Html<&'static str> {
    Html(include_str!("../web-dashboard/index.html"))
}

/// WebSocket handler for real-time updates
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<WebServerState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_websocket(socket, state))
}

/// Handle WebSocket connection
async fn handle_websocket(mut socket: WebSocket, state: WebServerState) {
    // Send initial state
    let scene_state = state.scene_state.read().await;
    let initial_msg = json!({
        "type": "state_update",
        "state": {
            "objects": scene_state.objects,
            "musical_context": scene_state.musical_context,
            "equilibrium": match &scene_state.equilibrium_status {
                crate::state::EquilibriumStatus::Equilibrium => 0.0,
                crate::state::EquilibriumStatus::Disequilibrium { residual } => *residual,
                crate::state::EquilibriumStatus::Unknown => 0.0,
            },
            "coherence_gap": scene_state.calculate_equilibrium(),
        }
    });

    if let Ok(msg) = serde_json::to_string(&initial_msg) {
        let _ = socket.send(axum::extract::ws::Message::Text(msg)).await;
    }
    drop(scene_state);

    // Handle incoming messages
    while let Some(result) = socket.next().await {
        match result {
            Ok(msg) => {
                if let axum::extract::ws::Message::Text(text) = msg {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                        handle_client_message(data, &mut socket, &state).await;
                    }
                }
            }
            Err(e) => {
                tracing::warn!("WebSocket error: {}", e);
                break;
            }
        }
    }
}

/// Handle client messages
async fn handle_client_message(
    data: serde_json::Value,
    socket: &mut WebSocket,
    state: &WebServerState,
) {
    let msg_type = data.get("type").and_then(|v| v.as_str()).unwrap_or("unknown");

    match msg_type {
        "get_state" => {
            let scene_state = state.scene_state.read().await;
            let response = json!({
                "type": "state_update",
                "state": {
                    "objects": scene_state.objects,
                    "musical_context": scene_state.musical_context,
                    "equilibrium": match &scene_state.equilibrium_status {
                        crate::state::EquilibriumStatus::Equilibrium => 0.0,
                        crate::state::EquilibriumStatus::Disequilibrium { residual } => *residual,
                        crate::state::EquilibriumStatus::Unknown => 0.0,
                    },
                    "coherence_gap": scene_state.calculate_equilibrium(),
                }
            });
            let _ = socket.send(axum::extract::ws::Message::Text(
                serde_json::to_string(&response).unwrap_or_default()
            )).await;
        }
        _ => {
            tracing::debug!("Unknown message type: {}", msg_type);
        }
    }
}

/// Broadcast state update to all connected clients (placeholder)
pub async fn broadcast_state_update(_state: &WebServerState) {
    // In a real implementation, this would maintain a list of connected clients
    // and broadcast to all of them. For now, this is a placeholder.
    tracing::debug!("Broadcasting state update");
}
