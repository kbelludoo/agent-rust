use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;
use tracing::info;

const DEFAULT_JEV_URL: &str = "https://api.typesafe.ai/v1/systemone";
const DEFAULT_JEV_MODEL: &str = "jev-latest";
const FALLBACK_JEV_KEY: &str = "apikey_291ce7edd1556ac4ad28a12d76708a64e1e_472e3b37846c05536ac52bece2bfd54da5fb6300e5e46ec58ea624d7d0043e18";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JevAction {
    RestartService,
    WaitLonger,
    AlertOperator,
    Unknown(String),
}

impl From<&str> for JevAction {
    fn from(s: &str) -> Self {
        match s {
            "restart_service" => JevAction::RestartService,
            "wait_longer" => JevAction::WaitLonger,
            "alert_operator" => JevAction::AlertOperator,
            other => JevAction::Unknown(other.to_string()),
        }
    }
}

impl std::fmt::Display for JevAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JevAction::RestartService => write!(f, "restart_service"),
            JevAction::WaitLonger => write!(f, "wait_longer"),
            JevAction::AlertOperator => write!(f, "alert_operator"),
            JevAction::Unknown(s) => write!(f, "unknown({})", s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JevTriageDecision {
    pub action: JevAction,
    pub action_confidence: f32,
    pub is_critical: bool,
    pub critical_probability: f32,
    pub urgency_score: f32,
    pub latency_ms: u128,
}

impl JevTriageDecision {
    pub fn summary(&self) -> String {
        format!(
            "Action: {} (conf: {:.0}%) | Urgency: {:.2}/2.0 | Critical Prob: {:.0}% (took {}ms)",
            self.action,
            self.action_confidence * 100.0,
            self.urgency_score,
            self.critical_probability * 100.0,
            self.latency_ms
        )
    }
}

#[derive(Debug, Clone)]
pub struct JevClient {
    client: Client,
    api_key: String,
    endpoint: String,
}

impl Default for JevClient {
    fn default() -> Self {
        Self::new()
    }
}

impl JevClient {
    pub fn new() -> Self {
        let api_key = env::var("TYPESAFE_API_KEY")
            .or_else(|_| env::var("JEV_API_KEY"))
            .unwrap_or_else(|_| FALLBACK_JEV_KEY.to_string());

        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        Self {
            client,
            api_key,
            endpoint: DEFAULT_JEV_URL.to_string(),
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.api_key.trim().is_empty()
    }

    /// Executa triagem rápida (<100-300ms) de System One para avaliar bot travado ou com erro
    pub async fn triage_incident(
        &self,
        bot_name: &str,
        service_name: &str,
        reason: &str,
        logs_excerpt: &str,
    ) -> Result<JevTriageDecision> {
        let start = std::time::Instant::now();
        let state = format!(
            "Bot: {}\nService: {}\nIncident Reason: {}\nRecent Logs:\n{}",
            bot_name, service_name, reason, logs_excerpt
        );

        let payload = serde_json::json!({
            "model": DEFAULT_JEV_MODEL,
            "state": state,
            "questions": {
                "action": {
                    "type": "choice",
                    "instructions": "What is the best immediate recovery action for this bot incident?",
                    "criteria": {
                        "restart_service": "Restart the systemd service process immediately to unfreeze or recover connection",
                        "wait_longer": "Wait another monitoring cycle; bot might be legitimately idle, loading, or in safe zone",
                        "alert_operator": "Alert human operator immediately for manual intervention"
                    }
                },
                "critical_failure": {
                    "type": "noul",
                    "instructions": "Is this an unrecoverable critical error requiring direct operator intervention?"
                },
                "urgency": {
                    "type": "score",
                    "instructions": "How urgent is this incident on a low to high scale?",
                    "criteria": ["low", "medium", "high"]
                }
            }
        });

        let resp = self
            .client
            .post(&self.endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .context("Failed to send HTTP request to TypeSafe Jev System One")?;

        let status = resp.status();
        if !status.is_success() {
            let err_body = resp.text().await.unwrap_or_default();
            anyhow::bail!("TypeSafe Jev API returned HTTP {}: {}", status, err_body);
        }

        let raw_val: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse JSON response from Jev API")?;

        let latency_ms = start.elapsed().as_millis();

        let action_choice = raw_val["answers"]["action"]["choice"]
            .as_str()
            .unwrap_or("restart_service");
        let action_conf = raw_val["answers"]["action"]["confidence"]
            .as_f64()
            .map(|v| v as f32)
            .unwrap_or(0.5);

        let critical_prob = raw_val["answers"]["critical_failure"]["noul"]
            .as_f64()
            .map(|v| v as f32)
            .unwrap_or(0.0);

        let urgency = raw_val["answers"]["urgency"]["score"]
            .as_f64()
            .map(|v| v as f32)
            .unwrap_or(1.0);

        let decision = JevTriageDecision {
            action: JevAction::from(action_choice),
            action_confidence: action_conf,
            is_critical: critical_prob >= 0.5,
            critical_probability: critical_prob,
            urgency_score: urgency,
            latency_ms,
        };

        info!(
            "⚡ [Jev System One] Triagem concluída para {}: {} (em {}ms)",
            bot_name,
            decision.summary(),
            latency_ms
        );

        Ok(decision)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jev_action_conversions() {
        assert_eq!(JevAction::from("restart_service").to_string(), "restart_service");
        assert_eq!(JevAction::from("wait_longer").to_string(), "wait_longer");
        assert_eq!(JevAction::from("alert_operator").to_string(), "alert_operator");
        assert_eq!(JevAction::from("custom").to_string(), "unknown(custom)");
    }

    #[test]
    fn test_jev_triage_decision_summary() {
        let decision = JevTriageDecision {
            action: JevAction::RestartService,
            action_confidence: 0.81,
            is_critical: false,
            critical_probability: 0.25,
            urgency_score: 1.86,
            latency_ms: 120,
        };

        let summary = decision.summary();
        assert!(summary.contains("restart_service"));
        assert!(summary.contains("81%"));
        assert!(summary.contains("1.86"));
        assert!(summary.contains("120ms"));
    }
}
