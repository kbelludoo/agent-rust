use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoredBot {
    pub id: String,
    pub name: String,
    pub port: u16,
    pub url: String,
    pub service_name: String,
    pub dashboard_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretsConfig {
    #[serde(default)]
    pub telegram_token: String,
    #[serde(default)]
    pub telegram_chat_id: String,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub port: u16,
    pub poll_interval_sec: u64,
    pub stall_threshold_sec: u64,
    pub telegram_token: String,
    pub telegram_chat_id: String,
    pub bots: Vec<MonitoredBot>,
}

impl AppConfig {
    pub fn load() -> Self {
        let (telegram_token, telegram_chat_id) = Self::load_telegram_secrets();

        let bots = vec![
            MonitoredBot {
                id: "huntera".to_string(),
                name: "⚔️ Huntera Idle".to_string(),
                port: 3000,
                url: "http://127.0.0.1:3000/api/status".to_string(),
                service_name: "bot-huntera.service".to_string(),
                dashboard_url: "https://kbelludoo.github.io/bot-huntera-rust/".to_string(),
            },
            MonitoredBot {
                id: "baiak".to_string(),
                name: "🔥 Baiak Idle".to_string(),
                port: 3001,
                url: "http://127.0.0.1:3001/api/status".to_string(),
                service_name: "bot-baiak.service".to_string(),
                dashboard_url: "https://kbelludoo.github.io/bot-baiak-rust/".to_string(),
            },
            MonitoredBot {
                id: "tibiaidle".to_string(),
                name: "🛡️ Tibia Idle".to_string(),
                port: 3002,
                url: "http://127.0.0.1:3002/api/status".to_string(),
                service_name: "bot-tibiaidle.service".to_string(),
                dashboard_url: "https://kbelludoo.github.io/bot-tibiaidle-rust/".to_string(),
            },
            MonitoredBot {
                id: "poketrue".to_string(),
                name: "⚡ PokeTrue".to_string(),
                port: 3003,
                url: "http://127.0.0.1:3003/api/status".to_string(),
                service_name: "bot-poketrue.service".to_string(),
                dashboard_url: "https://kbelludoo.github.io/bot-poketrue-rust/".to_string(),
            },
            MonitoredBot {
                id: "pokeidle".to_string(),
                name: "⚡ PokeIdle".to_string(),
                port: 3004,
                url: "http://127.0.0.1:3004/api/status".to_string(),
                service_name: "bot-pokeidle.service".to_string(),
                dashboard_url: "https://kbelludoo.github.io/bot-pokeidle-rust/".to_string(),
            },
            MonitoredBot {
                id: "lolidle".to_string(),
                name: "🗡️ LoL Idle".to_string(),
                port: 3005,
                url: "http://127.0.0.1:3005/api/status".to_string(),
                service_name: "bot-lolidle.service".to_string(),
                dashboard_url: "https://kbelludoo.github.io/bot-lolidle-rust/".to_string(),
            },
            MonitoredBot {
                id: "luxidle".to_string(),
                name: "🌟 Lux Idle".to_string(),
                port: 3006,
                url: "http://127.0.0.1:3006/api/status".to_string(),
                service_name: "bot-luxidle.service".to_string(),
                dashboard_url: "https://kbelludoo.github.io/bot-luxidle-rust/".to_string(),
            },
            MonitoredBot {
                id: "sigma".to_string(),
                name: "🔮 Sigma Idle".to_string(),
                port: 3007,
                url: "http://127.0.0.1:3007/api/status".to_string(),
                service_name: "bot-sigma.service".to_string(),
                dashboard_url: "https://kbelludoo.github.io/bot-sigma-rust/".to_string(),
            },
            MonitoredBot {
                id: "leveling".to_string(),
                name: "🏹 Leveling Idle".to_string(),
                port: 3008,
                url: "http://127.0.0.1:3008/api/status".to_string(),
                service_name: "bot-leveling.service".to_string(),
                dashboard_url: "https://kbelludoo.github.io/bot-leveling-rust/".to_string(),
            },
            MonitoredBot {
                id: "farmbot".to_string(),
                name: "🌾 Farm Ricochet".to_string(),
                port: 3009,
                url: "http://127.0.0.1:3009/api/status".to_string(),
                service_name: "bot-farmbot.service".to_string(),
                dashboard_url: "https://kbelludoo.github.io/bot-farmbot-rust/".to_string(),
            },
            MonitoredBot {
                id: "pokelegacy".to_string(),
                name: "🐲 PokeLegacy".to_string(),
                port: 3010,
                url: "http://127.0.0.1:3010/api/status".to_string(),
                service_name: "bot-pokelegacy.service".to_string(),
                dashboard_url: "https://kbelludoo.github.io/bot-pokelegacy-rust/".to_string(),
            },
        ];

        Self {
            port: 8082,
            poll_interval_sec: 30,
            stall_threshold_sec: 600, // 10 minutos sem alteração de métricas
            telegram_token,
            telegram_chat_id,
            bots,
        }
    }

    fn load_telegram_secrets() -> (String, String) {
        if let (Ok(t), Ok(c)) = (env::var("TELEGRAM_TOKEN"), env::var("TELEGRAM_CHAT_ID")) {
            if !t.is_empty() && !c.is_empty() {
                return (t, c);
            }
        }

        let candidates = [
            ".secrets.json",
            "../vps_backup_essential/.secrets.json",
            "C:/Users/kbell/Downloads/vps_backup_essential/.secrets.json",
            "C:/Users/kbell/Downloads/bot-template-go/.secrets.json",
            "/home/ubuntu/.secrets.json",
        ];

        for path in &candidates {
            if Path::new(path).exists() {
                if let Ok(content) = fs::read_to_string(path) {
                    if let Ok(cfg) = serde_json::from_str::<SecretsConfig>(&content) {
                        if !cfg.telegram_token.is_empty() && !cfg.telegram_chat_id.is_empty() {
                            return (cfg.telegram_token, cfg.telegram_chat_id);
                        }
                    }
                }
            }
        }

        (String::new(), String::new())
    }
}
