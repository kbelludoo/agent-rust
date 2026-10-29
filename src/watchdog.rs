use crate::ai::diagnostician::BotDiagnostician;
use crate::ai::jev::JevAction;
use crate::probe::BotSnapshot;
use crate::telegram::TelegramNotifier;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
struct BotProgressState {
    last_gold: u64,
    last_exp: u64,
    last_stamina: u32,
    last_kills: u64,
    last_change_time: Instant,
    restart_attempts: u32,
    last_restart_time: Option<Instant>,
    last_telegram_alert_time: Option<Instant>,
}

impl Default for BotProgressState {
    fn default() -> Self {
        Self {
            last_gold: 0,
            last_exp: 0,
            last_stamina: 0,
            last_kills: 0,
            last_change_time: Instant::now(),
            restart_attempts: 0,
            last_restart_time: None,
            last_telegram_alert_time: None,
        }
    }
}

pub struct WatchdogSentinel {
    telegram: TelegramNotifier,
    diagnostician: Option<Arc<BotDiagnostician>>,
    stall_threshold: Duration,
    alert_cooldown: Duration,
    states: Mutex<HashMap<String, BotProgressState>>,
}

impl WatchdogSentinel {
    pub fn new(
        telegram: TelegramNotifier,
        diagnostician: Option<Arc<BotDiagnostician>>,
        stall_threshold_sec: u64,
    ) -> Self {
        Self {
            telegram,
            diagnostician,
            stall_threshold: Duration::from_secs(stall_threshold_sec),
            alert_cooldown: Duration::from_secs(900), // 15 minutos entre alertas do mesmo bot
            states: Mutex::new(HashMap::new()),
        }
    }

    pub async fn evaluate_bot(&self, bot: &BotSnapshot) {
        let mut states = self.states.lock().await;
        let state = states.entry(bot.id.clone()).or_default();

        // 1. Caso o bot esteja OFFLINE
        if !bot.online {
            warn!("⚠️ [Sentinel] Bot {} (:{}) está OFFLINE! Tentando auto-cura...", bot.name, bot.port);
            drop(states);
            self.handle_offline_or_stalled(bot, "OFFLINE (Porta não responde)").await;
            return;
        }

        // 2. Avaliação de Progresso Ativo (Gold, EXP, Stamina ou Kills)
        // Regra do Usuário: conferir se algum número aumentou ou diminuiu.
        // Se ficar parado, forte indício de bot parado.
        let changed = (bot.total_gold != state.last_gold && (state.last_gold > 0 || bot.total_gold > 0))
            || (bot.total_exp != state.last_exp && (state.last_exp > 0 || bot.total_exp > 0))
            || (bot.total_stamina != state.last_stamina && (state.last_stamina > 0 || bot.total_stamina > 0))
            || (bot.total_kills != state.last_kills && (state.last_kills > 0 || bot.total_kills > 0));

        if changed {
            // Houve evolução/movimentação no jogo: resetamos o cronômetro de stall
            state.last_gold = bot.total_gold;
            state.last_exp = bot.total_exp;
            state.last_stamina = bot.total_stamina;
            state.last_kills = bot.total_kills;
            state.last_change_time = Instant::now();
            state.restart_attempts = 0;
            return;
        }

        // Se é o primeiro snapshot com valores, registramos a base
        if state.last_gold == 0 && state.last_exp == 0 && state.last_kills == 0 && (bot.total_gold > 0 || bot.total_exp > 0 || bot.total_kills > 0) {
            state.last_gold = bot.total_gold;
            state.last_exp = bot.total_exp;
            state.last_stamina = bot.total_stamina;
            state.last_kills = bot.total_kills;
            state.last_change_time = Instant::now();
            return;
        }

        // 3. Verificação de Inércia (Stall Temporal)
        let stall_duration = state.last_change_time.elapsed();
        if stall_duration >= self.stall_threshold {
            let mins = stall_duration.as_secs() / 60;
            let reason = format!(
                "STALLED há {} minutos (nenhum número aumentou ou diminuiu: Gold={}, EXP={}, Stamina={}, Kills={} - forte indício de bot parado)",
                mins, bot.total_gold, bot.total_exp, bot.total_stamina, bot.total_kills
            );
            warn!("🛑 [Sentinel] Bot {} {}", bot.name, reason);
            drop(states);
            self.handle_offline_or_stalled(bot, &reason).await;
        }
    }

