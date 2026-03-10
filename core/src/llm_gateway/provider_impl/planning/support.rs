use super::*;
use crate::platform::{
    app_matches_role, app_role_primary_name, current_platform,
    parse_opened_or_switched_app_history_entry, AppRole,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::GenericImageView;
use serde_json::{json, Value};

pub(super) fn list_mcp_tools() -> String {
    let mut registry = mcp_client::McpRegistry::new();
    if registry.load_config().is_err() {
        return "- none".to_string();
    }

    let tools = registry.list_all_tools();
    if tools.is_empty() {
        return "- none".to_string();
    }

    tools
        .into_iter()
        .map(|(server, tool)| format!("- {server}/{}: {}", tool.name, tool.description))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn parse_cli_action(response: &str) -> Option<Value> {
    let parsed = crate::llm_gateway::recover_json(response)?;
    if parsed.get("action").is_some_and(|value| value.is_object()) {
        Some(parsed["action"].clone())
    } else {
        Some(parsed)
    }
}

pub(super) fn maybe_downscale_image_b64(image_b64: &str) -> String {
    let max_b64 = std::env::var("STEER_OPENAI_VISION_MAX_B64")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v >= 512)
        .unwrap_or(4000);
    if image_b64.len() <= max_b64 {
        return image_b64.to_string();
    }

    let decoded = match STANDARD.decode(image_b64) {
        Ok(bytes) => bytes,
        Err(_) => return image_b64.to_string(),
    };
    let mut image = match image::load_from_memory(&decoded) {
        Ok(img) => img,
        Err(_) => return image_b64.to_string(),
    };

    for _ in 0..4 {
        let mut jpeg = Vec::new();
        {
            let mut encoder = JpegEncoder::new_with_quality(&mut jpeg, 70);
            if encoder.encode_image(&image).is_err() {
                return image_b64.to_string();
            }
        }
        let encoded = STANDARD.encode(jpeg);
        if encoded.len() <= max_b64 {
            return encoded;
        }

        let (w, h) = image.dimensions();
        let next_w = ((w as f32) * 0.75).max(320.0) as u32;
        let next_h = ((h as f32) * 0.75).max(240.0) as u32;
        if next_w >= w && next_h >= h {
            break;
        }
        image = image.resize(next_w, next_h, FilterType::Triangle);
    }

    image_b64.to_string()
}

fn extract_single_quoted_fragments(goal: &str) -> Vec<String> {
    let mut out = Vec::new();
    let parts: Vec<&str> = goal.split("'").collect();
    for (idx, part) in parts.iter().enumerate() {
        if idx % 2 == 1 {
            let trimmed = part.trim();
            if !trimmed.is_empty() {
                out.push(trimmed.to_string());
            }
        }
    }
    out
}

fn history_contains_opened_role_app(history: &[String], role: AppRole) -> bool {
    history.iter().any(|entry| {
        parse_opened_or_switched_app_history_entry(entry)
            .map(|opened| app_matches_role(current_platform().kind(), role, opened))
            .unwrap_or(false)
    })
}

pub(super) async fn fallback_plan_next_step(
    client: &OpenAILLMClient,
    goal: &str,
    _ui_tree: &Value,
    action_history: &[String],
) -> Result<Value> {
    let goal_lower = goal.to_lowercase();
    if goal_lower.contains("workflow") || goal_lower.contains("n8n") {
        return Ok(json!({
            "action": "n8n_create_workflow",
            "description": goal
        }));
    }

    if goal_lower.contains("mail")
        && history_contains_opened_role_app(action_history, AppRole::MailClient)
        && !OpenAILLMClient::history_contains_case_insensitive(action_history, "Typed")
    {
        if let Some(fragment) = extract_single_quoted_fragments(goal).first() {
            return Ok(json!({
                "action": "type",
                "text": fragment,
                "app": app_role_primary_name(current_platform().kind(), AppRole::MailClient)
            }));
        }
    }

    Ok(client.fallback_vision_action(goal, action_history))
}

pub(super) async fn fallback_plan_vision_step(
    client: &OpenAILLMClient,
    goal: &str,
    history: &[String],
) -> Result<Value> {
    Ok(client.fallback_vision_action(goal, history))
}
