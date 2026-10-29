mod ai;
mod config;
mod metrics;
mod probe;
mod server;
mod telegram;
mod watchdog;

use ai::diagnostician::BotDiagnostician;
use ai::router::ModelRouter;
use chrono::Utc;
use config::AppConfig;
use metrics::MetricsCollector;
use probe::FleetProber;
use server::{AppState, FleetReport};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use telegram::TelegramNotifier;
use tokio::sync::RwLock;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use watchdog::WatchdogSentinel;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    let cfg = AppConfig::load();
    info!("🚀 [Agent-Rust] Iniciando Sentinel Watchdog & AI Cascade Gateway...");
    info!("🤖 [Agent-Rust] Monitorando {} bots na frota", cfg.bots.len());

    let telegram = TelegramNotifier::new(cfg.telegram_token.clone(), cfg.telegram_chat_id.clone());
    if telegram.is_configured() {
        info!("📢 [Agent-Rust] Notificador Telegram ativo para alertas.");
    } else {
        info!("ℹ️ [Agent-Rust] Telegram não configurado (nenhum .secrets.json encontrado).");
    }

    // Inicializa o Router Multi-Provedor de IA com Auto-Fallback
    let ai_router = Arc::new(ModelRouter::new());
    let diagnostician = Arc::new(BotDiagnostician::new(Arc::clone(&ai_router)));

    let prober = FleetProber::new(cfg.bots.clone());
    let sentinel = Arc::new(WatchdogSentinel::new(
        telegram,
        Some(Arc::clone(&diagnostician)),
        cfg.stall_threshold_sec,
    ));
    let metrics_collector = Arc::new(MetricsCollector::new());

    let initial_report = FleetReport {
        updated_at: Utc::now().to_rfc3339(),
        total_bots: cfg.bots.len(),
        active_bots: 0,
        host: metrics_collector.collect(),
        bots: Vec::new(),
    };

    let report_state = Arc::new(RwLock::new(initial_report));
    let app_state = AppState {
        report: Arc::clone(&report_state),
        router: Arc::clone(&ai_router),
    };

    // Loop em background para monitorar métricas e auto-curar
    let prober_clone = prober.clone();
    let sentinel_clone = Arc::clone(&sentinel);
    let metrics_clone = Arc::clone(&metrics_collector);
    let report_clone = Arc::clone(&report_state);
    let poll_interval = Duration::from_secs(cfg.poll_interval_sec);

    tokio::spawn(async move {
        loop {
            let snapshots = prober_clone.probe_all().await;
            let active = snapshots.iter().filter(|s| s.online).count();

            // Avalia cada bot no Sentinel para detecção de Stall / Inércia
            for s in &snapshots {
                sentinel_clone.evaluate_bot(s).await;
            }

            // Atualiza o estado da frota para o Dashboard e API
            {
                let mut rep = report_clone.write().await;
                rep.updated_at = Utc::now().to_rfc3339();
                rep.active_bots = active;
                rep.host = metrics_clone.collect();
                rep.bots = snapshots;
            }

            tokio::time::sleep(poll_interval).await;
        }
    });

    // Inicia o servidor HTTP Axum (OpenAI Gateway + Sentinel REST)
    let router = server::create_router(app_state);
    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.port));
    info!("🌐 [Agent-Rust] Servidor OpenAI Gateway & REST ouvindo em http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