    async fn handle_offline_or_stalled(&self, bot: &BotSnapshot, reason: &str) {
        // Coleta logs recentes do systemd para enriquecer a triagem Jev e o diagnóstico LLM
        let logs = if let Some(diag) = &self.diagnostician {
            diag.fetch_recent_logs(&bot.service_name, 25).await
        } else {
            String::new()
        };

        // Triagem Rápida System One (<200ms) com o modelo Jev da TypeSafe AI
        let jev_decision = if let Some(diag) = &self.diagnostician {
            diag.triage_bot(&bot.name, &bot.service_name, reason, &logs).await
        } else {
            None
        };

        let (should_restart, should_alert, attempts) = {
            let mut states = self.states.lock().await;
            let state = states.entry(bot.id.clone()).or_default();

            // Se o Jev sugerir esperar (ex: bot em cooldown normal ou carregando) com alta confiança, adiamos o restart
            let jev_wait = jev_decision.as_ref().is_some_and(|d| {
                matches!(d.action, JevAction::WaitLonger) && d.action_confidence >= 0.70
            });

            let restart_due = !jev_wait && state.last_restart_time.is_none_or(|t| t.elapsed() > Duration::from_secs(180));
            let alert_due = state.last_telegram_alert_time.is_none_or(|t| t.elapsed() > self.alert_cooldown);

            if restart_due {
                state.restart_attempts += 1;
                state.last_restart_time = Some(Instant::now());
            }

            let attempts = state.restart_attempts;

            // Se Jev indicar falha crítica iminente ou ação explícita de alertar operador, forçamos o alerta
            let jev_force_alert = jev_decision.as_ref().is_some_and(|d| {
                d.is_critical || matches!(d.action, JevAction::AlertOperator)
            });

            let alert = alert_due && (attempts >= 1 || jev_force_alert);

            if alert {
                state.last_telegram_alert_time = Some(Instant::now());
            }

            (restart_due, alert, attempts)
        };

        let mut restart_success = false;

        // Auto-Cura nível 1: reiniciar serviço systemd
        if should_restart && !bot.service_name.is_empty() {
            info!("🔄 [Sentinel] Acionando auto-cura: sudo systemctl restart {}", bot.service_name);
            match Command::new("sudo")
                .args(["systemctl", "restart", &bot.service_name])
                .output()
                .await
            {
                Ok(output) => {
                    if output.status.success() {
                        info!("✅ [Sentinel] Serviço {} reiniciado com sucesso via auto-cura.", bot.service_name);
                        restart_success = true;
                    } else {
                        let err = String::from_utf8_lossy(&output.stderr);
                        error!("❌ [Sentinel] Falha ao reiniciar {}: {}", bot.service_name, err);
                    }
                }
                Err(e) => {
                    error!("❌ [Sentinel] Erro ao invocar comando de reinício: {}", e);
                }
            }
        }

        // Se falhou ou atingiu o limite de tolerância, dispara mensagem no Telegram com Parecer da IA
        if should_alert {
            let status_cura = if restart_success {
                "Reinicialização executada, mas bot ainda não pontuou."
            } else if should_restart {
                "Tentativa de reinicialização automática falhou!"
            } else {
                "Aguardando ciclo de auto-cura conforme recomendação System One."
            };

            // Consulta IA para diagnóstico inteligente dos logs
            let ai_diagnostic = if let Some(diag) = &self.diagnostician {
                diag.diagnose_bot(&bot.name, &bot.service_name, reason, &logs).await
            } else {
                "Análise heurística de SRE: verifique conexão e credenciais do bot.".to_string()
            };

            let jev_summary = if let Some(ref dec) = jev_decision {
                format!(
                    "⚡ *Decisão Rápida Jev (System One):*\n\
                     • *Ação Recomendada:* `{}` (confiança: {:.0}%)\n\
                     • *Urgência:* `{:.1}/2.0` | *Crítico:* `{}`\n\
                     • *Latência de Decisão:* `{}ms`\n\n",
                    dec.action,
                    dec.action_confidence * 100.0,
                    dec.urgency_score,
                    if dec.is_critical { "SIM ⚠️" } else { "NÃO ✅" },
                    dec.latency_ms
                )
            } else {
                String::new()
            };

            let msg = format!(
                "🚨 *ALERTA VPS: BOT TRAVADO OU COM ERRO*\n\n\
                 • *Bot:* {}\n\
                 • *Porta:* `{}`\n\
                 • *Motivo:* {}\n\
                 • *Tentativas de Auto-Cura:* {}\n\
                 • *Ação do Sistema:* {}\n\n\
                 {}\
                 🧠 *Parecer da IA (Cascade Router):*\n\
                 _{}_\n\n\
                 🔗 *Dashboard:* {}\n\
                 _Ação humana recomendada se o erro persistir._",
                bot.name,
                bot.port,
                reason,
                attempts,
                status_cura,
                jev_summary,
                ai_diagnostic,
                bot.dashboard_url
            );

            self.telegram.send_alert(&msg).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sentinel_stall_detection() {
        let telegram = TelegramNotifier::new(String::new(), String::new());
        let sentinel = WatchdogSentinel::new(telegram, None, 1);

        let mut bot = BotSnapshot {
            id: "test-bot".to_string(),
            name: "Test Bot".to_string(),
            port: 3000,
            service_name: "test.service".to_string(),
            dashboard_url: "http://localhost".to_string(),
            online: true,
            latency_ms: 10,
            total_gold: 100,
            total_exp: 1000,
            total_stamina: 50,
            total_kills: 10,
            active_accounts: 1,
            accounts: Vec::new(),
            error: None,
        };

        // 1º tick: baseline inicial
        sentinel.evaluate_bot(&bot).await;

        // Simula passagem de tempo sem dormir (determinístico)
        {
            let mut states = sentinel.states.lock().await;
            let state = states.get_mut("test-bot").unwrap();
            state.last_change_time = Instant::now() - Duration::from_secs(2);
        }

        // 2º tick: métricas inalteradas -> deve detectar stall
        sentinel.evaluate_bot(&bot).await;

        let states = sentinel.states.lock().await;
        let state = states.get("test-bot").unwrap();
        assert!(state.restart_attempts >= 1, "Deveria ter tentado reiniciar o bot estagnado");

        drop(states);

        // 3º tick: métricas aumentaram (gold mudou de 100 para 200) -> deve recuperar
        bot.total_gold = 200;
        sentinel.evaluate_bot(&bot).await;

        let states = sentinel.states.lock().await;
        let state = states.get("test-bot").unwrap();
        assert_eq!(state.restart_attempts, 0, "Deveria ter zerado as tentativas após recuperação");
        assert_eq!(state.last_gold, 200);
        drop(states);

        // 4º tick: métrica diminuiu (stamina caiu de 50 para 40 por caça ativa) -> deve manter ativo e não stallar
        bot.total_stamina = 40;
        sentinel.evaluate_bot(&bot).await;
        let states = sentinel.states.lock().await;
        let state = states.get("test-bot").unwrap();
        assert_eq!(state.last_stamina, 40, "Deveria registrar decréscimo de stamina como atividade");
        assert_eq!(state.restart_attempts, 0);
    }
}
