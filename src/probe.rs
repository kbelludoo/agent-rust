use crate::config::MonitoredBot;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountTelemetry {
    pub id: String,
    pub name: String,
    pub gold: u64,
    pub exp: u64,
    pub stamina: u32,
    pub kills: u64,
    pub circuit_breaker: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotSnapshot {
    pub id: String,
    pub name: String,
    pub port: u16,
    pub service_name: String,
    pub dashboard_url: String,
    pub online: bool,
    pub latency_ms: u64,
    pub total_gold: u64,
    pub total_exp: u64,
    pub total_stamina: u32,
    pub total_kills: u64,
    pub active_accounts: usize,
    pub accounts: Vec<AccountTelemetry>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct FleetProber {
    client: Client,
    bots: Arc<RwLock<Vec<MonitoredBot>>>,
}

impl FleetProber {
    pub fn new(bots: Vec<MonitoredBot>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(3000))
            .build()
            .unwrap_or_default();

        Self {
            client,
            bots: Arc::new(RwLock::new(bots)),
        }
    }

    pub async fn current_bots(&self) -> Vec<MonitoredBot> {
        let b = self.bots.read().await;
        b.clone()
    }

    pub async fn register_discovered_bot(&self, bot: MonitoredBot) {
        let mut b = self.bots.write().await;
        if !b.iter().any(|existing| existing.port == bot.port || existing.id == bot.id) {
            b.push(bot);
        }
    }

    pub async fn probe_bot(&self, bot: &MonitoredBot) -> BotSnapshot {
        let start = Instant::now();
        let mut snapshot = BotSnapshot {
            id: bot.id.clone(),
            name: bot.name.clone(),
            port: bot.port,
            service_name: bot.service_name.clone(),
            dashboard_url: bot.dashboard_url.clone(),
            online: false,
            latency_ms: 0,
            total_gold: 0,
            total_exp: 0,
            total_stamina: 0,
            total_kills: 0,
            active_accounts: 0,
            accounts: Vec::new(),
            error: None,
        };

        match self.client.get(&bot.url).send().await {
            Ok(resp) => {
                snapshot.latency_ms = start.elapsed().as_millis() as u64;
                if resp.status().is_success() {
                    snapshot.online = true;
                    if let Ok(json) = resp.json::<Value>().await {
                        Self::parse_telemetry(&mut snapshot, &json);
                    }
                } else {
                    snapshot.error = Some(format!("HTTP {}", resp.status()));
                }
            }
            Err(e) => {
                snapshot.latency_ms = start.elapsed().as_millis() as u64;
                snapshot.error = Some(e.to_string());
            }
        }

        snapshot
    }

    fn parse_telemetry(snapshot: &mut BotSnapshot, json: &Value) {
        if let Some(accs) = json.get("accounts").and_then(|a| a.as_array()) {
            snapshot.active_accounts = accs.len();
            for a in accs {
                let id = a.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let gold = a.get("gold").and_then(|v| v.as_u64()).unwrap_or(0);
                let exp = a.get("exp").and_then(|v| v.as_u64()).unwrap_or(0);
                let stamina = a.get("stamina").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let kills = a.get("kills").and_then(|v| v.as_u64()).unwrap_or(0);
                let cb = a.get("cb_state").or_else(|| a.get("cbState")).and_then(|v| v.as_str()).unwrap_or("CLOSED").to_string();

                snapshot.total_gold += gold;
                snapshot.total_exp += exp;
                snapshot.total_stamina += stamina;
                snapshot.total_kills += kills;

                snapshot.accounts.push(AccountTelemetry {
                    id,
                    name,
                    gold,
                    exp,
                    stamina,
                    kills,
                    circuit_breaker: cb,
                });
            }
        } else if let Some(gold) = json.get("gold").and_then(|v| v.as_u64()) {
            snapshot.total_gold = gold;
            snapshot.total_exp = json.get("exp").and_then(|v| v.as_u64()).unwrap_or(0);
            snapshot.total_stamina = json.get("stamina").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            snapshot.total_kills = json.get("kills").and_then(|v| v.as_u64()).unwrap_or(0);
            snapshot.active_accounts = 1;
        }
    }

    pub async fn probe_all(&self) -> Vec<BotSnapshot> {
        let current = self.bots.read().await.clone();
        let mut results = Vec::with_capacity(current.len());
        for b in &current {
            results.push(self.probe_bot(b).await);
        }
        results
    }
}
