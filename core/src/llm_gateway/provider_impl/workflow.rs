use super::*;

pub(crate) async fn build_n8n_workflow(
    client: &OpenAILLMClient,
    user_prompt: &str,
) -> Result<String> {
    let dynamic_context = client.get_workflow_context();

    let base_prompt = r##"
You are an expert n8n Workflow Architect. Generate VALID, EXECUTABLE n8n workflow JSON.

## CRITICAL RULES
1. Output ONLY raw JSON. NO markdown, NO explanations.
2. Every node MUST have: name, type, typeVersion, position, parameters
3. Connections MUST reference existing node names exactly
4. Use REAL n8n node types from the AVAILABLE NODES section
5. ROBUSTNESS MATTERS:
   - For risky nodes (HTTP, OS Control), ensure valid inputs.
   - If using 'executeCommand', favor commands that fail gracefully or are checked.

## NODE FORMAT
{
  "name": "Unique Node Name",
  "type": "n8n-nodes-base.httpRequest",
  "typeVersion": 1,
  "position": [X, Y],
  "parameters": { ... }
}

## CONNECTION FORMAT
{
  "Source Node Name": {
    "main": [
      [{ "node": "Target Node Name", "type": "main", "index": 0 }]
    ]
  }
}

"##;

    let system_prompt = format!(
        "{}\n{}\n\nNow generate a workflow for the user request. Output ONLY the JSON.",
        base_prompt, dynamic_context
    );

    let body = json!({
        "model": client.model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ]
    });

    let content = match client
        .post_with_retry("https://api.openai.com/v1/chat/completions", &body)
        .await
    {
        Ok(res_json) => res_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("{}")
            .to_string(),
        Err(e) => {
            eprintln!(
                "⚠️ [build_n8n_workflow] OpenAI error: {}. Invoking fallback...",
                e
            );
            let messages = vec![
                json!({"role": "system", "content": system_prompt}),
                json!({"role": "user", "content": user_prompt}),
            ];
            let fallback_res = crate::cli_llm::fallback_chat_completion(&messages).await?;
            let parsed: Value = recover_json(&fallback_res)
                .ok_or_else(|| anyhow::anyhow!("Failed to parse JSON from fallback"))?;
            parsed.to_string()
        }
    };

    let clean_json = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```");

    Ok(clean_json.to_string())
}

pub(crate) async fn fix_n8n_workflow(
    client: &OpenAILLMClient,
    user_prompt: &str,
    bad_json: &str,
    error_msg: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let system_prompt = format!(
        r##"
You are an expert n8n Workflow Architect.
You previously generated a workflow that FAILED to validate or execute.
Your goal is to FIX the JSON based on the error message.

## ORIGINAL REQUEST
{}

## ERROR MESSAGE
{}

## CRITICAL RULES (RE-EMPHASIZED)
1. Output ONLY raw JSON. NO markdown.
2. Check node types and version compatibility.
3. Verify connections reference exact node names.
4. Ensure all required parameters are present.

Now output the CORRECTED JSON.
"##,
        user_prompt, error_msg
    );

    let body = json!({
        "model": client.model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": bad_json}
        ]
    });

    let res_json = client
        .post_with_retry("https://api.openai.com/v1/chat/completions", &body)
        .await?;
    let content = res_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("{}")
        .to_string();

    let clean_json = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```");
    Ok(clean_json.to_string())
}

pub(crate) async fn propose_workflow(
    client: &OpenAILLMClient,
    logs: &[String],
) -> Result<AutomationProposal, Box<dyn std::error::Error>> {
    if logs.is_empty() {
        return Ok(AutomationProposal::default());
    }

    let sample = if logs.len() > 200 {
        let mut s = logs[0..50].to_vec();
        s.extend_from_slice(&logs[logs.len() - 150..]);
        s
    } else {
        logs.to_vec()
    };

    let system_prompt = r#"
You are an expert Workflow Analyst for general office workers (Marketing, HR, Finance, Dev).
Your goal is to detect Repetitive Manual Work (Toil) from user logs and propose n8n automations.

## WHAT TO LOOK FOR (OFFICE PATTERNS)
1. "Copy-Paste Loops": User switches between Excel/Sheets and a Web Form (CRM, ERP) repeatedly.
2. "Notification Fatigue": User checks Email/Slack constantly for specific keywords (e.g., "Invoice", "Approve").
3. "File Shuffling": User downloads files (PDF/CSV) -> Renames them -> Uploads to Drive/Slack.
4. "Meeting Prep": User opens Calendar -> Opens Notion/Docs -> Copies attendees -> Writes agenda.

## OUTPUT JSON FORMAT
{
  "title": "Clear, Benefit-focused Title (e.g., 'Auto-Save Invoices to Drive')",
  "summary": "Explain the pain point and the solution (e.g., 'You check email for invoices 5 times a day. This workflow saves them to GDrive automatically.')",
  "trigger": "Trigger event (e.g., 'New Gmail with attachment')",
  "actions": ["Save to Drive", "Notify Slack", "Log to Sheet"],
  "confidence": 0.0 to 1.0 (High if pattern is clear and repetitive),
  "n8n_prompt": "Create a workflow that triggers on [Trigger], then [Action 1], then [Action 2]. Handle errors."
}

## GUIDELINES
- Avoid developer jargon if possible. Use "Save file" instead of "Binary Write".
- If logs show random browsing (YouTube, News), return confidence 0.0.
- If logs show repeated "Cmd+C" / "Cmd+V" sequences across apps, that is a HIGH confidence signal.
"#;

    let prompt = format!(
        "Logs:\n{}\n\nDecide if a workflow should be recommended.",
        sample.join("\n")
    );

    let body = json!({
        "model": client.model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.2,
        "response_format": { "type": "json_object" }
    });

    let res_json = match client
        .post_with_retry("https://api.openai.com/v1/chat/completions", &body)
        .await
    {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "⚠️ [propose_workflow] OpenAI error: {}. Invoking fallback...",
                e
            );
            let messages = vec![
                json!({"role": "system", "content": system_prompt}),
                json!({"role": "user", "content": prompt}),
            ];
            let fallback_res = crate::cli_llm::fallback_chat_completion(&messages).await?;
            let parsed: Value = recover_json(&fallback_res)
                .ok_or_else(|| anyhow::anyhow!("Failed to parse JSON from fallback"))?;

            json!({
                "choices": [{
                    "message": { "content": serde_json::to_string(&parsed).unwrap() }
                }]
            })
        }
    };
    let content = res_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("{}");

    let proposal: AutomationProposal = serde_json::from_str(content)?;
    Ok(proposal)
}

