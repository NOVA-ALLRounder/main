use super::support::{fallback_plan_next_step, list_mcp_tools, parse_cli_action};
use super::*;
use serde_json::json;

pub(crate) async fn plan_next_step(
    client: &OpenAILLMClient,
    goal: &str,
    ui_tree: &Value,
    action_history: &[String],
) -> Result<Value> {
    let system_prompt = format!(
        "You are a Local OS desktop automation planner. Decide the NEXT SINGLE ACTION only.\n\
Return JSON only.\n\
Allowed actions: click_visual, click_ref, type, shortcut, read, scroll, open_app, open_url, select_all, copy, paste, read_clipboard, n8n_create_workflow, n8n_execute_workflow, done, wait.\n\
Rules:\n\
- If using open_app, include a non-empty name.\n\
- If the task is incomplete, do not return done.\n\
- Prefer deterministic clipboard actions (select_all/copy/paste) over generic shortcut for clipboard flows.\n\
- For workflow requests, you may return n8n_create_workflow.\n\
- Never return markdown.\n\
Available MCP tools:\n{}",
        list_mcp_tools()
    );

    let user_prompt = format!(
        "Goal:\n{}\n\nUI Tree JSON:\n{}\n\nAction History:\n{}\n\nReturn a single JSON object describing the next action.",
        goal,
        ui_tree,
        if action_history.is_empty() {
            "(none)".to_string()
        } else {
            action_history.join("\n")
        }
    );

    let messages = vec![
        json!({ "role": "system", "content": system_prompt }),
        json!({ "role": "user", "content": user_prompt }),
    ];

    if OpenAILLMClient::cli_first_enabled() {
        let fallback_res = crate::cli_llm::fallback_chat_completion(&messages).await?;
        if let Some(action) = parse_cli_action(&fallback_res) {
            return Ok(action);
        }
        return fallback_plan_next_step(client, goal, ui_tree, action_history).await;
    }

    let body = json!({
        "model": client.model,
        "messages": messages,
        "temperature": 0.1,
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
                fallback_plan_next_step(client, goal, ui_tree, action_history).await
            }
        }
        Err(_) => fallback_plan_next_step(client, goal, ui_tree, action_history).await,
    }
}
