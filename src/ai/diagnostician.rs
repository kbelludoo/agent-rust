use crate::ai::router::ModelRouter;
use std::sync::Arc;
use tokio::process::Command;
use tracing::{info, warn};

#[derive(Clone)]
pub struct BotDiagnostician {
    router: Arc<ModelRouter>,
}

impl BotDiagnostician {
    pub fn new(router: Arc<ModelRouter>) -> Self {
        Self { router }
    }

    /// Captura as últimas linhas de log de um serviço systemd
    pub async fn fetch_recent_logs(&self, service_name: &str, lines: u32) -> String {
        if service_name.is_empty() {
            return "Nenhum serviço systemd associado.".to_string();
        }

        match Command::new("journalctl")
            .args(["-u", service_name, "-n", &lines.to_string(), "--no-pager"])
            .output()
            .await
        {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.trim().is_empty() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    if stderr.trim().is_empty() {
                        "Logs vazios no journalctl.".to_string()
                    } else {
                        stderr.to_string()
                    }
                } else {
                    stdout.to_string()
                }
            }
            Err(e) => format!("Falha ao invocar journalctl: {}", e),
        }
    }

    /// Executa diagnóstico do erro usando a IA (com auto-fallback)
    pub async fn diagnose_bot(&self, bot_name: &str, service_name: &str, reason: &str) -> String {
        info!("🔍 [Diagnostician] Acionando análise de IA para o bot {} ({})", bot_name, service_name);
        let logs = self.fetch_recent_logs(service_name, 25).await;

        let prompt = format!(
            "Você é um especialista em SRE e bots. O bot '{}' travou com o motivo: '{}'.\n\
             Abaixo estão os últimos logs do sistema:\n\
             ```\n{}\n```\n\
             Responda em exatamente 2 frases curtas: 1) O que causou o travamento? 2) Como solucionar?",
            bot_name, reason, logs
        );

        match self.router.execute_agent(&prompt).await {
            Ok(resp) => {
                info!("🧠 [Diagnostician] Diagnóstico gerado via {}: {}", resp.provider, resp.response);
                format!("{} (via {})", resp.response.trim(), resp.provider)
            }
            Err(e) => {
                warn!("⚠️ [Diagnostician] Não foi possível obter diagnóstico por IA: {}", e);
                format!("Falha temporária ao consultar IA ({}).", e)
            }
        }
    }
}
