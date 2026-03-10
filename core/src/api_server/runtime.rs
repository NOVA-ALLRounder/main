use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::{db, llm_gateway, recommendation_executor};

static INFLIGHT_AGENT_EXECUTIONS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static TELEGRAM_LISTENER_STARTED: OnceLock<AtomicBool> = OnceLock::new();
pub(crate) static API_SERVER_STARTED_AT: OnceLock<String> = OnceLock::new();
static INFLIGHT_PROVISION_OPS: OnceLock<Mutex<HashSet<i64>>> = OnceLock::new();
static WORKFLOW_PROVISION_RECOVERY_STARTED: AtomicBool = AtomicBool::new(false);

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

struct TelegramListenerRunGuard;

impl Drop for TelegramListenerRunGuard {
    fn drop(&mut self) {
        telegram_listener_started_flag().store(false, Ordering::SeqCst);
    }
}

struct InflightProvisionOpGuard {
    op_id: i64,
}

impl InflightProvisionOpGuard {
    fn claim(op_id: i64) -> Option<Self> {
        let mut guard = inflight_provision_ops_lock();
        if guard.insert(op_id) {
            Some(Self { op_id })
        } else {
            None
        }
    }
}

impl Drop for InflightProvisionOpGuard {
    fn drop(&mut self) {
        let mut guard = inflight_provision_ops_lock();
        guard.remove(&self.op_id);
    }
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
        let _listener_guard = TelegramListenerRunGuard;
        std::sync::Arc::new(bot).start_polling().await;
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
    if WORKFLOW_PROVISION_RECOVERY_STARTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

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

                            let Some(claim_guard) = InflightProvisionOpGuard::claim(op.id) else {
                                continue;
                            };

                            let llm_for_op = llm_client.clone();
                            tokio::spawn(async move {
                                let _claim_guard = claim_guard;
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

fn inflight_provision_ops_lock() -> MutexGuard<'static, HashSet<i64>> {
    match inflight_provision_ops().lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("⚠️ inflight provision ops mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    }
}

#[cfg(test)]
fn reset_workflow_provision_recovery_started_for_tests() {
    WORKFLOW_PROVISION_RECOVERY_STARTED.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn inflight_provision_op_guard_releases_claim_on_drop() {
        {
            let mut guard = inflight_provision_ops_lock();
            guard.clear();
        }

        let first = InflightProvisionOpGuard::claim(42).expect("first claim");
        assert!(InflightProvisionOpGuard::claim(42).is_none());
        drop(first);
        assert!(InflightProvisionOpGuard::claim(42).is_some());

        let mut guard = inflight_provision_ops_lock();
        guard.clear();
    }

    #[test]
    #[serial]
    fn telegram_listener_run_guard_resets_started_flag_on_drop() {
        telegram_listener_started_flag().store(true, Ordering::SeqCst);

        {
            let _guard = TelegramListenerRunGuard;
            assert!(telegram_listener_started_flag().load(Ordering::SeqCst));
        }

        assert!(!telegram_listener_started_flag().load(Ordering::SeqCst));
    }

    #[test]
    #[serial]
    fn workflow_provision_recovery_loop_only_claims_once() {
        reset_workflow_provision_recovery_started_for_tests();

        assert!(WORKFLOW_PROVISION_RECOVERY_STARTED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok());
        assert!(WORKFLOW_PROVISION_RECOVERY_STARTED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err());

        reset_workflow_provision_recovery_started_for_tests();
    }
}
