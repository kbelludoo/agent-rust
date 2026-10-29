use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub default_model: String,
    pub active: bool,
    pub priority: u32,
}

pub struct ProviderRegistry;

impl ProviderRegistry {
    pub fn load_from_env() -> Vec<Provider> {
        Self::load_dotenv();
        let mut providers = Vec::new();

        // 1. Apinex Gateway (Free: Gemini 3.8 Flash, DeepSeek v4.1, GPT-6 Luna)
        if let Ok(key) = env::var("APINEX_API_KEY") {
            if !key.is_empty() {
                providers.push(Provider {
                    name: "Apinex Gemini Free".to_string(),
                    base_url: "https://api.apinex.bond/v1".to_string(),
                    api_key: key.clone(),
                    default_model: env::var("APINEX_MODEL").unwrap_or_else(|_| "free/gemini-3.8-flash".to_string()),
                    active: true,
                    priority: 1,
                });
                providers.push(Provider {
                    name: "Apinex DeepSeek Free".to_string(),
                    base_url: "https://api.apinex.bond/v1".to_string(),
                    api_key: key,
                    default_model: "free/deepseek-v4.1-flash".to_string(),
                    active: true,
                    priority: 2,
                });
            }
        }

        // 2. TokenHarbor Gateway (Free: DeepSeek v4.1 Flash Free)
        if let Ok(key) = env::var("TOKENHARBOR_API_KEY") {
            if !key.is_empty() {
                providers.push(Provider {
                    name: "TokenHarbor DeepSeek Free".to_string(),
                    base_url: "https://tokenharbor.ai/v1".to_string(),
                    api_key: key,
                    default_model: env::var("TOKENHARBOR_MODEL").unwrap_or_else(|_| "deepseek-v4.1-flash:free".to_string()),
                    active: true,
                    priority: 3,
                });
            }
        }

        // 3. AgentRouter Gateway (DeepSeek v4 Flash com reasoning)
        if let Ok(key) = env::var("AGENTROUTER_API_KEY") {
            if !key.is_empty() {
                providers.push(Provider {
                    name: "AgentRouter DeepSeek v4".to_string(),
                    base_url: "https://agentrouter.org/v1".to_string(),
                    api_key: key,
                    default_model: env::var("AGENTROUTER_MODEL").unwrap_or_else(|_| "deepseek-v4-flash".to_string()),
                    active: true,
                    priority: 4,
                });
            }
        }

        // 4. Mistral AI (Codestral / Free)
        if let Ok(key) = env::var("MISTRAL_API_KEY") {
            if !key.is_empty() {
                providers.push(Provider {
                    name: "Mistral Codestral".to_string(),
                    base_url: "https://api.mistral.ai/v1".to_string(),
                    api_key: key,
                    default_model: env::var("MISTRAL_MODEL").unwrap_or_else(|_| "codestral-latest".to_string()),
                    active: true,
                    priority: 5,
                });
            }
        }

        // 5. NVIDIA Build / NIM (HTTP/2 persistent, Llama 3.2 Vision / Kimi K3)
        if let Ok(key) = env::var("NVIDIA_API_KEY") {
            if !key.is_empty() {
                providers.push(Provider {
                    name: "NVIDIA NIM Llama".to_string(),
                    base_url: "https://integrate.api.nvidia.com/v1".to_string(),
                    api_key: key,
                    default_model: env::var("NVIDIA_MODEL").unwrap_or_else(|_| "meta/llama-3.2-11b-vision-instruct".to_string()),
                    active: true,
                    priority: 6,
                });
            }
        }

        // 6. Kilo Gateway (kilo-auto/free)
        if let Ok(key) = env::var("KILO_API_KEY") {
            if !key.is_empty() {
                providers.push(Provider {
                    name: "Kilo Gateway Free".to_string(),
                    base_url: "https://api.kilo.ai/api/gateway".to_string(),
                    api_key: key,
                    default_model: "kilo-auto/free".to_string(),
                    active: true,
                    priority: 7,
                });
            }
        }

        // 7. Groq Fallback (Tier Gratuito)
        if let Ok(key) = env::var("GROQ_API_KEY") {
            if !key.is_empty() {
                providers.push(Provider {
                    name: "Groq Llama 3.3".to_string(),
                    base_url: "https://api.groq.com/openai/v1".to_string(),
                    api_key: key,
                    default_model: "llama-3.3-70b-versatile".to_string(),
                    active: true,
                    priority: 8,
                });
            }
        }

        // 8. OpenRouter Fallback (Modelos Livres)
        if let Ok(key) = env::var("OPENROUTER_API_KEY") {
            if !key.is_empty() {
                providers.push(Provider {
                    name: "OpenRouter Free".to_string(),
                    base_url: "https://openrouter.ai/api/v1".to_string(),
                    api_key: key,
                    default_model: "meta-llama/llama-3.3-70b-instruct:free".to_string(),
                    active: true,
                    priority: 9,
                });
            }
        }

        // 9. OpenCode Local Server (porta 4096 / 8888)
        providers.push(Provider {
            name: "OpenCode Local Bridge".to_string(),
            base_url: "http://localhost:4096/v1".to_string(),
            api_key: "dummy".to_string(),
            default_model: "opencode/deepseek-v4.1-flash-free".to_string(),
            active: true,
            priority: 10,
        });

        // Ordena por prioridade
        providers.sort_by_key(|p| p.priority);
        providers
    }

    fn load_dotenv() {
        let candidates = [
            ".env",
            "../agent-go/.env",
            "C:/Users/kbell/Downloads/agent-go/.env",
            "/home/ubuntu/agent-go/.env",
            "/home/ubuntu/.env",
        ];

        for path in &candidates {
            if Path::new(path).exists() {
                if let Ok(content) = fs::read_to_string(path) {
                    for line in content.lines() {
                        let line = line.trim();
                        if line.is_empty() || line.starts_with('#') {
                            continue;
                        }
                        if let Some((k, v)) = line.split_once('=') {
                            let k = k.trim();
                            let v = v.trim().trim_matches('"').trim_matches('\'');
                            if env::var(k).is_err() {
                                env::set_var(k, v);
                            }
                        }
                    }
                    break;
                }
            }
        }
    }
}
