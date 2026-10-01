//! Web server for PCHI Conductor dashboard

use crate::{GovernanceLog, SceneState};
use axum::{
    extract::{Request, State, ws::WebSocket, WebSocketUpgrade},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Json, Response},
    routing::get,
    Router,
};
use base64::Engine;
use std::sync::Arc;
use tokio::sync::RwLock;
use futures_util::StreamExt;
use serde_json::json;

/// Fixed username for dashboard Basic Auth. This is a single-operator
/// tool (self-hosted on a machine the operator already controls), not a
/// multi-user system — one shared secret is enough, so there's nothing
/// gained by making the username configurable too.
const DASHBOARD_USERNAME: &str = "operator";

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
    /// Shared-secret password gating `/`, `/ws`, and `/governance/export`
    /// via HTTP Basic Auth. `None` disables auth entirely — the default,
    /// since this conductor is meant to be self-hosted on a machine its
    /// operator already controls; there's no default audience this needs
    /// to be closed against. Set this before exposing a conductor beyond
    /// localhost.
    pub dashboard_password: Option<String>,
}

/// Create web server router. When `state.dashboard_password` is set,
/// every route requires HTTP Basic Auth — chosen (over a bespoke header,
/// what `/governance/export` used before) because it's the one scheme a
/// plain browser `GET /` navigation can satisfy on its own, via the
/// browser's native credential prompt, with no login page or session
/// cookie to build. `/ws` works the same way in practice: a browser that
/// already authenticated to load the dashboard page caches those
/// credentials for the origin and attaches them automatically to the
/// same-origin WebSocket handshake its own JS opens — standard browser
/// behavior, not something this code arranges. A non-browser WebSocket
/// client (a script, not a loaded page) has no page to inherit
/// credentials from, so it must send its own `Authorization: Basic ...`
/// header on the handshake request.
pub fn create_router(state: WebServerState) -> Router {
    Router::new()
        .route("/", get(dashboard_handler))
        .route("/ws", get(websocket_handler))
        .route("/governance/export", get(governance_export_handler))
        .layer(middleware::from_fn_with_state(state.clone(), require_dashboard_auth))
        .with_state(state)
}

/// Constant-time byte comparison. Not critical for a single self-hosted
/// operator secret the way it would be for a multi-tenant credential
/// store, but cheap to do correctly rather than with `==`.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

fn basic_auth_ok(headers: &HeaderMap, required_password: &str) -> bool {
    let Some(auth) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let Some(encoded) = auth.strip_prefix("Basic ") else {
        return false;
    };
    let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(encoded) else {
        return false;
    };
    let Ok(decoded) = String::from_utf8(decoded) else {
        return false;
    };
    let Some((user, pass)) = decoded.split_once(':') else {
        return false;
    };
    user == DASHBOARD_USERNAME && constant_time_eq(pass.as_bytes(), required_password.as_bytes())
}

async fn require_dashboard_auth(
    State(state): State<WebServerState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    match &state.dashboard_password {
        None => next.run(request).await,
        Some(required) if basic_auth_ok(&headers, required) => next.run(request).await,
        Some(_) => (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Basic realm=\"pchi-conductor\"")],
            "Authentication required",
        )
            .into_response(),
    }
}

/// GET /governance/export — the signed, hash-chained batch of every rule
/// firing this conductor has recorded. Auth, if any, is already enforced
/// by the router-level Basic Auth layer; this handler only has to decide
/// whether governance is configured at all.
async fn governance_export_handler(State(state): State<WebServerState>) -> impl IntoResponse {
    let Some(gov) = &state.governance else {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(json!({"error": "governance_not_configured"})),
        )
            .into_response();
    };
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use tower::ServiceExt;

    fn test_state(dashboard_password: Option<&str>) -> WebServerState {
        let dir = std::env::temp_dir().join(format!("pchi-websrv-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let gov = crate::governance::GovernanceLog::open(
            dir.join("key.hex").to_str().unwrap(),
            dir.join("log.jsonl").to_str().unwrap(),
        )
        .unwrap();
        WebServerState {
            scene_state: Arc::new(RwLock::new(SceneState::new())),
            governance: Some(Arc::new(gov)),
            dashboard_password: dashboard_password.map(str::to_string),
        }
    }

    fn basic_auth_header(user: &str, pass: &str) -> String {
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", user, pass))
        )
    }

    #[tokio::test]
    async fn auth_disabled_by_default_allows_dashboard() {
        let router = create_router(test_state(None));
        let res = router
            .oneshot(HttpRequest::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn auth_enabled_rejects_missing_credentials() {
        let router = create_router(test_state(Some("secret")));
        let res = router
            .oneshot(HttpRequest::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
        assert!(res.headers().get(header::WWW_AUTHENTICATE).is_some());
    }

    #[tokio::test]
    async fn auth_enabled_rejects_wrong_password() {
        let router = create_router(test_state(Some("secret")));
        let res = router
            .oneshot(
                HttpRequest::get("/")
                    .header(header::AUTHORIZATION, basic_auth_header("operator", "nope"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_enabled_rejects_wrong_username() {
        let router = create_router(test_state(Some("secret")));
        let res = router
            .oneshot(
                HttpRequest::get("/")
                    .header(header::AUTHORIZATION, basic_auth_header("someone-else", "secret"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_enabled_accepts_correct_credentials() {
        let router = create_router(test_state(Some("secret")));
        let res = router
            .oneshot(
                HttpRequest::get("/")
                    .header(header::AUTHORIZATION, basic_auth_header("operator", "secret"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn auth_enabled_gates_governance_export_too() {
        let router = create_router(test_state(Some("secret")));
        let res = router
            .oneshot(HttpRequest::get("/governance/export").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(
            res.status(),
            StatusCode::UNAUTHORIZED,
            "the auth layer must cover every route, not just the dashboard page"
        );
    }

    #[tokio::test]
    async fn governance_export_works_once_authenticated() {
        let router = create_router(test_state(Some("secret")));
        let res = router
            .oneshot(
                HttpRequest::get("/governance/export")
                    .header(header::AUTHORIZATION, basic_auth_header("operator", "secret"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
