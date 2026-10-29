use crate::metrics::HostMetrics;
use crate::probe::BotSnapshot;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetReport {
    pub updated_at: String,
    pub total_bots: usize,
    pub active_bots: usize,
    pub host: HostMetrics,
    pub bots: Vec<BotSnapshot>,
}

#[derive(Clone)]
pub struct AppState {
    pub report: Arc<RwLock<FleetReport>>,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/fleet", get(fleet_handler))
        .route("/api/restart/:id", post(restart_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health_handler() -> &'static str {
    "OK - Agent-Rust Sentinel Running"
}

async fn fleet_handler(State(state): State<AppState>) -> impl IntoResponse {
    let report = state.report.read().await;
    Json((*report).clone())
}

async fn restart_handler(Path(id): Path<String>) -> (StatusCode, Json<serde_json::Value>) {
    info!("Manual restart request for bot: {}", id);
    let service_name = format!("bot-{}.service", id);
    let output = tokio::process::Command::new("sudo")
        .args(["systemctl", "restart", &service_name])
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true, "message": format!("Serviço {} reiniciado", service_name) })),
        ),
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "ok": false, "error": err.to_string() })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}
