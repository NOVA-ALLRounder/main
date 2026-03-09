use super::*;

pub(crate) async fn analyze_routine(client: &OpenAILLMClient, logs: &[String]) -> Result<String> {
    if logs.is_empty() {
        return Ok("No data to analyze.".to_string());
    }

    let sample = if logs.len() > 100 {
        let mut s = logs[0..50].to_vec();
        s.extend_from_slice(&logs[logs.len() - 50..]);
        s
    } else {
        logs.to_vec()
    };

    let prompt = format!(
        "Analyze the following user activity logs (JSON) from the last 24 hours. \
        Identify any repeating patterns, routines, or habits. \
        Output a concise summary bullet list.\n\nLogs:\n{}",
        sample.join("\n")
    );

    let messages = vec![
        json!({
            "role": "system",
            "content": "You are a helpful assistant that analyzes user behavior patterns."
        }),
        json!({"role": "user", "content": prompt}),
    ];
    client.chat_completion(messages).await
}

pub(crate) async fn recommend_automation(
    client: &OpenAILLMClient,
    logs: &[String],
) -> Result<String> {
    if logs.is_empty() {
        return Ok("No data to assist recommendation.".to_string());
    }

    let sample = if logs.len() > 150 {
        let mut s = logs[0..50].to_vec();
        s.extend_from_slice(&logs[logs.len() - 100..]);
        s
    } else {
        logs.to_vec()
    };

    let prompt = format!(
        "Based on the user behavior logs (JSON) below, identify a repetitive manual task that can be automated.\n\
        Then, generate a robust BASH SCRIPT (or Python) to automate it.\n\
        \n\
        Output Format:\n\
        ### Problem\n\
        (Description)\n\
        \n\
        ### Solution\n\
        ```bash\n\
        #!/bin/bash\n\
        (Code)\n\
        ```\n\
        \n\
        Logs:\n\
        {}",
        sample.join("\n")
    );

    let messages = vec![
        json!({
            "role": "system",
            "content": "You are a pragmatic automation engineer. You write safe, effective scripts."
        }),
        json!({"role": "user", "content": prompt}),
    ];
    client.chat_completion(messages).await
}

pub(crate) async fn analyze_tendency(client: &OpenAILLMClient, logs: &[String]) -> Result<String> {
    let system_prompt = r#"
You are a User Behavior Analyst.
Analyze the following stream of user interaction logs (key presses, clicks, app focus).
Identify the user's current INTENT and TENDENCY.

Output specific, actionable intents like:
- "Writing code in Rust"
- "Debugging a Swift build error"
- "Searching for documentation on n8n"
- "Idle / Browsing social media"

If the user seems to be performing a repetitive manual task (e.g., copying data from PDF to Excel), HIGHLIGHT IT as a candidate for automation.

Output format: Just the intent description in 1-2 sentences.
"#;

    let log_text = logs.join("\n");
    let user_msg = format!("LOGS:\n{}", log_text);
    let messages = vec![
        json!({"role": "system", "content": system_prompt}),
        json!({"role": "user", "content": user_msg}),
    ];
    client.chat_completion(messages).await
}

#[allow(dead_code)]
pub(crate) async fn propose_solution_stack(client: &OpenAILLMClient, goal: &str) -> Result<Value> {
    let prompt = format!(
        "Analyze the goal and recommend a technical solution stack.\n\
        GOAL: {}\n\
        \n\
        Output JSON:\n\
        {{\n\
            \"recommended\": \"Primary Tech Stack (e.g. React + FastAPI)\",\n\
            \"alternatives\": [\"Option 2\", \"Option 3\"],\n\
            \"reasoning\": \"Why this stack is best for this goal\"\n\
        }}",
        goal
    );

    let body = json!({
        "model": client.model,
        "messages": [
            { "role": "system", "content": "You are a Solution Architect. Propose the best stack for the user's goal." },
            { "role": "user", "content": prompt }
        ],
        "response_format": { "type": "json_object" }
    });

    let res_json = client
        .post_with_retry("https://api.openai.com/v1/chat/completions", &body)
        .await?;

    let content = res_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("{}");

    let parsed: Value = serde_json::from_str(content)?;
    Ok(parsed)
}

pub(crate) async fn analyze_user_feedback(
    client: &OpenAILLMClient,
    feedback: &str,
    history_summary: &str,
) -> Result<FeedbackAnalysis> {
    let system_prompt = r#"
You are a product assistant. Analyze user feedback and decide whether to refine the goal.
Output JSON:
{
  "action": "refine" | "complete",
  "new_goal": "..." // only when action=refine
}
Guidelines:
- If feedback requests changes, clarify or adjust goal -> action=refine.
- If feedback says it's good or done -> action=complete.
- Keep new_goal short and concrete.
"#;

    let user_msg = format!("History: {}\nUser feedback: {}", history_summary, feedback);

    let request_body = json!({
        "model": client.model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_msg }
        ],
        "temperature": 0.2,
        "response_format": { "type": "json_object" }
    });

    let response = client
        .client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", client.api_key))
        .json(&request_body)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(anyhow::anyhow!("Feedback analysis error: {}", error_text));
    }

    let body: Value = response.json().await?;
    let content = body["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No content"))?;
    let parsed: Value = serde_json::from_str(content)?;

    let action = parsed["action"].as_str().unwrap_or("complete").to_string();
    let new_goal = parsed["new_goal"].as_str().map(|s| s.to_string());

    Ok(FeedbackAnalysis { action, new_goal })
}
