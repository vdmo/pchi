//! Web server for PCHI Conductor dashboard

use crate::SceneState;
use axum::{
    extract::{State, ws::WebSocket, WebSocketUpgrade},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;

/// Web server state
#[derive(Clone)]
pub struct WebServerState {
    /// Current scene state
    pub scene_state: Arc<RwLock<SceneState>>,
}

/// Create web server router
pub fn create_router(state: WebServerState) -> Router {
    Router::new()
        .route("/", get(dashboard_handler))
        .route("/ws", get(websocket_handler))
        .with_state(state)
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
