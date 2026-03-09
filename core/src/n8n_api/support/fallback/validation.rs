use anyhow::Result;
use serde_json::Value;

use super::template::build_orchestrator_fallback_workflow;

fn workflow_is_too_simple(value: &Value) -> bool {
    let nodes = match value.get("nodes").and_then(|n| n.as_array()) {
        Some(v) if !v.is_empty() => v,
        _ => return true,
    };

    let min_nodes = std::env::var("STEER_N8N_MIN_NODE_COUNT")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|v| v.clamp(6, 30))
        .unwrap_or(8);

    if nodes.len() < min_nodes {
        return true;
    }

    let mut has_trigger = false;
    let mut has_transform = false;
    let mut has_validation = false;
    let mut has_branch = false;
    let mut has_error_path = false;
    let mut has_observability = false;
    let mut has_external_io = false;

    for node in nodes {
        let Some(raw_ty) = node.get("type").and_then(|v| v.as_str()) else {
            continue;
        };
        let ty = raw_ty.to_ascii_lowercase();
        let node_name = node
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        match ty.as_str() {
            "n8n-nodes-base.manualtrigger" | "n8n-nodes-base.webhook" => has_trigger = true,
            "n8n-nodes-base.set"
            | "n8n-nodes-base.code"
            | "n8n-nodes-base.function"
            | "n8n-nodes-base.functionitem"
            | "n8n-nodes-base.itemlists" => has_transform = true,
            "n8n-nodes-base.if" | "n8n-nodes-base.switch" => {
                has_branch = true;
                if node_name.contains("error") || node_name.contains("fail") {
                    has_error_path = true;
                }
            }
            "n8n-nodes-base.httprequest"
            | "n8n-nodes-base.notion"
            | "n8n-nodes-base.telegram"
            | "n8n-nodes-base.emailsend"
            | "n8n-nodes-base.slack" => has_external_io = true,
            _ => {}
        }

        if node_name.contains("validat") || node_name.contains("schema") {
            has_validation = true;
        }
        if node_name.contains("observability")
            || node_name.contains("metric")
            || node_name.contains("telemetry")
            || node_name.contains("log")
        {
            has_observability = true;
        }
        if node_name.contains("error") || node_name.contains("fallback") {
            has_error_path = true;
        }
    }

    !(has_trigger
        && has_transform
        && has_validation
        && has_branch
        && has_error_path
        && has_observability
        && has_external_io)
}

pub fn normalize_workflow_for_create(name: &str, workflow_json: &Value) -> Result<Value> {
    let mut normalized = workflow_json.clone();
    let allow_fallback = super::super::config::parse_bool_env_with_default(
        "STEER_N8N_ALLOW_SIMPLE_WORKFLOW_FALLBACK",
        true,
    );

    let is_empty = normalized
        .get("nodes")
        .and_then(|n| n.as_array())
        .map(|arr| arr.is_empty())
        .unwrap_or(true);

    if is_empty {
        if allow_fallback {
            println!("⚠️ Workflow nodes empty. Falling back to orchestrator workflow template.");
            normalized = build_orchestrator_fallback_workflow(name, None, "empty_nodes");
        } else {
            return Err(anyhow::anyhow!(
                "workflow validation failed: nodes are empty. \
Set STEER_N8N_ALLOW_SIMPLE_WORKFLOW_FALLBACK=1 to allow orchestrator fallback."
            ));
        }
    } else if workflow_is_too_simple(&normalized) {
        if allow_fallback {
            println!(
                "⚠️ Workflow nodes too simple. Replacing with orchestrator workflow template."
            );
            normalized = build_orchestrator_fallback_workflow(name, None, "too_simple_nodes");
        } else {
            return Err(anyhow::anyhow!(
                "workflow validation failed: nodes are too simple. \
Set STEER_N8N_ALLOW_SIMPLE_WORKFLOW_FALLBACK=1 to allow orchestrator fallback."
            ));
        }
    }

    if let Some(nodes) = normalized.get_mut("nodes").and_then(|n| n.as_array_mut()) {
        for node in nodes.iter_mut() {
            let is_webhook = node
                .get("type")
                .and_then(|v| v.as_str())
                .map(|t| t == "n8n-nodes-base.webhook")
                .unwrap_or(false);
            if !is_webhook {
                continue;
            }
            if let Some(node_obj) = node.as_object_mut() {
                let missing_webhook_id = node_obj
                    .get("webhookId")
                    .and_then(|v| v.as_str())
                    .map(|v| v.trim().is_empty())
                    .unwrap_or(true);
                if missing_webhook_id {
                    node_obj.insert(
                        "webhookId".to_string(),
                        Value::String(uuid::Uuid::new_v4().to_string()),
                    );
                }
                if node_obj
                    .get("disabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    node_obj.insert("disabled".to_string(), Value::Bool(false));
                }
            }
        }
    }

    Ok(normalized)
}
