use super::support::{
    fallback_plan_vision_step, list_mcp_tools, maybe_downscale_image_b64, parse_cli_action,
};
use super::*;
use serde_json::json;

pub(crate) async fn plan_vision_step(
    client: &OpenAILLMClient,
    goal: &str,
    image_b64: &str,
    history: &[String],
) -> Result<Value> {
    if OpenAILLMClient::cli_first_enabled() {
        return fallback_plan_vision_step(client, goal, history).await;
    }

    let minimal_prompt = std::env::var("STEER_VISION_PROMPT_MINIMAL")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);
    let max_tokens = std::env::var("STEER_VISION_MAX_TOKENS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v >= 32 && *v <= 1024)
        .unwrap_or(64);

    let system_prompt = if minimal_prompt {
        format!(
            "You are a Local OS vision planner. Goal: {goal}. Return ONE JSON action only. Allowed actions: click_visual, type, shortcut, read, scroll, open_app, open_url, select_all, copy, paste, read_clipboard, done, wait. Available MCP tools:\n{tools}",
            goal = goal,
            tools = list_mcp_tools()
        )
    } else {
        crate::prompts::VISION_PLANNING_PROMPT
            .replace("{goal}", goal)
            .replace("{mcp_tools}", &list_mcp_tools())
    };

    let user_text = if history.is_empty() {
        "History:\n(none)\n\nReturn only one valid JSON object.".to_string()
    } else {
        format!(
            "History:\n{}\n\nReturn only one valid JSON object.",
            history.join("\n")
        )
    };

    let body = json!({
        "model": client.vision_model(),
        "messages": [
            { "role": "system", "content": system_prompt },
            {
                "role": "user",
                "content": [
                    { "type": "text", "text": user_text },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/jpeg;base64,{}", maybe_downscale_image_b64(image_b64))
                        }
                    }
                ]
            }
        ],
        "max_tokens": max_tokens,
        "response_format": { "type": "json_object" }
    });

    match client
        .post_with_retry("https://api.openai.com/v1/chat/completions", &body)
        .await
    {
        Ok(res_json) => {
            let content = res_json["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("{}");
            if let Some(action) = parse_cli_action(content) {
                Ok(action)
            } else {
                fallback_plan_vision_step(client, goal, history).await
            }
        }
        Err(_) => fallback_plan_vision_step(client, goal, history).await,
    }
}
