use super::*;

impl OpenAILLMClient {
    pub fn new() -> Result<Self> {
        crate::load_env_with_fallback();
        let api_key = env::var("OPENAI_API_KEY")
            .map_err(|_| anyhow::anyhow!("OPENAI_API_KEY not set in .env"))?;
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            .build()?;

        let default_model = env::var("STEER_OPENAI_MODEL")
            .or_else(|_| env::var("OPENAI_MODEL"))
            .ok()
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| "gpt-4o-mini".to_string());

        Ok(Self {
            client,
            api_key,
            model: default_model,
        })
    }

    pub(crate) fn llm_primary_mode() -> Option<String> {
        env::var("STEER_LLM_PRIMARY")
            .ok()
            .map(|v| v.trim().to_lowercase())
            .filter(|v| !v.is_empty())
    }

    pub(crate) fn cli_first_enabled() -> bool {
        matches!(
            Self::llm_primary_mode().as_deref(),
            Some("cli")
                | Some("codex")
                | Some("gemini")
                | Some("claude")
                | Some("local")
                | Some("llama")
        )
    }

    pub(crate) fn vision_model(&self) -> String {
        env::var("STEER_VISION_MODEL")
            .ok()
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| self.model.clone())
    }

    pub(crate) fn history_contains_case_insensitive(history: &[String], needle: &str) -> bool {
        let needle_lower = needle.to_lowercase();
        history
            .iter()
            .any(|h| h.to_lowercase().contains(&needle_lower))
    }

    pub(crate) fn ordered_apps_in_goal(goal: &str) -> Vec<&'static str> {
        let goal_lower = goal.to_lowercase();
        let app_catalog: [&'static str; 7] = [
            "Calendar",
            "Safari",
            "Finder",
            "TextEdit",
            "Notes",
            "Calculator",
            "Mail",
        ];

        let mut found: Vec<(usize, &'static str)> = app_catalog
            .iter()
            .filter_map(|app| goal_lower.find(&app.to_lowercase()).map(|idx| (idx, *app)))
            .collect();
        found.sort_by_key(|(idx, _)| *idx);
        found.into_iter().map(|(_, app)| app).collect()
    }

    fn cmd_sequence_from_goal(goal: &str) -> Vec<char> {
        let text = goal.to_lowercase();
        let mut seq = Vec::new();
        let mut start = 0usize;

        while let Some(rel) = text[start..].find("cmd+") {
            let pos = start + rel + 4;
            let rest = &text[pos..];
            let mut pushed = false;
            for ch in rest.chars() {
                if ch.is_ascii_whitespace() {
                    continue;
                }
                if matches!(ch, 'a' | 'c' | 'v' | 'n') {
                    seq.push(ch);
                }
                pushed = true;
                break;
            }
            if pushed {
                start = pos.saturating_add(1);
            } else {
                break;
            }
        }

        seq
    }

    fn cmd_sequence_from_history(history: &[String]) -> Vec<char> {
        let mut seq = Vec::new();
        for entry in history {
            let lower = entry.to_lowercase();
            if lower.contains("selected all contents") {
                seq.push('a');
                continue;
            }
            if lower.contains("copied selection") {
                seq.push('c');
                continue;
            }
            if lower.contains("pasted clipboard contents") {
                seq.push('v');
                continue;
            }
            if lower.contains("shortcut 'n'") && lower.contains("command") {
                seq.push('n');
                continue;
            }
            if lower.contains("shortcut 'a'") && lower.contains("command") {
                seq.push('a');
                continue;
            }
            if lower.contains("shortcut 'c'") && lower.contains("command") {
                seq.push('c');
                continue;
            }
            if lower.contains("shortcut 'v'") && lower.contains("command") {
                seq.push('v');
                continue;
            }
        }
        seq
    }

    fn next_missing_cmd(goal: &str, history: &[String]) -> Option<char> {
        let expected = Self::cmd_sequence_from_goal(goal);
        if expected.is_empty() {
            return None;
        }
        let actual = Self::cmd_sequence_from_history(history);
        let mut matched_prefix = 0usize;
        for c in actual {
            if matched_prefix < expected.len() && c == expected[matched_prefix] {
                matched_prefix += 1;
            }
        }
        expected.get(matched_prefix).copied()
    }

    fn history_count_case_insensitive(history: &[String], needle: &str) -> usize {
        let needle_lower = needle.to_lowercase();
        history
            .iter()
            .filter(|h| h.to_lowercase().contains(&needle_lower))
            .count()
    }

    fn extract_single_quoted_fragments(goal: &str) -> Vec<String> {
        let mut out = Vec::new();
        let parts: Vec<&str> = goal.split('\'').collect();
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

    fn first_missing_fragment_by_keywords(
        goal: &str,
        history: &[String],
        keywords: &[&str],
    ) -> Option<String> {
        let history_blob = history.join("\n").to_lowercase();
        Self::extract_single_quoted_fragments(goal)
            .into_iter()
            .find(|frag| {
                let frag_lower = frag.to_lowercase();
                let keyword_match = keywords
                    .iter()
                    .any(|k| frag_lower.contains(&k.to_lowercase()));
                keyword_match && !history_blob.contains(&frag_lower)
            })
    }

    pub(crate) fn fallback_vision_action(&self, goal: &str, history: &[String]) -> Value {
        let goal_lower = goal.to_lowercase();

        if let Some(next_cmd) = Self::next_missing_cmd(goal, history) {
            let action = match next_cmd {
                'n' => json!({ "action": "shortcut", "key": "n", "modifiers": ["command"] }),
                'a' => json!({ "action": "select_all" }),
                'c' => {
                    let copy_count =
                        Self::history_count_case_insensitive(history, "Copied selection");
                    let calculator_opened =
                        Self::history_contains_case_insensitive(history, "Opened app: Calculator");
                    if copy_count >= 1 && !calculator_opened {
                        if let Some(status_text) = Self::first_missing_fragment_by_keywords(
                            goal,
                            history,
                            &["상태", "status"],
                        ) {
                            json!({ "action": "type", "text": status_text })
                        } else {
                            json!({ "action": "open_app", "name": "Calculator" })
                        }
                    } else {
                        json!({ "action": "copy" })
                    }
                }
                'v' => {
                    let calculator_opened =
                        Self::history_contains_case_insensitive(history, "Opened app: Calculator");
                    if calculator_opened {
                        if let Some(cost_text) = Self::first_missing_fragment_by_keywords(
                            goal,
                            history,
                            &["예상비용", "cost", "budget"],
                        ) {
                            json!({ "action": "type", "text": cost_text })
                        } else {
                            json!({ "action": "paste" })
                        }
                    } else {
                        json!({ "action": "paste" })
                    }
                }
                _ => json!({ "action": "wait", "seconds": 1 }),
            };
            return action;
        }

        for app in Self::ordered_apps_in_goal(goal) {
            let opened_marker = format!("Opened app: {}", app);
            if !Self::history_contains_case_insensitive(history, &opened_marker) {
                return json!({ "action": "open_app", "name": app });
            }
        }

        if goal_lower.contains("google")
            || goal_lower.contains("검색")
            || goal_lower.contains("search")
        {
            return json!({ "action": "open_url", "url": "https://www.google.com" });
        }

        json!({ "action": "wait", "seconds": 1 })
    }

    pub async fn post_with_retry(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, anyhow::Error> {
        let max_retries = std::env::var("STEER_OPENAI_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v >= 1 && *v <= 10)
            .unwrap_or(1);
        let retry_429_sec = std::env::var("STEER_OPENAI_429_RETRY_SEC")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v >= 1 && *v <= 120)
            .unwrap_or(2);
        let mut attempt = 0;
        let mut backoff = tokio::time::Duration::from_secs(1);

        loop {
            attempt += 1;
            let mut wait_override: Option<tokio::time::Duration> = None;

            let req = self
                .client
                .post(url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(body);

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        return Ok(resp.json().await?);
                    }

                    let headers = resp.headers().clone();
                    let error_text = resp.text().await.unwrap_or_default();

                    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        if error_text.to_lowercase().contains("quota")
                            || error_text.to_lowercase().contains("billing")
                            || error_text.contains("access_terminated")
                        {
                            return Err(anyhow::anyhow!("RATE_LIMITED_QUOTA: {}", error_text));
                        }

                        if attempt > max_retries {
                            return Err(anyhow::anyhow!(
                                "RATE_LIMITED_EXHAUSTED: OpenAI 429 after {} retries. Error: {}",
                                max_retries,
                                error_text
                            ));
                        }

                        let retry_after_header = headers
                            .get(reqwest::header::RETRY_AFTER)
                            .and_then(|v| v.to_str().ok())
                            .and_then(|s| s.trim().parse::<u64>().ok())
                            .filter(|v| *v >= 1 && *v <= 300);
                        let retry_after = retry_after_header.unwrap_or(retry_429_sec);
                        wait_override = Some(tokio::time::Duration::from_secs(retry_after));
                        eprintln!(
                            "⚠️ LLM rate limited (429). Body: '{}'. Retrying in {}s (attempt {}/{})...",
                            error_text.replace('\n', " "),
                            retry_after,
                            attempt,
                            max_retries
                        );
                    } else if status.is_server_error() {
                        if attempt > max_retries {
                            return Err(anyhow::anyhow!("HTTP {}: {}", status, error_text));
                        }
                    } else {
                        return Err(anyhow::anyhow!("HTTP {}: {}", status, error_text));
                    }
                }
                Err(e) => {
                    if attempt > max_retries {
                        return Err(anyhow::anyhow!("Max retries exceeded: {}", e));
                    }
                    eprintln!(
                        "⚠️ LLM Network Error (Attempt {}/{}): {}. Retrying in {:?}...",
                        attempt, max_retries, e, backoff
                    );
                }
            }

            let sleep_for = wait_override.unwrap_or(backoff);
            tokio::time::sleep(sleep_for).await;
            backoff = std::cmp::max(backoff * 2, sleep_for);
        }
    }

    pub(crate) fn get_workflow_context(&self) -> String {
        let mut context = String::from("## AVAILABLE NODES\n");

        context.push_str("### Core Nodes (Always Available)\n");
        context.push_str("- Triggers: n8n-nodes-base.cron, n8n-nodes-base.webhook, n8n-nodes-base.manualTrigger\n");
        context.push_str("- HTTP: n8n-nodes-base.httpRequest (v4)\n");
        context
            .push_str("- Logic: n8n-nodes-base.if, n8n-nodes-base.switch, n8n-nodes-base.merge\n");
        context
            .push_str("- Data: n8n-nodes-base.set, n8n-nodes-base.code, n8n-nodes-base.function\n");
        context
            .push_str("- Files: n8n-nodes-base.readBinaryFiles, n8n-nodes-base.writeBinaryFile\n");
        context.push_str("- OS Control: n8n-nodes-base.executeCommand\n\n");

        context.push_str("### OS AUTOMATION CAPABILITIES\n");

        let has_cliclick = std::process::Command::new("which")
            .arg("cliclick")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if has_cliclick {
            context.push_str("- ✅ EXACT MOUSE CONTROL: 'cliclick' IS INSTALLED.\n");
            context
                .push_str("  - Use: `cliclick c:x,y` (click), `cliclick dc:x,y` (double click)\n");
        } else {
            context.push_str("- ⚠️ MOUSE CONTROL: 'cliclick' is NOT installed.\n");
            context.push_str("  - PREFERRED: Use AppleScript via `osascript` for basic clicks if absolutely necessary, OR suggest installing cliclick.\n");
            context.push_str("  - Command: `osascript -e 'tell application \"System Events\" to click at {x,y}'` (Note: requires Accessibility permission)\n");
        }

        context.push_str("- ✅ KEYBOARD: Use AppleScript via `osascript`.\n");
        context.push_str("  - Command: `osascript -e 'tell application \"System Events\" to keystroke \"text\"'`\n\n");

        context.push_str("### OS AUTOMATION RULES (CRITICAL)\n");
        context.push_str("1. DO NOT invent nodes like 'n8n-nodes-base.click'. Use 'n8n-nodes-base.executeCommand'.\n");
        context.push_str(
            "2. ALWAYS wrap OS commands in a way that handles potential permissions errors.\n",
        );

        context.push_str("### Configured Integrations (Prefer These)\n");

        if std::env::var("GOOGLE_CLIENT_ID").is_ok() || std::env::var("GMAIL_CREDENTIALS").is_ok() {
            context.push_str(
                "- ✅ Gmail: n8n-nodes-base.gmail, n8n-nodes-base.gmailTrigger (CONFIGURED)\n",
            );
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

        context.push_str("\n### Other Popular Nodes\n");
        context.push_str("- Discord: n8n-nodes-base.discord\n");
        context.push_str("- GitHub: n8n-nodes-base.github\n");
        context.push_str("- Airtable: n8n-nodes-base.airtable\n");
        context.push_str("- RSS: n8n-nodes-base.rssFeedRead\n");
        context.push_str("- Wait: n8n-nodes-base.wait\n");
        context.push_str("- DateTime: n8n-nodes-base.dateTime\n");

        context
    }
}
