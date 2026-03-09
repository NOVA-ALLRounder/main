use super::super::*;

pub(in crate::recommendation_executor) fn mock_workflow_json(name: &str) -> serde_json::Value {
    n8n_api::build_orchestrator_fallback_workflow(name, None, "test_mock_template")
}

pub(in crate::recommendation_executor) fn workflow_has_nodes(value: &serde_json::Value) -> bool {
    value
        .get("nodes")
        .and_then(|n| n.as_array())
        .map(|nodes| !nodes.is_empty())
        .unwrap_or(false)
}

pub(in crate::recommendation_executor) fn mark_provision_progress(op_id: i64, detail: &str) {
    if let Err(error) = db::mark_workflow_provision_in_progress(op_id, Some(detail)) {
        eprintln!(
            "⚠️ Failed to update workflow provision progress: op_id={} detail='{}' error={}",
            op_id, detail, error
        );
    }
}

pub(in crate::recommendation_executor) fn parse_bool_env(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

pub(in crate::recommendation_executor) fn n8n_create_active_default() -> bool {
    parse_bool_env("STEER_N8N_ACTIVE_ON_CREATE", true)
}

pub(in crate::recommendation_executor) fn auto_trigger_on_approve_default() -> bool {
    parse_bool_env("STEER_N8N_AUTO_TRIGGER_ON_APPROVE", true)
}

pub(in crate::recommendation_executor) fn auto_trigger_timeout_secs() -> u64 {
    std::env::var("STEER_N8N_AUTO_TRIGGER_TIMEOUT_SEC")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|v| v.clamp(3, 60))
        .unwrap_or(8)
}

pub(in crate::recommendation_executor) fn auto_trigger_retry_attempts() -> u32 {
    std::env::var("STEER_N8N_AUTO_TRIGGER_RETRIES")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .map(|v| v.clamp(1, 12))
        .unwrap_or(6)
}

pub(in crate::recommendation_executor) fn auto_trigger_retry_delay_ms() -> u64 {
    std::env::var("STEER_N8N_AUTO_TRIGGER_RETRY_DELAY_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|v| v.clamp(200, 5_000))
        .unwrap_or(600)
}

pub(in crate::recommendation_executor) fn auto_trigger_execution_wait_secs() -> u64 {
    std::env::var("STEER_N8N_AUTO_TRIGGER_EXECUTION_WAIT_SEC")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|v| v.clamp(10, 900))
        .unwrap_or(120)
}

pub(in crate::recommendation_executor) fn auto_trigger_execution_poll_ms() -> u64 {
    std::env::var("STEER_N8N_AUTO_TRIGGER_EXECUTION_POLL_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|v| v.clamp(300, 10_000))
        .unwrap_or(1_500)
}

pub(in crate::recommendation_executor) fn is_execution_success_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "success" | "completed"
    )
}

pub(in crate::recommendation_executor) fn is_execution_failure_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "error" | "failed" | "failure" | "crashed" | "cancelled" | "canceled" | "timeout"
    )
}

pub(in crate::recommendation_executor) fn workflow_generation_prompt(
    rec: &db::Recommendation,
) -> String {
    let base = rec.n8n_prompt.trim();
    let feedback_status = rec
        .feedback_status
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let feedback_note = rec
        .feedback_note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if feedback_status != "refine" {
        return base.to_string();
    }

    let Some(note) = feedback_note else {
        return base.to_string();
    };

    if base.is_empty() {
        format!("Incorporate this user refinement request: {}", note)
    } else {
        format!(
            "{}\n\nUser refinement request to incorporate before generating the workflow:\n- {}",
            base, note
        )
    }
}

pub(in crate::recommendation_executor) fn ensure_recommendation_ready_for_approval(
    rec: &db::Recommendation,
) -> Result<()> {
    let decision = recommendation_policy::evaluate_recommendation_approval_readiness(rec);
    if decision.ready {
        return Ok(());
    }

    Err(anyhow!(
        "recommendation {} is not ready for approval: {}",
        rec.id,
        decision.reasons.join(" ")
    ))
}

pub(in crate::recommendation_executor) fn should_use_test_mock_workflow() -> bool {
    env_flag("STEER_TEST_ASSUME_APPROVED") && env_flag("STEER_N8N_MOCK")
}
