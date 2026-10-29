use crate::config::MonitoredBot;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashSet;
use std::time::Duration;
use tracing::info;

pub struct BotAutoDiscovery {
    client: Client,
    scan_ports: Vec<u16>,
}

impl BotAutoDiscovery {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(1500))
            .build()
            .unwrap_or_default();

        // Escaneia portas padrão de bots (3000 a 3025 e 8090)
        let mut scan_ports: Vec<u16> = (3000..=3025).collect();
        scan_ports.push(8090);

        Self { client, scan_ports }
    }

    /// Escaneia portas e serviços locais para descobrir novos bots dinamicamente
    pub async fn discover_new_bots(&self, existing_bots: &[MonitoredBot]) -> Vec<MonitoredBot> {
        let known_ports: HashSet<u16> = existing_bots.iter().map(|b| b.port).collect();
        let known_ids: HashSet<String> = existing_bots.iter().map(|b| b.id.clone()).collect();
        let mut newly_discovered = Vec::new();

        for port in &self.scan_ports {
            if known_ports.contains(port) {
                continue;
            }

            let test_urls = [
                format!("http://127.0.0.1:{}/api/status", port),
                format!("http://127.0.0.1:{}/api/fleet", port),
            ];

            for url in &test_urls {
                if let Ok(resp) = self.client.get(url).send().await {
                    if resp.status().is_success() {
                        if let Ok(json) = resp.json::<Value>().await {
                            // Extrai nome do jogo ou gera ID automático
                            let game_name = json.get("game")
                                .and_then(|v| v.as_str())
                                .or_else(|| json.get("name").and_then(|v| v.as_str()))
                                .unwrap_or("Novo Bot Rust");

                            let raw_id = game_name.to_lowercase()
                                .replace(' ', "-")
                                .chars()
                                .filter(|c| c.is_alphanumeric() || *c == '-')
                                .collect::<String>();

                            let bot_id = if raw_id.is_empty() || known_ids.contains(&raw_id) {
                                format!("bot-port-{}", port)
                            } else {
                                raw_id
                            };

                            let service_name = format!("bot-{}.service", bot_id);
                            let dashboard_url = format!("https://kbelludoo.github.io/bot-{}-rust/", bot_id);

                            let discovered = MonitoredBot {
                                id: bot_id.clone(),
                                name: format!("🎮 {}", game_name),
                                port: *port,
                                url: url.clone(),
                                service_name,
                                dashboard_url,
                            };

                            info!("✨ [Auto-Discovery] Novo bot detectado na porta {}: {}! Registrando na frota...", port, game_name);
                            newly_discovered.push(discovered);
                            break;
                        }
                    }
                }
            }
        }

        newly_discovered
    }
}
