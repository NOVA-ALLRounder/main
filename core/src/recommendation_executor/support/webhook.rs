use super::super::*;

fn parse_http_method(raw: Option<&str>) -> Method {
    let candidate = raw.unwrap_or("POST").trim().to_uppercase();
    Method::from_bytes(candidate.as_bytes()).unwrap_or(Method::POST)
}

#[derive(Debug, Clone)]
pub(in crate::recommendation_executor) struct WebhookTarget {
    pub(super) method: Method,
    pub(super) path: String,
    pub(super) node_name: String,
}

pub(in crate::recommendation_executor) async fn wait_for_execution_success(
    n8n: &n8n_api::N8nApi,
    workflow_id: &str,
    execution_id_hint: Option<&str>,
) -> Result<n8n_api::ExecutionResult> {
    let timeout_sec = auto_trigger_execution_wait_secs();
    let poll_ms = auto_trigger_execution_poll_ms();
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_sec);
    let hinted = execution_id_hint
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string());

    let mut last_seen: Option<n8n_api::ExecutionResult> = None;
    while std::time::Instant::now() <= deadline {
        let executions = n8n.list_executions(workflow_id, 20).await?;
        let selected = if let Some(hint) = hinted.as_deref() {
            executions.into_iter().find(|e| e.id == hint)
        } else {
            executions.into_iter().next()
        };

        if let Some(exec) = selected {
            if exec.finished {
                if is_execution_success_status(&exec.status) {
                    return Ok(exec);
                }
                return Err(anyhow!(
                    "n8n execution finished with non-success status: workflow_id={} execution_id={} status={}",
                    workflow_id,
                    exec.id,
                    exec.status
                ));
            }
            if is_execution_failure_status(&exec.status) {
                return Err(anyhow!(
                    "n8n execution entered failure status before finish: workflow_id={} execution_id={} status={}",
                    workflow_id,
                    exec.id,
                    exec.status
                ));
            }
            last_seen = Some(exec);
        }
        tokio::time::sleep(Duration::from_millis(poll_ms)).await;
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

pub(in crate::recommendation_executor) fn normalize_localhost_base_url(input: &str) -> String {
    let mut base = input.trim().trim_end_matches('/').to_string();
    for (from, to) in [
        ("http://127.0.0.1", "http://localhost"),
        ("https://127.0.0.1", "https://localhost"),
        ("http://0.0.0.0", "http://localhost"),
        ("https://0.0.0.0", "https://localhost"),
        ("http://[::1]", "http://localhost"),
        ("https://[::1]", "https://localhost"),
    ] {
        if base.starts_with(from) {
            base = base.replacen(from, to, 1);
            break;
        }
    }
    base
}

pub(in crate::recommendation_executor) fn resolve_n8n_webhook_base_url() -> String {
    if let Ok(raw) = std::env::var("STEER_N8N_WEBHOOK_BASE_URL") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return normalize_localhost_base_url(trimmed);
        }
    }
    if let Ok(raw) = std::env::var("N8N_EDITOR_URL") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return normalize_localhost_base_url(trimmed);
        }
    }
    let api = std::env::var("STEER_N8N_API_URL")
        .or_else(|_| std::env::var("N8N_API_URL"))
        .ok()
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "http://localhost:5678/api/v1".to_string());
    for suffix in ["/api/v1", "/api/v2", "/api"] {
        if let Some(stripped) = api.strip_suffix(suffix) {
            let candidate = stripped.trim_end_matches('/');
            if !candidate.is_empty() {
                return normalize_localhost_base_url(candidate);
            }
        }
    }
    normalize_localhost_base_url(&api)
}

