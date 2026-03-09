use crate::controller::actions::ActionRunner;
use anyhow::{anyhow, Result};

impl ActionRunner {
    pub(in crate::controller::actions) fn build_n8n_scope_workflow(
        name: &str,
        marker: &str,
        goal_prompt: Option<&str>,
    ) -> serde_json::Value {
        let marker_value = if marker.trim().is_empty() {
            "RUN_SCOPE_TEST_03".to_string()
        } else {
            marker.trim().to_string()
        };
        let mut prompt_seed_parts = Vec::new();
        if let Some(goal_text) = goal_prompt {
            let trimmed = goal_text.trim();
            if !trimmed.is_empty() {
                prompt_seed_parts.push(trimmed.to_string());
            }
        }
        prompt_seed_parts.push(format!("scope_marker={}", marker_value));
        let prompt_seed = prompt_seed_parts.join("\n");
        crate::n8n_api::build_orchestrator_fallback_workflow(
            name,
            Some(prompt_seed.as_str()),
            "scope_bootstrap",
        )
    }

    pub(in crate::controller::actions) fn extract_latest_n8n_workflow_id(
        history: &[String],
    ) -> Option<String> {
        for entry in history.iter().rev() {
            if let Some(pos) = entry.to_lowercase().find("n8n workflow created:") {
                let rest = entry[(pos + "n8n workflow created:".len())..].trim();
                let id = rest
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_matches(|c: char| {
                        c == '(' || c == ')' || c == ',' || c == '"' || c == '\''
                    })
                    .to_string();
                if !id.is_empty() {
                    return Some(id);
                }
            }
            if let Some(pos) = entry.to_lowercase().find("workflow_id=") {
                let mut id = String::new();
                for ch in entry[(pos + "workflow_id=".len())..].chars() {
                    if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                        id.push(ch);
                    } else {
                        break;
                    }
                }
                if !id.is_empty() {
                    return Some(id);
                }
            }
        }
        None
    }

    pub(in crate::controller::actions) fn is_n8n_success_status(status: &str) -> bool {
        matches!(
            status.trim().to_ascii_lowercase().as_str(),
            "success" | "completed"
        )
    }

    pub(in crate::controller::actions) fn is_n8n_failure_status(status: &str) -> bool {
        matches!(
            status.trim().to_ascii_lowercase().as_str(),
            "error" | "failed" | "failure" | "crashed" | "cancelled" | "canceled" | "timeout"
        )
    }

    pub(in crate::controller::actions) async fn wait_for_n8n_execution_success(
        n8n: &crate::n8n_api::N8nApi,
        workflow_id: &str,
        execution_id_hint: Option<&str>,
    ) -> Result<crate::n8n_api::ExecutionResult> {
        let timeout_sec = std::env::var("STEER_N8N_ACTION_EXECUTION_WAIT_SEC")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(|v| v.clamp(10, 900))
            .unwrap_or(120);
        let poll_ms = std::env::var("STEER_N8N_ACTION_EXECUTION_POLL_MS")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(|v| v.clamp(300, 10_000))
            .unwrap_or(1_500);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_sec);
        let hinted = execution_id_hint
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string());

        let mut last_seen: Option<crate::n8n_api::ExecutionResult> = None;
        while std::time::Instant::now() <= deadline {
            let executions = n8n.list_executions(workflow_id, 20).await?;
            let selected = if let Some(hint) = hinted.as_deref() {
                executions.into_iter().find(|e| e.id == hint)
            } else {
                executions.into_iter().next()
            };

            if let Some(exec) = selected {
                if exec.finished {
                    if Self::is_n8n_success_status(&exec.status) {
                        return Ok(exec);
                    }
                    return Err(anyhow!(
                        "n8n execution finished with non-success status: workflow_id={} execution_id={} status={}",
                        workflow_id,
                        exec.id,
                        exec.status
                    ));
                }
                if Self::is_n8n_failure_status(&exec.status) {
                    return Err(anyhow!(
                        "n8n execution entered failure status before finish: workflow_id={} execution_id={} status={}",
                        workflow_id,
                        exec.id,
                        exec.status
                    ));
                }
                last_seen = Some(exec);
            }
            tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;
        }

        if let Some(last) = last_seen {
            return Err(anyhow!(
                "n8n execution completion timeout: workflow_id={} execution_id={} status={} finished={}",
                workflow_id,
                last.id,
                last.status,
                last.finished
            ));
        }
        Err(anyhow!(
            "n8n execution completion timeout: workflow_id={} (no execution observed)",
            workflow_id
        ))
    }
}