pub(crate) async fn generate_recommendation_from_pattern(
    client: &OpenAILLMClient,
    pattern_description: &str,
    sample_events: &[String],
) -> Result<AutomationProposal> {
    let system_prompt = r#"
You are a workflow automation expert. Based on the detected user behavior pattern, generate a workflow automation recommendation.

Output JSON schema:
{
  "title": "Short, descriptive title in Korean",
  "summary": "1-2 sentence description of what this automation does",
  "trigger": "What triggers this workflow (e.g., 'Gmail 새 이메일 도착', 'Downloads 폴더에 파일 생성')",
  "actions": ["Action 1", "Action 2", ...],
  "n8n_prompt": "Description for n8n workflow generation",
  "confidence": 0.0-1.0 (how confident you are this is useful)
}

Guidelines:
- Focus on practical, useful automations
- Keep it simple - 2-3 actions max
- Use Korean for user-facing text
- Set confidence low (< 0.7) if pattern seems random or not automatable
"#;

    let samples_str = sample_events
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let user_msg = format!(
        "Pattern detected: {}\n\nSample events:\n{}",
        pattern_description, samples_str
    );

    let messages = vec![
        json!({ "role": "system", "content": system_prompt }),
        json!({ "role": "user", "content": user_msg }),
    ];

    if OpenAILLMClient::cli_first_enabled() {
        let fallback_res = crate::cli_llm::fallback_chat_completion(&messages).await?;
        let parsed: Value = recover_json(&fallback_res)
            .ok_or_else(|| anyhow::anyhow!("Failed to recover JSON from CLI-first fallback"))?;
        return Ok(parse_automation_proposal(&parsed));
    }

    let request_body = json!({
        "model": client.model,
        "messages": messages.clone(),
        "temperature": 0.3,
        "response_format": { "type": "json_object" }
    });

    let body: Value = match client
        .post_with_retry("https://api.openai.com/v1/chat/completions", &request_body)
        .await
    {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "⚠️ [generate_recommendation] OpenAI error: {}. Invoking fallback...",
                e
            );
            let fallback_res = crate::cli_llm::fallback_chat_completion(&messages).await?;
            let parsed: Value = recover_json(&fallback_res)
                .ok_or_else(|| anyhow::anyhow!("Failed to recover JSON from fallback"))?;

            json!({
                "choices": [{
                    "message": {
                        "content": serde_json::to_string(&parsed).unwrap()
                    }
                }]
            })
        }
    };

    let content = body["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No content"))?;

    let parsed: Value = serde_json::from_str(content)?;
    Ok(parse_automation_proposal(&parsed))
}

fn parse_automation_proposal(parsed: &Value) -> AutomationProposal {
    AutomationProposal {
        title: parsed["title"]
            .as_str()
            .unwrap_or("Unnamed Workflow")
            .to_string(),
        summary: parsed["summary"].as_str().unwrap_or("").to_string(),
        trigger: parsed["trigger"].as_str().unwrap_or("manual").to_string(),
        actions: parsed["actions"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
        n8n_prompt: parsed["n8n_prompt"].as_str().unwrap_or("").to_string(),
        confidence: parsed["confidence"].as_f64().unwrap_or(0.5),
        evidence: vec![],
        pattern_id: None,
        category: crate::recommendation_policy::CATEGORY_UNKNOWN.to_string(),
        business_score: 0.0,
    }
}
