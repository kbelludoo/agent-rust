use crate::ai::jev::JevClient;
use crate::ai::router::ModelRouter;
use crate::ai::types::{AgentRequest, ChatCompletionRequest, ModelEntry, ModelListResponse};
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

#[derive(Debug, Deserialize)]
pub struct TriageRequest {
    pub bot_name: String,
    #[serde(default)]
    pub service_name: String,
    pub reason: String,
    #[serde(default)]
    pub logs: String,
}

#[derive(Clone)]
pub struct AppState {
    pub report: Arc<RwLock<FleetReport>>,
    pub router: Arc<ModelRouter>,
    pub jev: Arc<JevClient>,
}

pub fn create_router(state: AppState, web_dir: Option<String>) -> Router {
    let mut router = Router::new()
        // OpenAI-Compatible Endpoints
        .route("/v1/chat/completions", post(chat_completions_handler))
        .route("/v1/models", get(models_handler))
        .route("/agent", post(agent_handler))
        // Fleet & Watchdog Endpoints
        .route("/api/health", get(health_handler))
        .route("/api/fleet", get(fleet_handler))
        .route("/api/restart/:id", post(restart_handler))
        // Jev System One Triager
        .route("/api/jev/triage", post(jev_triage_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    if let Some(dir) = web_dir {
        if std::path::Path::new(&dir).exists() {
            info!("📁 [Agent-Rust] Servindo arquivos web estáticos de: {}", dir);
            router = router.fallback_service(tower_http::services::ServeDir::new(dir));
        }
    }

    router
}

async fn health_handler() -> &'static str {
    "OK - Agent-Rust Sentinel, AI Gateway & Jev System One Running"
}

async fn fleet_handler(State(state): State<AppState>) -> impl IntoResponse {
    let report = state.report.read().await;
    Json((*report).clone())
}

async fn models_handler(State(state): State<AppState>) -> impl IntoResponse {
    let providers = state.router.active_providers();
    let mut data: Vec<ModelEntry> = providers
        .iter()
        .map(|p| ModelEntry {
            id: p.default_model.clone(),
            object: "model".to_string(),
            owned_by: p.name.clone(),
        })
        .collect();

    // Adiciona o modelo System One Jev da TypeSafe AI
    data.push(ModelEntry {
        id: "jev-latest".to_string(),
        object: "system_one".to_string(),
        owned_by: "TypeSafe AI (Jev System One)".to_string(),
    });

    Json(ModelListResponse {
        object: "list".to_string(),
        data,
    })
}

async fn chat_completions_handler(
    State(state): State<AppState>,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match state.router.chat_completion(req).await {
        Ok(resp) => Ok(Json(resp)),
        Err(e) => Err((StatusCode::BAD_GATEWAY, format!("AI Cascade Error: {}", e))),
    }
}

async fn agent_handler(
    State(state): State<AppState>,
    Json(req): Json<AgentRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match state.router.execute_agent(&req.message).await {
        Ok(resp) => Ok(Json(resp)),
        Err(e) => Err((StatusCode::BAD_GATEWAY, format!("AI Error: {}", e))),
    }
}

async fn jev_triage_handler(
    State(state): State<AppState>,
    Json(req): Json<TriageRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match state
        .jev
        .triage_incident(&req.bot_name, &req.service_name, &req.reason, &req.logs)
        .await
    {
        Ok(resp) => Ok(Json(resp)),
        Err(e) => Err((
            StatusCode::BAD_GATEWAY,
            format!("Jev System One Error: {}", e),
        )),
    }
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
