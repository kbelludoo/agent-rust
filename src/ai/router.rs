use crate::ai::caveman::{strip_prolixity, system_directive};
use crate::ai::providers::{Provider, ProviderRegistry};
use crate::ai::types::{AgentResponse, ChatChoice, ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Usage};
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct ModelRouter {
    client: Client,
    providers: Arc<Vec<Provider>>,
}

impl ModelRouter {
    pub fn new() -> Self {
        let providers = ProviderRegistry::load_from_env();
        info!("🤖 [AI Router] Carregados {} provedores de IA para fallback em cascata", providers.len());
        for p in &providers {
            info!("  └─ [{}] {} (Modelo: {})", p.priority, p.name, p.default_model);
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            client,
            providers: Arc::new(providers),
        }
    }

    pub fn active_providers(&self) -> &[Provider] {
        &self.providers
    }

    /// Executa uma conclusão com auto-fallback automático em cascata
    pub async fn chat_completion(&self, mut req: ChatCompletionRequest) -> anyhow::Result<ChatCompletionResponse> {
        let start = Instant::now();

        // Injeta diretiva Caveman para brevidade se não houver system prompt
        if !req.messages.iter().any(|m| m.role == "system") {
            req.messages.insert(0, ChatMessage {
                role: "system".to_string(),
                content: system_directive().to_string(),
            });
        }

        let mut last_error = String::new();

        for provider in self.providers.iter().filter(|p| p.active) {
            let model_name = req.model.as_deref().unwrap_or(&provider.default_model);
            let url = format!("{}/chat/completions", provider.base_url.trim_end_matches('/'));

            let payload = json!({
                "model": model_name,
                "messages": req.messages,
                "temperature": req.temperature.unwrap_or(0.3),
                "max_tokens": req.max_tokens.unwrap_or(1024),
                "stream": false
            });

            info!("🤖 [AI Router] Tentando provedor: {} ({})", provider.name, model_name);

            let res = self.client
                .post(&url)
                .header("Authorization", format!("Bearer {}", provider.api_key))
                .header("Content-Type", "application/json")
                .json(&payload)
                .send()
                .await;

            match res {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let text = resp.text().await.unwrap_or_default();
                        if let Ok(mut comp) = serde_json::from_str::<ChatCompletionResponse>(&text) {
                            // Limpa prolixidade da resposta via Caveman
                            for c in &mut comp.choices {
                                c.message.content = strip_prolixity(&c.message.content);
                            }
                            comp.provider_used = Some(provider.name.clone());
                            info!("✅ [AI Router] Sucesso via {} em {}ms", provider.name, start.elapsed().as_millis());
                            return Ok(comp);
                        } else {
                            // Tenta parse manual se formato for ligeiramente diferente
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                                if let Some(content) = val.pointer("/choices/0/message/content").and_then(|c| c.as_str()) {
                                    let clean_content = strip_prolixity(content);
                                    let comp = ChatCompletionResponse {
                                        id: format!("comp-{}", start.elapsed().as_millis()),
                                        object: "chat.completion".to_string(),
                                        created: chrono::Utc::now().timestamp(),
                                        model: model_name.to_string(),
                                        choices: vec![ChatChoice {
                                            index: 0,
                                            message: ChatMessage {
                                                role: "assistant".to_string(),
                                                content: clean_content,
                                            },
                                            finish_reason: Some("stop".to_string()),
                                        }],
                                        usage: Some(Usage {
                                            prompt_tokens: 50,
                                            completion_tokens: 50,
                                            total_tokens: 100,
                                        }),
                                        provider_used: Some(provider.name.clone()),
                                    };
                                    info!("✅ [AI Router] Sucesso via {} (fallback parser) em {}ms", provider.name, start.elapsed().as_millis());
                                    return Ok(comp);
                                }
                            }
                            last_error = format!("{}: resposta JSON inválida", provider.name);
                            warn!("⚠️ [AI Router] {}", last_error);
                        }
                    } else {
                        last_error = format!("{}: HTTP {}", provider.name, status);
                        warn!("⚠️ [AI Router] Falha no provedor {}. Cascata para o próximo...", last_error);
                    }
                }
                Err(e) => {
                    last_error = format!("{}: erro de conexão ({})", provider.name, e);
                    warn!("⚠️ [AI Router] Falha de rede no provedor {}. Cascata para o próximo...", last_error);
                }
            }
        }

        error!("❌ [AI Router] Todos os provedores de IA falharam na cascata. Último erro: {}", last_error);
        anyhow::bail!("Todos os provedores de IA na cascata falharam: {}", last_error)
    }

    /// Executa uma instrução direta de agente
    pub async fn execute_agent(&self, message: &str) -> anyhow::Result<AgentResponse> {
        let start = Instant::now();
        let req = ChatCompletionRequest {
            model: None,
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: message.to_string(),
            }],
            temperature: Some(0.2),
            max_tokens: Some(512),
            stream: Some(false),
        };

        let resp = self.chat_completion(req).await?;
        let content = resp.choices.first().map(|c| c.message.content.clone()).unwrap_or_default();
        let provider = resp.provider_used.unwrap_or_else(|| "Unknown".to_string());

        Ok(AgentResponse {
            response: content,
            provider,
            duration_ms: start.elapsed().as_millis(),
        })
    }
}
