use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct TelegramNotifier {
    client: Client,
    token: String,
    chat_id: String,
}

impl TelegramNotifier {
    pub fn new(token: String, chat_id: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .unwrap_or_default();

        Self {
            client,
            token,
            chat_id,
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.token.is_empty() && !self.chat_id.is_empty()
    }

    pub async fn send_alert(&self, message: &str) -> bool {
        if !self.is_configured() {
            warn!("📢 [Telegram] Credenciais não configuradas. Mensagem suprimida: {}", message);
            return false;
        }

        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);
        let payload = json!({
            "chat_id": self.chat_id,
            "text": message,
            "parse_mode": "Markdown",
            "disable_web_page_preview": false
        });

        match self.client.post(&url).json(&payload).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    info!("📢 [Telegram] Notificação de stall enviada com sucesso!");
                    true
                } else {
                    let err = resp.text().await.unwrap_or_default();
                    error!("📢 [Telegram] Falha ao enviar alerta: {}", err);
                    false
                }
            }
            Err(e) => {
                error!("📢 [Telegram] Erro de rede ao conectar com Telegram API: {}", e);
                false
            }
        }
    }
}
