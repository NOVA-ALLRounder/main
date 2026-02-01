use anyhow::Result;
use reqwest::Client;
use serde_json::{json, Value};
use serde::{Serialize, Deserialize};
use std::env;
use crate::recommendation::AutomationProposal;
use crate::context_pruning;


use std::time::Duration;
use tokio::time::sleep;

#[derive(Clone)]
pub struct LLMClient {
    client: Client,
    api_key: String,
    model: String,
}

impl LLMClient {

    pub fn new() -> Result<Self> {
        dotenv::dotenv().ok(); // Load .env
        let api_key = env::var("OPENAI_API_KEY").map_err(|_| anyhow::anyhow!("OPENAI_API_KEY not set in .env"))?;
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(120))         // [Fix] Total request timeout
            .connect_timeout(Duration::from_secs(10))  // [Fix] Connection timeout
            .build()?;
        
        Ok(Self {
            client,
            api_key,
            model: "gpt-4o".to_string(),
        })
    }

    /// Internal helper for robust API calls (Retry Logic)
    async fn post_with_retry(&self, url: &str, body: &Value) -> Result<reqwest::Response> {
        let max_retries = 3;
        let mut attempt = 0;
        let mut backoff = Duration::from_secs(1);

        loop {
            attempt += 1;
            match self.client.post(url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(body)
                .send()
                .await 
            {
                Ok(resp) => {
                    if resp.status().is_server_error() || resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        // Retry on 5xx or 429
                        if attempt > max_retries {
                            return Ok(resp); // Return the error response after max retries
                        }
                    } else {
                        // Success or client error (4xx) - don't retry 4xx (except 429)
                        return Ok(resp);
                    }
                },
                Err(e) => {
                    // Network error
                    if attempt > max_retries {
                        return Err(anyhow::anyhow!("Max retries exceeded: {}", e));
                    }
                    eprintln!("⚠️ LLM Network Error (Attempt {}/{}): {}. Retrying in {:?}...", attempt, max_retries, e, backoff);
                }
            }

            sleep(backoff).await;
            backoff *= 2; // Exponential backoff (1s, 2s, 4s)
        }
    }

    #[allow(dead_code)]
    pub async fn plan_next_step(&self, goal: &str, ui_tree: &Value, action_history: &[String]) -> Result<Value> {
        let system_prompt = r#"
You are a MacOS Automation Agent. Your job is to FULLY achieve the user's goal.
You CAN control the ENTIRE computer - you can open anything, navigate anywhere.

Available Actions:

### OPENING APPS/WEBSITES:
1. Open URL: { "action": "open_url", "url": "https://..." }
2. Shell: { "action": "shell.run", "command": "..." }
3. Search Files: { "action": "system.search", "query": "..." }
4. Read File: { "action": "shell.run", "command": "cat /path/to/file.txt" }

### READING CONTENT:
5. Read Web Page: { "action": "read_page" }
6. Read UI: { "action": "ui.read" }

### UI INTERACTION:
7. Click Element: { "action": "ui.click", "element_id": "UUID" }
8. Click Text (POWERFUL): { "action": "ui.click_text", "text": "Button Label" }
9. Type: { "action": "ui.type", "text": "Hello" }

### COMPLETION:
10. Report: { "action": "report", "message": "Here's what I found: ..." }
11. Done: { "action": "done" }
12. Fail: { "action": "fail", "reason": "..." }

Output ONLY valid JSON.
"#;
        
        let history_str = if action_history.is_empty() {
            "None yet".to_string()
        } else {
            action_history.join("\n- ")
        };
        
        let user_msg = format!(
            "GOAL: {}\n\nPREVIOUS ACTIONS I'VE TAKEN:\n- {}\n\nCURRENT UI STATE:\n{}",
            goal,
            history_str,
            serde_json::to_string_pretty(ui_tree).unwrap_or_default()
        );

        let request_body = json!({
            "model": &self.model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_msg }
            ],
            "response_format": { "type": "json_object" },
            "temperature": 0.0
        });

        let response = self.post_with_retry("https://api.openai.com/v1/chat/completions", &request_body).await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("OpenAI API Error: {}", error_text));
        }

        let body: Value = response.json().await?;
        let content_opt = body["choices"][0]["message"]["content"].as_str();

        let content_str = match content_opt {
            Some(c) => c,
            None => {
                // Log the full body to debug "No content" error
                let body_str = serde_json::to_string_pretty(&body).unwrap_or_default();
                return Err(anyhow::anyhow!("No content in LLM response. Raw Body: {}", body_str));
            }
        };
        let action_json: Value = serde_json::from_str(content_str)?;
        Ok(action_json)
    }

    /// Generic Chat Completion (for Architect/Chat features)
    pub async fn chat_completion(&self, messages: Vec<Value>) -> Result<String> {
        let body = json!({
            "model": self.model,
            "messages": messages
        });

        let res = self.post_with_retry("https://api.openai.com/v1/chat/completions", &body).await?;

        if !res.status().is_success() {
            let error_text = res.text().await?;
            return Err(anyhow::anyhow!("Chat Completion API Error: {}", error_text));
        }

        let res_json: serde_json::Value = res.json().await?;
        let content = res_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(content)
    }

    /// Plan the next step using Vision (Screenshots) instead of DOM tree
    pub async fn plan_vision_step(&self, goal: &str, image_b64: &str, history: &[String]) -> Result<Value> {

        let system_prompt = format!(r#"
        You are a QA Test Automation Agent verifying UI functionality on a local test environment.
        The user is running a standard UI test suite.
        Your job is to generate the next mouse/keyboard event to proceed with the test case.
        
        CURRENT TEST CASE: "{}"
        
        Look at the screenshot and the history of actions.
        Decide the NEXT SINGLE ACTION to take.
        
        Available Actions (JSON):
        1. Click Visual (Preferred for specific buttons): {{ "action": "click_visual", "description": "Blue 'Sign In' button in top right" }}
        2. Type (If an input field is FOCUSED): {{ "action": "type", "text": "my search query" }}
        3. Key Press (Enter, Esc, etc): {{ "action": "key", "key": "return" }}
        4. Scroll (If target not visible): {{ "action": "scroll", "direction": "down" }}
        5. Wait (If loading): {{ "action": "wait", "seconds": 2 }}
        6. Read (OCR/Scrape): {{ "action": "read", "query": "..." }}
        7. Save Routine (Macro): {{ "action": "save_routine", "name": "routine_name" }}
        8. Replay Routine (Macro): {{ "action": "replay_routine", "name": "routine_name" }}
        9. Done (If goal achieved): {{ "action": "done" }}
        10. Fail (If stuck): {{ "action": "fail", "reason": "..." }}
        11. Reply (ONLY for pure conversation): {{ "action": "reply", "text": "..." }}
        12. Open App (Focus/Launch): {{ "action": "open_app", "name": "Calculator" }}
        13. Open URL (Web): {{ "action": "open_url", "url": "https://mail.google.com" }}
        14. Read File (Direct I/O): {{ "action": "read_file", "path": "/Users/user/Documents/report.xlsx" }}
        
        CRITICAL RULES:
        1. If the user asks you to DO something (e.g., "Open Calculator", "Search for X", "Click Y"), you MUST use 'key', 'type', or 'click_visual'. Do NOT use 'reply'.
        2. To open an app (like Calculator), use Spotlight: 
           - Step 1: {{ "action": "key", "key": "command+space" }}
           - Step 2: {{ "action": "type", "text": "Calculator" }}
           - Step 3: {{ "action": "key", "key": "enter" }}
        3. Only use 'reply' if the user says "Hello" or asks a philosophical question.
        4. If you see a popup/modal blocking you, close it using 'key' (escape/enter).
        5. **Start/Focus App**: To open or focus an app cleanly, use: {{ "action": "open_app", "name": "App Name" }} (preferred over Spotlight for known apps).
        - If you need to search, click the search bar first, THEN type.
        - Be precise in your visual descriptions.
        "#, goal);
        
        let history_str = if history.is_empty() {
            "None".to_string()
        } else {
            history.join("\n- ")
        };
        let user_msg = format!("GOAL: {}\n\nHISTORY:\n- {}", goal, history_str);

        let body = json!({
            "model": "gpt-4o", 
            "messages": [
                { "role": "system", "content": system_prompt },
                {
                    "role": "user",
                    "content": [
                        { "type": "text", "text": user_msg },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:image/jpeg;base64,{}", image_b64),
                                "detail": "high" // Need details for text reading
                            }
                        }
                    ]
                }
            ],
            "max_tokens": 300,
            "response_format": { "type": "json_object" }
        });

        let res = self.post_with_retry("https://api.openai.com/v1/chat/completions", &body).await?;

        if !res.status().is_success() {
            let error_text = res.text().await?;
            return Err(anyhow::anyhow!("Vision Planning API Error: {}", error_text));
        }

        let res_json: serde_json::Value = res.json().await?;
        
        // Handle Refusal (Safety Filter)
        if let Some(refusal) = res_json["choices"][0]["message"]["refusal"].as_str() {
             return Err(anyhow::anyhow!("LLM Refused (Safety): {}", refusal));
        }

        let content_opt = res_json["choices"][0]["message"]["content"].as_str();
        let content = match content_opt {
            Some(c) => c,
            None => {
                let body_str = serde_json::to_string_pretty(&res_json).unwrap_or_default();
                return Err(anyhow::anyhow!("No content in Vision LLM response. Raw Body: {}", body_str));
            }
        };

        // Sanitize content (sometimes it adds markdown code blocks)
        let clean_content = content.trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```");

        let action_json: Value = serde_json::from_str(clean_content)?;
        Ok(action_json)
    }

    pub async fn analyze_routine(&self, logs: &[String]) -> Result<String> {
        if logs.is_empty() {
            return Ok("No data to analyze.".to_string());
        }

        // Summarize logs to avoid token limit
        // Simple strategy: take first 50 and last 50 events if too many
        let sample = if logs.len() > 100 {
            let mut s = logs[0..50].to_vec();
            s.extend_from_slice(&logs[logs.len()-50..]);
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

        let body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": "You are a helpful assistant that analyzes user behavior patterns."},
                {"role": "user", "content": prompt}
            ]
        });

        let res = self.post_with_retry("https://api.openai.com/v1/chat/completions", &body).await?;

        let res_json: serde_json::Value = res.json().await?;
        let content = res_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("No analysis generated.")
            .to_string();

        Ok(content)
    }

    pub async fn recommend_automation(&self, logs: &[String]) -> Result<String> {
        if logs.is_empty() {
            return Ok("No data to assist recommendation.".to_string());
        }

        // Limit logs
        let sample = if logs.len() > 150 {
            let mut s = logs[0..50].to_vec();
            s.extend_from_slice(&logs[logs.len()-100..]); // Bias towards recent
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

        let body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": "You are a pragmatic automation engineer. You write safe, effective scripts."},
                {"role": "user", "content": prompt}
            ]
        });

        let res = self.post_with_retry("https://api.openai.com/v1/chat/completions", &body).await?;

        let res_json: serde_json::Value = res.json().await?;
        let content = res_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("No recommendation generated.")
            .to_string();

        Ok(content)
    }

    /// Build n8n workflow JSON from user prompt
    /// `context` can include: available integrations, user preferences, project-specific nodes
    pub async fn build_n8n_workflow(&self, user_prompt: &str) -> Result<String> {
        // Dynamically build context based on what's available
        let dynamic_context = self.get_workflow_context();
        
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

        // Combine base prompt with dynamic context
        let system_prompt = format!("{}\n{}\n\nNow generate a workflow for the user request. Output ONLY the JSON.", base_prompt, dynamic_context);

        let body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt}
            ]
        });

        let res = self.post_with_retry("https://api.openai.com/v1/chat/completions", &body).await?;

        let res_json: serde_json::Value = res.json().await?;
        let content = res_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("{}")
            .to_string();
            
        // Clean up markdown if model disobeys
        let clean_json = content.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```");

        Ok(clean_json.to_string())
    }

    pub async fn fix_n8n_workflow(&self, user_prompt: &str, bad_json: &str, error_msg: &str) -> Result<String, Box<dyn std::error::Error>> {
        let system_prompt = format!(r##"
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
"##, user_prompt, error_msg);

        let body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": bad_json}
            ]
        });

        let res = self.post_with_retry("https://api.openai.com/v1/chat/completions", &body).await?;

        let res_json: serde_json::Value = res.json().await?;
        let content = res_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("{}")
            .to_string();
            
        let clean_json = content.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```");
        Ok(clean_json.to_string())
    }

    /// Analyze screen content using Vision API
    pub async fn analyze_screen(&self, prompt: &str, image_b64: &str) -> Result<String, Box<dyn std::error::Error>> {
        let body = json!({
            "model": "gpt-4o", 
            "messages": [
                {
                    "role": "user",
                    "content": [
                        { "type": "text", "text": prompt },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:image/jpeg;base64,{}", image_b64)
                            }
                        }
                    ]
                }
            ],
            "max_tokens": 500
        });

        let res = self.post_with_retry("https://api.openai.com/v1/chat/completions", &body).await?;

        let res_json: serde_json::Value = res.json().await?;
        
        if let Some(err) = res_json.get("error") {
            return Err(anyhow::anyhow!("OpenAI API Error: {:?}", err).into());
        }

        let content = res_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(content)
    }

    /// Find coordinates of a UI element using Vision API
    pub async fn find_element_coordinates(&self, element_description: &str, image_b64: &str) -> Result<Option<(i32, i32)>> {
        let system_prompt = r#"
        You are a Screen Coordinate Locator.
        Analyze the screenshot and find the generic center coordinates (x, y) of the UI element described by the user.
        
        Output JSON ONLY:
        { "found": true, "x": 123, "y": 456 }
        or
        { "found": false }
        
        DO NOT output markdown or explanations.
        "#;
        
        let user_msg = format!("Find this element: {}", element_description);

        let body = json!({
            "model": "gpt-4o", 
            "messages": [
                { "role": "system", "content": system_prompt },
                {
                    "role": "user",
                    "content": [
                        { "type": "text", "text": user_msg },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:image/jpeg;base64,{}", image_b64)
                            }
                        }
                    ]
                }
            ],
            "max_tokens": 100,
            "response_format": { "type": "json_object" }
        });

        let res = self.post_with_retry("https://api.openai.com/v1/chat/completions", &body).await?;

        if !res.status().is_success() {
            let error_text = res.text().await?;
            return Err(anyhow::anyhow!("Vision API Error: {}", error_text));
        }

        let res_json: serde_json::Value = res.json().await?;
        let content = res_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("{}");

        let parsed: Value = serde_json::from_str(content)?;
        
        if parsed["found"].as_bool().unwrap_or(false) {
            let x = parsed["x"].as_i64().unwrap_or(0) as i32;
            let y = parsed["y"].as_i64().unwrap_or(0) as i32;
            Ok(Some((x, y)))
        } else {
            Ok(None)
        }
    }

    pub async fn score_quality(&self, system_prompt: &str, payload: &serde_json::Value) -> Result<String> {
        let body = json!({
            "model": "gpt-4o",
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": payload.to_string() }
            ],
            "temperature": 0.2,
            "response_format": { "type": "json_object" }
        });

        let response = self.post_with_retry("https://api.openai.com/v1/chat/completions", &body).await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Quality scoring API Error: {}", error_text));
        }

        let body: Value = response.json().await?;
        let content = body["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No content in quality scoring response"))?;
        Ok(content.to_string())
    }

    /// Dynamically build context for workflow generation based on available integrations & tools
    fn get_workflow_context(&self) -> String {
        let mut context = String::from("## AVAILABLE NODES\n");
        
        // Always available core nodes
        context.push_str("### Core Nodes (Always Available)\n");
        context.push_str("- Triggers: n8n-nodes-base.cron, n8n-nodes-base.webhook, n8n-nodes-base.manualTrigger\n");
        context.push_str("- HTTP: n8n-nodes-base.httpRequest (v4)\n");
        context.push_str("- Logic: n8n-nodes-base.if, n8n-nodes-base.switch, n8n-nodes-base.merge\n");
        context.push_str("- Data: n8n-nodes-base.set, n8n-nodes-base.code, n8n-nodes-base.function\n");
        context.push_str("- Files: n8n-nodes-base.readBinaryFiles, n8n-nodes-base.writeBinaryFile\n");
        context.push_str("- OS Control: n8n-nodes-base.executeCommand\n\n");

        context.push_str("### OS AUTOMATION CAPABILITIES\n");
        
        // Check for 'cliclick' (better mouse control)
        let has_cliclick = std::process::Command::new("which").arg("cliclick").output().map(|o| o.status.success()).unwrap_or(false);
        
        if has_cliclick {
            context.push_str("- ✅ EXACT MOUSE CONTROL: 'cliclick' IS INSTALLED.\n");
            context.push_str("  - Use: `cliclick c:x,y` (click), `cliclick dc:x,y` (double click)\n");
        } else {
            context.push_str("- ⚠️ MOUSE CONTROL: 'cliclick' is NOT installed.\n");
            context.push_str("  - PREFERRED: Use AppleScript via `osascript` for basic clicks if absolutely necessary, OR suggest installing cliclick.\n");
            context.push_str("  - Command: `osascript -e 'tell application \"System Events\" to click at {x,y}'` (Note: requires Accessibility permission)\n");
        }

        context.push_str("- ✅ KEYBOARD: Use AppleScript via `osascript`.\n");
        context.push_str("  - Command: `osascript -e 'tell application \"System Events\" to keystroke \"text\"'`\n\n");

        context.push_str("### OS AUTOMATION RULES (CRITICAL)\n");
        context.push_str("1. DO NOT invent nodes like 'n8n-nodes-base.click'. Use 'n8n-nodes-base.executeCommand'.\n");
        context.push_str("2. ALWAYS wrap OS commands in a way that handles potential permissions errors.\n");
        
        // Check for configured integrations
        context.push_str("### Configured Integrations (Prefer These)\n");
        
        // Check env vars for configured services
        if std::env::var("GOOGLE_CLIENT_ID").is_ok() || std::env::var("GMAIL_CREDENTIALS").is_ok() {
            context.push_str("- ✅ Gmail: n8n-nodes-base.gmail, n8n-nodes-base.gmailTrigger (CONFIGURED)\n");
            context.push_str("- ✅ Google Calendar: n8n-nodes-base.googleCalendar (CONFIGURED)\n");
            context.push_str("- ✅ Google Sheets: n8n-nodes-base.googleSheets (CONFIGURED)\n");
        }
        
        if std::env::var("SLACK_TOKEN").is_ok() || std::env::var("SLACK_WEBHOOK").is_ok() {
            context.push_str("- ✅ Slack: n8n-nodes-base.slack (CONFIGURED)\n");
        }
        
        if std::env::var("TELEGRAM_BOT_TOKEN").is_ok() {
            context.push_str("- ✅ Telegram: n8n-nodes-base.telegram (CONFIGURED)\n");
        }
        
        if std::env::var("NOTION_API_KEY").is_ok() {
            context.push_str("- ✅ Notion: n8n-nodes-base.notion (CONFIGURED)\n");
        }
        
        if std::env::var("OPENAI_API_KEY").is_ok() {
            context.push_str("- ✅ OpenAI: @n8n/n8n-nodes-langchain.openAi (CONFIGURED)\n");
        }
        
        // Other common nodes that can be added without credentials
        context.push_str("\n### Other Popular Nodes\n");
        context.push_str("- Discord: n8n-nodes-base.discord\n");
        context.push_str("- GitHub: n8n-nodes-base.github\n");
        context.push_str("- Airtable: n8n-nodes-base.airtable\n");
        context.push_str("- RSS: n8n-nodes-base.rssFeedRead\n");
        context.push_str("- Wait: n8n-nodes-base.wait\n");
        context.push_str("- DateTime: n8n-nodes-base.dateTime\n");
        
        context
    }

    pub async fn propose_workflow(&self, logs: &[String]) -> Result<AutomationProposal, Box<dyn std::error::Error>> {
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
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.2,
            "response_format": { "type": "json_object" }
        });

        let res = self.post_with_retry("https://api.openai.com/v1/chat/completions", &body).await?;

        if !res.status().is_success() {
            let error_text = res.text().await?;
            return Err(anyhow::anyhow!("Proposal API Error: {}", error_text).into());
        }

        let res_json: serde_json::Value = res.json().await?;
        let content = res_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("{}");

        let proposal: AutomationProposal = serde_json::from_str(content)?;
        Ok(proposal)
    }

    pub async fn analyze_tendency(&self, logs: &[String]) -> Result<String> {
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

        let request_body = json!({
            "model": "gpt-4o", // Strong model for reasoning
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_msg }
            ],
            "temperature": 0.3
        });

        let response = self.post_with_retry("https://api.openai.com/v1/chat/completions", &request_body).await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Analysis API Error: {}", error_text));
        }

        let body: Value = response.json().await?;
        let content = body["choices"][0]["message"]["content"].as_str()
            .ok_or_else(|| anyhow::anyhow!("No content"))?;
            
        Ok(content.to_string())
    }

    /// Parse natural language input into a structured command
    #[allow(dead_code)]
    pub async fn parse_intent(&self, user_input: &str) -> Result<Value> {
        self.parse_intent_with_history(user_input, &[]).await
    }

    pub async fn parse_intent_with_history(&self, user_input: &str, history: &[crate::db::ChatMessage]) -> Result<Value> {
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
        
        // Construct messages array with history (pruned)
        let mut messages = Vec::new();
        messages.push(json!({ "role": "system", "content": system_prompt }));

        let pruned_history = context_pruning::prune_chat_history(history);

        // Add history (already pruned by TTL/max count)
        for msg in pruned_history.iter() {
            // Map 'role' to OpenAI roles
            let role = if msg.role == "user" { "user" } else { "assistant" };
            messages.push(json!({ "role": role, "content": msg.content }));
        }

        // Add current user message
        messages.push(json!({ "role": "user", "content": user_input }));

        let request_body = json!({
            "model": "gpt-4o-mini",
            "messages": messages,
            "temperature": 0.1,
            "response_format": { "type": "json_object" }
        });

        let response = self.post_with_retry("https://api.openai.com/v1/chat/completions", &request_body).await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Intent Parse API Error: {}", error_text));
        }

        let body: Value = response.json().await?;
        let content = body["choices"][0]["message"]["content"].as_str()
            .ok_or_else(|| anyhow::anyhow!("No content"))?;
            
        let parsed: Value = serde_json::from_str(content)?;
        Ok(parsed)
    }

    /// Generate embeddings for RAG
    pub async fn get_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let request_body = json!({
            "model": "text-embedding-3-small",
            "input": text,
            //"dimensions": 1536 // Default
        });

        let response = self.post_with_retry("https://api.openai.com/v1/embeddings", &request_body).await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Embedding API Error: {}", error_text));
        }

        let body: Value = response.json().await?;
        let vector = body["data"][0]["embedding"].as_array()
            .ok_or_else(|| anyhow::anyhow!("Invalid embedding response"))?
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect();
        
        Ok(vector)
    }

    /// Generate workflow recommendation from detected pattern
    pub async fn generate_recommendation_from_pattern(&self, pattern_description: &str, sample_events: &[String]) -> Result<AutomationProposal> {
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

        let samples_str = sample_events.iter().take(3).cloned().collect::<Vec<_>>().join("\n");
        let user_msg = format!(
            "Pattern detected: {}\n\nSample events:\n{}",
            pattern_description, samples_str
        );

        let request_body = json!({
            "model": "gpt-4o-mini",
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_msg }
            ],
            "temperature": 0.3,
            "response_format": { "type": "json_object" }
        });

        let response = self.post_with_retry("https://api.openai.com/v1/chat/completions", &request_body).await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Recommendation generation error: {}", error_text));
        }

        let body: Value = response.json().await?;
        let content = body["choices"][0]["message"]["content"].as_str()
            .ok_or_else(|| anyhow::anyhow!("No content"))?;
            
        let parsed: Value = serde_json::from_str(content)?;
        
        Ok(AutomationProposal {
            title: parsed["title"].as_str().unwrap_or("Unnamed Workflow").to_string(),
            summary: parsed["summary"].as_str().unwrap_or("").to_string(),
            trigger: parsed["trigger"].as_str().unwrap_or("manual").to_string(),
            actions: parsed["actions"].as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
            n8n_prompt: parsed["n8n_prompt"].as_str().unwrap_or("").to_string(),
            confidence: parsed["confidence"].as_f64().unwrap_or(0.5),
            evidence: vec![], // Populated by caller
            pattern_id: None, // Populated by caller
        })
    }


    /// Proactively suggest a tech stack or approach for a goal (Transformers7 feature)
    #[allow(dead_code)]
    pub async fn propose_solution_stack(&self, goal: &str) -> Result<Value> {
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
            "model": "gpt-4o",
            "messages": [
                { "role": "system", "content": "You are a Solution Architect. Propose the best stack for the user's goal." },
                { "role": "user", "content": prompt }
            ],
            "response_format": { "type": "json_object" }
        });

        let res = self.post_with_retry("https://api.openai.com/v1/chat/completions", &body).await?;

        let res_json: serde_json::Value = res.json().await?;
        let content = res_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("{}");
            
        let parsed: Value = serde_json::from_str(content)?;
        Ok(parsed)
    }

    /// Run inference on Local LLM (Ollama)
    #[allow(dead_code)]
    pub async fn inference_local(&self, prompt: &str, model: Option<&str>) -> Result<String> {
        let model_name = model.unwrap_or("llama3"); // Default to llama3 or user pref
        
        let body = json!({
            "model": model_name,
            "prompt": prompt,
            "stream": false
        });

        // Default Ollama local URL
        let url = "http://localhost:11434/api/generate";

        let res = self.client.post(url)
            .json(&body)
            .send()
            .await;

        match res {
            Ok(response) => {
                if !response.status().is_success() {
                    let err_text = response.text().await.unwrap_or_default();
                    return Err(anyhow::anyhow!("Ollama API Error: {}", err_text));
                }
                
                let val: Value = response.json().await?;
                let content = val["response"].as_str().unwrap_or("").to_string();
                Ok(content)
            },
            Err(e) => {
                // Connecting to localhost might fail if Ollama is not running.
                Err(anyhow::anyhow!("Failed to connect to Local LLM (Ollama): {}", e))
            }
        }
    }

    /// Smart Router: Decide between Cloud (OpenAI) and Local (Ollama)
    /// Returns: (use_local: bool, model_name: &str)
    pub fn route_task(&self, task_description: &str, pii_detected: bool) -> (bool, String) {
        // Rule 1: Privacy First
        if pii_detected {
            return (true, "llama3".to_string());
        }

        // Rule 2: Complexity Check (Simple Heuristic for Phase 4)
        // If task is short/simple -> Local
        // If task implies deep reasoning ("Plan", "Analyze", "Code") -> Cloud
        let lower = task_description.to_lowercase();
        if lower.contains("plan") || lower.contains("analyze") || lower.contains("code") || lower.contains("debug") {
             return (false, "gpt-4o".to_string());
        }

        // Default to Local for simple chat/summarization to save cost
        (true, "llama3".to_string())
    }

    pub async fn analyze_user_feedback(&self, feedback: &str, history_summary: &str) -> Result<FeedbackAnalysis> {
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
            "model": "gpt-4o-mini",
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_msg }
            ],
            "temperature": 0.2,
            "response_format": { "type": "json_object" }
        });

        let response = self.client.post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Feedback analysis error: {}", error_text));
        }

        let body: Value = response.json().await?;
        let content = body["choices"][0]["message"]["content"].as_str()
            .ok_or_else(|| anyhow::anyhow!("No content"))?;
        let parsed: Value = serde_json::from_str(content)?;

        let action = parsed["action"].as_str().unwrap_or("complete").to_string();
        let new_goal = parsed["new_goal"].as_str().map(|s| s.to_string());

        Ok(FeedbackAnalysis { action, new_goal })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackAnalysis {
    pub action: String,
    pub new_goal: Option<String>,
}

/// [Phase 28] Streaming Chat Completion
/// Returns chunks via callback for real-time UI updates
impl LLMClient {
    pub async fn chat_completion_stream<F>(
        &self,
        messages: Vec<Value>,
        mut on_chunk: F,
    ) -> Result<String>
    where
        F: FnMut(&str) + Send,
    {
        use futures::StreamExt;
        
        let body = json!({
            "model": self.model,
            "messages": messages,
            "stream": true
        });

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Stream API Error: {}", error_text));
        }

        let mut full_content = String::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            let chunk_str = String::from_utf8_lossy(&chunk);
            
            // Parse SSE lines
            for line in chunk_str.lines() {
                if line.starts_with("data: ") {
                    let data = &line[6..];
                    if data == "[DONE]" {
                        break;
                    }
                    if let Ok(parsed) = serde_json::from_str::<Value>(data) {
                        if let Some(delta) = parsed["choices"][0]["delta"]["content"].as_str() {
                            full_content.push_str(delta);
                            on_chunk(delta);
                        }
                    }
                }
            }
        }

        Ok(full_content)
    }
}
