mod ai;
mod config;
mod discovery;
mod metrics;
mod probe;
mod server;
mod telegram;
mod watchdog;

use ai::diagnostician::BotDiagnostician;
use ai::router::ModelRouter;
use chrono::Utc;
use config::AppConfig;
use discovery::BotAutoDiscovery;
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
    info!("🤖 [Agent-Rust] Monitorando {} bots na frota base", cfg.bots.len());

    let telegram = TelegramNotifier::new(cfg.telegram_token.clone(), cfg.telegram_chat_id.clone());
    if telegram.is_configured() {
        info!("📢 [Agent-Rust] Notificador Telegram ativo para alertas.");
    } else {
        info!("ℹ️ [Agent-Rust] Telegram não configurado (nenhum .secrets.json encontrado).");
    }

    // Inicializa o Router Multi-Provedor de IA com Auto-Fallback e Jev System One
    let ai_router = Arc::new(ModelRouter::new());
    let diagnostician = Arc::new(BotDiagnostician::new(Arc::clone(&ai_router)));
    let jev = Arc::clone(diagnostician.jev());
    if jev.is_configured() {
        info!("⚡ [Agent-Rust] Jev System One Client ativo (TypeSafe AI <100ms Triage)");
    }
    let auto_discovery = Arc::new(BotAutoDiscovery::new());

    let prober = FleetProber::new(cfg.bots.clone());
    let sentinel = Arc::new(WatchdogSentinel::new(
        telegram.clone(),
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
        jev,
    };

    // Loop em background para auto-discovery dinâmico e monitoramento
    let prober_clone = prober.clone();
    let sentinel_clone = Arc::clone(&sentinel);
    let metrics_clone = Arc::clone(&metrics_collector);
    let report_clone = Arc::clone(&report_state);
    let discovery_clone = Arc::clone(&auto_discovery);
    let telegram_clone = telegram.clone();
    let poll_interval = Duration::from_secs(cfg.poll_interval_sec);

    tokio::spawn(async move {
        loop {
            // 1. Auto-Discovery Dinâmico: detecta novos bots na VPS automaticamente
            let current = prober_clone.current_bots().await;
            let new_bots = discovery_clone.discover_new_bots(&current).await;
            for nb in new_bots {
                let msg = format!(
                    "🚀 *NOVO BOT DETECTADO NA VPS*\n\n\
                     • *Nome:* {}\n\
                     • *Porta:* `{}`\n\
                     • *Ação:* Integrado automaticamente ao Sentinel Watchdog e ao Dashboard!",
                    nb.name, nb.port
                );
                telegram_clone.send_alert(&msg).await;
                prober_clone.register_discovered_bot(nb).await;
            }

            // 2. Healthcheck e coleta de métricas
            let snapshots = prober_clone.probe_all().await;
            let active = snapshots.iter().filter(|s| s.online).count();

            // 3. Avalia cada bot no Sentinel para detecção de Stall / Inércia
            for s in &snapshots {
                sentinel_clone.evaluate_bot(s).await;
            }

            // 4. Atualiza o estado da frota para o Dashboard e API
            {
                let mut rep = report_clone.write().await;
                rep.updated_at = Utc::now().to_rfc3339();
                rep.total_bots = snapshots.len();
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
