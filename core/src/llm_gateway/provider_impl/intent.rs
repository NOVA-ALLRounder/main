use super::*;

pub(crate) async fn parse_intent(client: &OpenAILLMClient, user_input: &str) -> Result<Value> {
    parse_intent_with_history(client, user_input, &[]).await
}

pub(crate) async fn parse_intent_with_history(
    client: &OpenAILLMClient,
    user_input: &str,
    history: &[crate::db::ChatMessage],
) -> Result<Value> {
    let system_prompt = r#"
You are a command parser for a Local OS Agent. Convert natural language into structured commands.

Available commands:
- gmail_list: List recent emails. Params: count (number, default 5)
- gmail_read: Read a specific email. Params: id (string)
- gmail_send: Send email. Params: to, subject, body
- calendar_today: Show today's events. No params.
- calendar_week: Show this week's events. No params.
- calendar_add: Add calendar event. Params: title, start, end
- telegram_send: Send telegram message. Params: message
- notion_create: Create notion page. Params: title, content
- build_workflow: Create n8n automation. Params: description
- create_routine: Schedule recurring task. Params: cron (CRON format e.g., '0 9 * * *'), prompt (instruction), name (short title)
- system_status: Show system status. No params.
- help: Show help. No params.
- unknown: Cannot parse. Params: original_text

Return JSON only:
{
  "command": "command_name",
  "params": { ... },
  "confidence": 0.0-1.0
}
"#;

    let mut messages = Vec::new();
    messages.push(json!({ "role": "system", "content": system_prompt }));

    let pruned_history = crate::context_pruning::prune_chat_history(history);
    for msg in &pruned_history {
        let role = if msg.role == "user" {
            "user"
        } else {
            "assistant"
        };
        messages.push(json!({ "role": role, "content": msg.content }));
    }
    messages.push(json!({ "role": "user", "content": user_input }));

    if OpenAILLMClient::cli_first_enabled() {
        let fallback_res = crate::cli_llm::fallback_chat_completion(&messages).await?;
        let parsed = recover_json(&fallback_res)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse JSON from CLI-first fallback"))?;
        return Ok(parsed);
    }

    let request_body = json!({
        "model": client.model,
        "messages": messages.clone(),
        "temperature": 0.1,
        "response_format": { "type": "json_object" }
    });

    let body: Value = match client
        .post_with_retry("https://api.openai.com/v1/chat/completions", &request_body)
        .await
    {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "⚠️ [parse_intent] OpenAI error: {}. Invoking fallback...",
                e
            );
            let fallback_res = crate::cli_llm::fallback_chat_completion(&messages).await?;
            let parsed = recover_json(&fallback_res)
                .ok_or_else(|| anyhow::anyhow!("Failed to parse JSON from fallback"))?;
            return Ok(parsed);
        }
    };

    let content = body["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No content"))?;

    let parsed: Value = serde_json::from_str(content)?;
    Ok(parsed)
}

pub(crate) async fn get_embedding(client: &OpenAILLMClient, text: &str) -> Result<Vec<f32>> {
    let request_body = json!({
        "model": "text-embedding-3-small",
        "input": text,
    });

    let body: Value = client
        .post_with_retry("https://api.openai.com/v1/embeddings", &request_body)
        .await?;

    let vector = body["data"][0]["embedding"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Invalid embedding response"))?
        .iter()
        .map(|v| v.as_f64().unwrap_or(0.0) as f32)
        .collect();

    Ok(vector)
}

pub(crate) async fn inference_local(
    client: &OpenAILLMClient,
    prompt: &str,
    model: Option<&str>,
) -> Result<String> {
    let model_name = model.unwrap_or("llama3");

    let body = json!({
        "model": model_name,
        "prompt": prompt,
        "stream": false
    });

    let url = "http://localhost:11434/api/generate";
    let res = client.client.post(url).json(&body).send().await;

    match res {
        Ok(response) => {
            if !response.status().is_success() {
                let err_text = response.text().await.unwrap_or_default();
                return Err(anyhow::anyhow!("Ollama API Error: {}", err_text));
            }

            let val: Value = response.json().await?;
            let content = val["response"].as_str().unwrap_or("").to_string();
            Ok(content)
        }
        Err(e) => Err(anyhow::anyhow!(
            "Failed to connect to Local LLM (Ollama): {}",
            e
        )),
    }
}

pub(crate) fn route_task(
    client: &OpenAILLMClient,
    task_description: &str,
    pii_detected: bool,
) -> (bool, String) {
    if pii_detected {
        return (true, "llama3".to_string());
    }

    let lower = task_description.to_lowercase();
    if lower.contains("plan")
        || lower.contains("analyze")
        || lower.contains("code")
        || lower.contains("debug")
    {
        return (false, client.model.clone());
    }

    (true, "llama3".to_string())
}