pub(in crate::recommendation_executor) fn extract_program_webhook_target(
    workflow: &Value,
) -> Option<WebhookTarget> {
    let nodes = workflow.get("nodes")?.as_array()?;
    let mut fallback: Option<WebhookTarget> = None;
    for node in nodes {
        let node_type = node.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if node_type != "n8n-nodes-base.webhook" {
            continue;
        }
        let params = node.get("parameters").and_then(|v| v.as_object());
        let raw_path = params
            .and_then(|p| p.get("path"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .trim_start_matches('/')
            .to_string();
        if raw_path.is_empty() {
            continue;
        }
        let method = parse_http_method(
            params
                .and_then(|p| p.get("httpMethod").and_then(|v| v.as_str()))
                .or_else(|| params.and_then(|p| p.get("method").and_then(|v| v.as_str()))),
        );
        let candidate = WebhookTarget {
            method,
            path: raw_path,
            node_name: node
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string(),
        };
        let name = node
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        if name.contains("program webhook") || name.contains("programwebhook") {
            return Some(candidate);
        }
        if fallback.is_none() {
            fallback = Some(candidate);
        }
    }
    fallback
}

pub(in crate::recommendation_executor) async fn activate_and_auto_trigger_workflow(
    recommendation_id: i64,
    workflow_id: &str,
    workflow_json: Option<&Value>,
    trigger_payload_hint: Option<&str>,
) -> Result<()> {
    if env_flag("STEER_N8N_MOCK") || !auto_trigger_on_approve_default() {
        return Ok(());
    }

    let n8n = n8n_api::N8nApi::from_env()?;
    n8n.activate_workflow(workflow_id).await?;

    let mut webhook_trigger_sent = false;
    if let Some(target) = workflow_json.and_then(extract_program_webhook_target) {
        let base = resolve_n8n_webhook_base_url();
        let node_segment = if target.node_name.trim().is_empty() {
            "programwebhook".to_string()
        } else {
            target
                .node_name
                .trim()
                .to_lowercase()
                .replace([' ', '\t'], "")
        };
        let legacy_node_segment_for_url = "program%2520webhook".to_string();
        let compact_node_segment_for_url = node_segment.replace('%', "%25");
        let path_segment_for_url = target.path.replace('%', "%25");
        let candidate_urls = [
            format!("{}/webhook/{}", base, target.path),
            format!(
                "{}/webhook/{}/{}/{}",
                base, workflow_id, compact_node_segment_for_url, path_segment_for_url
            ),
            format!(
                "{}/webhook/{}/{}/{}",
                base, workflow_id, legacy_node_segment_for_url, path_segment_for_url
            ),
        ];
        let prompt = trigger_payload_hint
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| format!("recommendation {}", recommendation_id));
        let notion_parent_page_id = std::env::var("NOTION_PAGE_ID")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_default();
        let notion_token = std::env::var("NOTION_TOKEN")
            .ok()
            .or_else(|| std::env::var("NOTION_API_KEY").ok())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_default();
        let telegram_chat_id = std::env::var("TELEGRAM_CHAT_ID")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_default();
        let telegram_bot_token = std::env::var("TELEGRAM_BOT_TOKEN")
            .ok()
            .or_else(|| std::env::var("TELEGRAM_ACCESS_TOKEN").ok())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_default();
        let payload = json!({
            "source": "allvia.auto_approve",
            "recommendation_id": recommendation_id,
            "workflow_id": workflow_id,
            "triggered_at": chrono::Utc::now().to_rfc3339(),
            "prompt": prompt,
            "request_text": prompt,
            "top_n": 5,
            "notion_parent_page_id": notion_parent_page_id,
            "notion_token": notion_token,
            "telegram_chat_id": telegram_chat_id,
            "telegram_bot_token": telegram_bot_token,
        });
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(auto_trigger_timeout_secs()))
            .build()?;
        let retry_attempts = auto_trigger_retry_attempts();
        let retry_delay_ms = auto_trigger_retry_delay_ms();
        let mut webhook_errors: Vec<String> = Vec::new();
        'trigger_attempts: for attempt in 1..=retry_attempts {
            for url in candidate_urls.iter() {
                let response = match client
                    .request(target.method.clone(), url)
                    .json(&payload)
                    .send()
                    .await
                {
                    Ok(resp) => resp,
                    Err(error) => {
                        webhook_errors.push(format!(
                            "attempt={} {} {} => request error: {}",
                            attempt, target.method, url, error
                        ));
                        continue;
                    }
                };
                let status = response.status();
                if status.is_success() {
                    println!(
                        "✅ Auto webhook trigger sent: recommendation={} workflow_id={} method={} url={} attempt={}",
                        recommendation_id, workflow_id, target.method, url, attempt
                    );
                    webhook_trigger_sent = true;
                    break 'trigger_attempts;
                }
                let body = response.text().await.unwrap_or_default();
                webhook_errors.push(format!(
                    "attempt={} {} {} => HTTP {} ({})",
                    attempt,
                    target.method,
                    url,
                    status.as_u16(),
                    body
                ));
            }
            if attempt < retry_attempts {
                tokio::time::sleep(Duration::from_millis(retry_delay_ms * (attempt as u64))).await;
            }
        }
        if !webhook_trigger_sent {
            eprintln!(
                "⚠️ Auto webhook trigger failed for recommendation={} workflow_id={}: {}",
                recommendation_id,
                workflow_id,
                webhook_errors.join(" | ")
            );
        }
    }

    if webhook_trigger_sent {
        match wait_for_execution_success(&n8n, workflow_id, None).await {
            Ok(execution) => {
                println!(
                    "✅ Auto webhook execution completed: recommendation={} workflow_id={} execution_id={} status={}",
                    recommendation_id, workflow_id, execution.id, execution.status
                );
                return Ok(());
            }
            Err(e) => {
                eprintln!(
                    "⚠️ Webhook trigger sent but completion not observed: recommendation={} workflow_id={} error={}. Falling back to direct execute.",
                    recommendation_id, workflow_id, e
                );
            }
        }
    }

    let execution = n8n.execute_workflow(workflow_id).await?;
    if execution.finished {
        if is_execution_success_status(&execution.status) {
            println!(
                "✅ Auto execute fallback completed immediately: recommendation={} workflow_id={} execution_id={} status={}",
                recommendation_id, workflow_id, execution.id, execution.status
            );
            return Ok(());
        }
        return Err(anyhow!(
            "n8n execute returned finished non-success status: workflow_id={} execution_id={} status={}",
            workflow_id,
            execution.id,
            execution.status
        ));
    }
    let execution_id = execution.id.trim().to_string();
    let completed = wait_for_execution_success(
        &n8n,
        workflow_id,
        if execution_id.is_empty() {
            None
        } else {
            Some(execution_id.as_str())
        },
    )
    .await?;
    println!(
        "✅ Auto execute fallback completed: recommendation={} workflow_id={} execution_id={} status={}",
        recommendation_id, workflow_id, completed.id, completed.status
    );
    Ok(())
}
