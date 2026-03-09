use serde_json::json;

use crate::controller::actions::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) async fn handle_n8n_create_workflow(
        plan: &serde_json::Value,
        goal: &str,
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
        action_data: &mut Option<serde_json::Value>,
    ) {
        crate::load_env_with_fallback();
        let marker = plan["marker"]
            .as_str()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .or_else(|| Self::preferred_run_scope_marker(Some(goal)))
            .unwrap_or_else(|| "RUN_SCOPE_TEST_03".to_string());
        let default_plain = format!("Steer Scope {}", marker);
        let mut name = plan["name"]
            .as_str()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| default_plain.clone());
        if name == default_plain {
            name = format!(
                "Steer Scope {} {}",
                marker,
                chrono::Local::now().format("%m%d-%H%M%S")
            );
        }
        let workflow = Self::build_n8n_scope_workflow(&name, &marker, Some(goal));
        match crate::n8n_api::N8nApi::from_env() {
            Ok(n8n) => match n8n.create_workflow(&name, &workflow, false).await {
                Ok(workflow_id) => {
                    let editor_url = format!("http://localhost:5678/workflow/{}", workflow_id);
                    let _ = crate::applescript::open_url(&editor_url);
                    *description =
                        format!("n8n workflow created: {} ({})", workflow_id, editor_url);
                    *action_status_override = Some("success");
                    *action_data = Some(json!({
                        "proof": "n8n_workflow_created",
                        "workflow_id": workflow_id,
                        "workflow_name": name,
                        "workflow_url": editor_url,
                        "marker": marker
                    }));
                    if let Some(data) = action_data.as_ref() {
                        Self::log_evidence(
                            "n8n",
                            "workflow",
                            &[
                                ("status", "confirmed".to_string()),
                                (
                                    "workflow_id",
                                    data.get("workflow_id")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                ),
                                (
                                    "workflow_name",
                                    data.get("workflow_name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                ),
                                (
                                    "marker",
                                    data.get("marker")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                ),
                            ],
                        );
                    }
                }
                Err(e) => {
                    *description = format!("n8n_create_workflow failed: {}", e);
                    *action_status_override = Some("failed");
                }
            },
            Err(e) => {
                *description = format!("n8n_create_workflow failed: {}", e);
                *action_status_override = Some("failed");
            }
        }
    }

    pub(in crate::controller::actions) async fn handle_n8n_execute_workflow(
        plan: &serde_json::Value,
        history: &[String],
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
        action_data: &mut Option<serde_json::Value>,
    ) {
        crate::load_env_with_fallback();
        let workflow_id = plan["workflow_id"]
            .as_str()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .or_else(|| Self::extract_latest_n8n_workflow_id(history))
            .unwrap_or_default();
        if workflow_id.is_empty() {
            *description = "n8n_execute_workflow failed: workflow_id is missing (no history match)"
                .to_string();
            *action_status_override = Some("failed");
            return;
        }

        match crate::n8n_api::N8nApi::from_env() {
            Ok(n8n) => match n8n.execute_workflow(&workflow_id).await {
                Ok(exec) => {
                    let mut final_exec = exec.clone();
                    if exec.finished {
                        if !Self::is_n8n_success_status(&exec.status) {
                            *description = format!(
                                "n8n_execute_workflow failed: workflow_id={} execution_id={} status={}",
                                workflow_id, exec.id, exec.status
                            );
                            *action_status_override = Some("failed");
                        }
                    } else {
                        let execution_id = exec.id.trim().to_string();
                        match Self::wait_for_n8n_execution_success(
                            &n8n,
                            &workflow_id,
                            if execution_id.is_empty() {
                                None
                            } else {
                                Some(execution_id.as_str())
                            },
                        )
                        .await
                        {
                            Ok(done_exec) => {
                                final_exec = done_exec;
                            }
                            Err(e) => {
                                *description = format!("n8n_execute_workflow failed: {}", e);
                                *action_status_override = Some("failed");
                            }
                        }
                    }
                    if *action_status_override != Some("failed") {
                        *description = format!(
                            "n8n execution completed: workflow_id={} execution_id={} status={}",
                            workflow_id, final_exec.id, final_exec.status
                        );
                        *action_status_override = Some("success");
                        *action_data = Some(json!({
                            "proof": "n8n_execution_completed",
                            "workflow_id": workflow_id,
                            "execution_id": final_exec.id,
                            "execution_status": final_exec.status,
                            "execution_finished": final_exec.finished
                        }));
                        if let Some(data) = action_data.as_ref() {
                            Self::log_evidence(
                                "n8n",
                                "execution",
                                &[
                                    ("status", "completed".to_string()),
                                    (
                                        "workflow_id",
                                        data.get("workflow_id")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string(),
                                    ),
                                    (
                                        "execution_id",
                                        data.get("execution_id")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string(),
                                    ),
                                    (
                                        "execution_status",
                                        data.get("execution_status")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string(),
                                    ),
                                ],
                            );
                        }
                    }
                }
                Err(e) => {
                    *description = format!("n8n_execute_workflow failed: {}", e);
                    *action_status_override = Some("failed");
                }
            },
            Err(e) => {
                *description = format!("n8n_execute_workflow failed: {}", e);
                *action_status_override = Some("failed");
            }
        }
    }
}
