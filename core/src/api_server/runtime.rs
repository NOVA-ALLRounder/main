use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::{db, llm_gateway, recommendation_executor};

static INFLIGHT_AGENT_EXECUTIONS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static TELEGRAM_LISTENER_STARTED: OnceLock<AtomicBool> = OnceLock::new();
pub(crate) static API_SERVER_STARTED_AT: OnceLock<String> = OnceLock::new();
static INFLIGHT_PROVISION_OPS: OnceLock<Mutex<HashSet<i64>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelegramListenerStartOutcome {
    Started,
    AlreadyRunning,
}

pub(crate) fn inflight_agent_executions() -> &'static Mutex<HashSet<String>> {
    INFLIGHT_AGENT_EXECUTIONS.get_or_init(|| Mutex::new(HashSet::new()))
}

pub(crate) fn telegram_listener_started_flag() -> &'static AtomicBool {
    TELEGRAM_LISTENER_STARTED.get_or_init(|| AtomicBool::new(false))
}

pub fn try_spawn_telegram_listener(
    llm: std::sync::Arc<dyn llm_gateway::LLMClient>,
) -> Result<TelegramListenerStartOutcome, &'static str> {
    let started_flag = telegram_listener_started_flag();
    if started_flag
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(TelegramListenerStartOutcome::AlreadyRunning);
    }

    let bot = match crate::telegram::TelegramBot::from_env(llm, None) {
        Some(value) => value,
        None => {
            started_flag.store(false, Ordering::SeqCst);
            return Err("missing_telegram_token");
        }
    };

    tokio::spawn(async move {
        std::sync::Arc::new(bot).start_polling().await;
        telegram_listener_started_flag().store(false, Ordering::SeqCst);
    });

    Ok(TelegramListenerStartOutcome::Started)
}

pub(crate) fn telegram_polling_requested() -> bool {
    match std::env::var("STEER_TELEGRAM_POLLING") {
        Ok(value) => is_truthy_env_value(&value),
        Err(_) => has_nonempty_env("TELEGRAM_BOT_TOKEN"),
    }
}

pub(crate) fn log_verification_run(
    kind: &str,
    ok: bool,
    summary: &str,
    details: Option<serde_json::Value>,
) {
    let details_str = details.map(|value| value.to_string());
    let _ = db::insert_verification_run(kind, ok, summary, details_str.as_deref());
}

pub(crate) fn mark_api_server_started_at() {
    API_SERVER_STARTED_AT.get_or_init(|| chrono::Utc::now().to_rfc3339());
}

pub(crate) fn spawn_workflow_provision_recovery_loop(
    llm_client: Option<std::sync::Arc<dyn llm_gateway::LLMClient>>,
) {
    tokio::spawn(async move {
        let _ = db::reconcile_workflow_provision_ops(50);
        loop {
            for status in ["requested", "provisioning"] {
                match db::list_workflow_provision_ops(25, Some(status), None) {
                    Ok(ops) => {
                        for op in ops {
                            let claim_token = match op
                                .claim_token
                                .as_deref()
                                .map(str::trim)
                                .filter(|value| value.starts_with("provisioning:"))
                            {
                                Some(value) => value.to_string(),
                                None => {
                                    let _ = db::mark_workflow_provision_failed(
                                        op.id,
                                        "workflow provisioning op missing valid claim token",
                                    );
                                    continue;
                                }
                            };

                            let should_spawn =
                                if let Ok(mut guard) = inflight_provision_ops().lock() {
                                    guard.insert(op.id)
                                } else {
                                    false
                                };
                            if !should_spawn {
                                continue;
                            }

                            let llm_for_op = llm_client.clone();
                            tokio::spawn(async move {
                                let preclaim = recommendation_executor::PreclaimedProvisioning {
                                    claim_token: Some(claim_token),
                                    provision_op_id: op.id,
                                    force_recreate: false,
                                };
                                if let Err(error) = recommendation_executor::execute_approved_recommendation_with_preclaim(
                                    op.recommendation_id,
                                    llm_for_op,
                                    preclaim,
                                )
                                .await
                                {
                                    eprintln!(
                                        "⚠️ Provision recovery failed: op_id={} recommendation_id={} status={} error={}",
                                        op.id, op.recommendation_id, status, error
                                    );
                                }
                                if let Ok(mut guard) = inflight_provision_ops().lock() {
                                    guard.remove(&op.id);
                                }
                            });
                        }
                    }
                    Err(error) => {
                        eprintln!(
                            "⚠️ Failed to list workflow provision ops (status={}): {}",
                            status, error
                        );
                    }
                }
            }

            let _ = db::reconcile_workflow_provision_ops(50);
            tokio::time::sleep(std::time::Duration::from_secs(8)).await;
        }
    });
}

fn has_nonempty_env(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn is_truthy_env_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn inflight_provision_ops() -> &'static Mutex<HashSet<i64>> {
    INFLIGHT_PROVISION_OPS.get_or_init(|| Mutex::new(HashSet::new()))
}
