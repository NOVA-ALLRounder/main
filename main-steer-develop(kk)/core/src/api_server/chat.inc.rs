pub(super) async fn handle_chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Json<ChatResponse> {
    let gate = crate::chat_gate::ChatGateConfig::from_env();
    let gate_ctx = crate::chat_gate::ChatGateContext {
        channel: req.channel.clone(),
        chat_type: req.chat_type.clone(),
        sender: req.sender.clone(),
        mentioned: req.mentioned,
    };
    if !gate.is_allowed(&gate_ctx) {
        return Json(ChatResponse {
            response: "Chat is currently blocked by gate policy for this channel/type.".to_string(),
            command: None,
        });
    }

    let sanitized = chat_sanitize::sanitize_chat_input(&req.message);
    if !sanitized.flags.is_empty() {
        eprintln!("Chat sanitize flags: {:?}", sanitized.flags);
    }
    let message = sanitized.text.trim().to_string();
    if message.is_empty() {
        return Json(ChatResponse {
            response: "Please enter a message.".to_string(),
            command: None,
        });
    }

    if let Err(e) = db::insert_chat_message("user", &message) {
        eprintln!("Failed to save user chat: {}", e);
    }

    let lower = message.to_lowercase();
    let ko_hello = "\u{C548}\u{B155}";
    let ko_hello_formal = "\u{C548}\u{B155}\u{D558}\u{C138}\u{C694}";
    let ko_calendar = "\u{C77C}\u{C815}";
    let ko_week = "\u{C774}\u{BC88}\u{C8FC}";
    let ko_mail = "\u{BA54}\u{C77C}";
    let ko_status = "\u{C0C1}\u{D0DC}";
    let ko_notepad = "\u{BA54}\u{BAA8}\u{C7A5}";
    let ko_calc = "\u{ACC4}\u{C0B0}\u{AE30}";
    let ko_explorer = "\u{D0D0}\u{C0C9}\u{AE30}";
    let ko_open = "\u{C5F4}\u{C5B4}";
    let ko_search = "\u{AC80}\u{C0C9}";
    let ko_mail_workflow = "\u{BA54}\u{C77C} \u{C6CC}\u{D06C}\u{D50C}\u{B85C}\u{C6B0}";
    let ko_mail_work = "\u{BA54}\u{C77C} \u{C5C5}\u{BB34}";
    let ko_execute = "\u{C2E4}\u{D589}";
    let ko_template = "\u{C0D8}\u{D50C}";

    if lower.contains(ko_hello)
        || lower.contains(ko_hello_formal)
        || lower == "hi"
        || lower == "hello"
        || lower == "hey"
    {
        return Json(ChatResponse {
            response: "Hello. Try: system status, today calendar, recent email, open notepad".to_string(),
            command: Some("smalltalk_greeting".to_string()),
        });
    }

    if message == "analyze_patterns" {
        let results = run_analysis_internal();
        let response = if results.is_empty() {
            "No repetitive pattern detected yet.".to_string()
        } else {
            format!("Detected {} pattern(s):\n{}", results.len(), results.join("\n"))
        };
        return Json(ChatResponse {
            response,
            command: Some("analyze_patterns".to_string()),
        });
    }

    if lower.contains(ko_calendar) || lower.contains("calendar") {
        let week_mode = lower.contains(ko_week) || lower.contains("week");
        let response = match integrations::calendar::CalendarClient::new().await {
            Ok(client) => {
                let events = if week_mode { client.list_week().await } else { client.list_today().await };
                match events {
                    Ok(items) if items.is_empty() => {
                        if week_mode { "No events this week.".to_string() } else { "No events today.".to_string() }
                    }
                    Ok(items) => {
                        let mut out = if week_mode {
                            String::from("This week's events:\n")
                        } else {
                            String::from("Today's events:\n")
                        };
                        for (_, summary, start_time) in items {
                            out.push_str(&format!("- {} ({})\n", summary, start_time));
                        }
                        out
                    }
                    Err(e) => format!("Calendar query failed: {}", e),
                }
            }
            Err(e) => format!("Calendar connection failed: {}", e),
        };
        return Json(ChatResponse {
            response,
            command: Some(if week_mode { "calendar_week" } else { "calendar_today" }.to_string()),
        });
    }

    if (lower.contains(ko_mail) || lower.contains("gmail") || lower.contains("email") || lower.contains("mail"))
        && !lower.contains("email workflow")
        && !lower.contains("mail workflow")
        && !lower.contains(ko_mail_workflow)
        && !lower.contains(ko_mail_work)
    {
        let response = match integrations::gmail::GmailClient::new().await {
            Ok(client) => match client.list_messages(5).await {
                Ok(items) if items.is_empty() => "No recent emails.".to_string(),
                Ok(items) => {
                    let mut out = String::from("Recent 5 emails:\n");
                    for (_, subject, from) in items {
                        out.push_str(&format!("- {} ({})\n", subject, from));
                    }
                    out
                }
                Err(e) => format!("Email query failed: {}", e),
            },
            Err(e) => format!("Gmail connection failed: {}", e),
        };
        return Json(ChatResponse {
            response,
            command: Some("gmail_list".to_string()),
        });
    }

    if lower.contains(ko_status) || lower == "status" || lower.contains("system status") {
        let mut rm = monitor::ResourceMonitor::new();
        return Json(ChatResponse {
            response: format!("System status: {}", rm.get_status()),
            command: Some("system_status".to_string()),
        });
    }

    if cfg!(target_os = "windows") {
        let runtime_ctl = state
            .runtime_control
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| crate::runtime_mode::RuntimeControl::from_env());
        let automation_allowed = runtime_ctl.allow_automation();
        let mode_name = runtime_ctl.mode.as_str().to_string();
        let emergency_stop = runtime_ctl.emergency_stop;

        let blocked_automation = |feature: &str| -> Json<ChatResponse> {
            Json(ChatResponse {
                response: format!(
                    "Automation '{}' blocked (mode={}, emergency_stop={}). Enable copilot/autopilot and clear emergency stop.",
                    feature, mode_name, emergency_stop
                ),
                command: Some("error_action_denied".to_string()),
            })
        };

        let run_app = |name: &str, alias: &str| -> Json<ChatResponse> {
            if !automation_allowed {
                return blocked_automation(alias);
            }
            match crate::windows::actions::launch_app(name) {
                Ok(_) => Json(ChatResponse {
                    response: format!("Launched {}.", alias),
                    command: Some("open_app".to_string()),
                }),
                Err(e) => Json(ChatResponse {
                    response: format!("Failed to launch {}: {}", alias, e),
                    command: None,
                }),
            }
        };

        if is_screen_summary_request(&lower) {
            if !automation_allowed {
                return blocked_automation("screen_summary");
            }
            return match deliver_screen_summary(&state, &message).await {
                Ok((target, final_text)) => {
                    let preview: String = final_text.chars().take(280).collect();
                    Json(ChatResponse {
                        response: format!(
                            "Screen summary delivered ({target}).\nPreview:\n{}{}",
                            preview,
                            if final_text.chars().count() > 280 { "\n..." } else { "" }
                        ),
                        command: Some("screen_summary_deliver".to_string()),
                    })
                }
                Err(e) => Json(ChatResponse {
                    response: format!("Screen summary automation failed: {}", e),
                    command: Some("error_exec_failed".to_string()),
                }),
            };
        }

        if lower.contains("email workflow")
            || lower.contains("mail workflow")
            || lower.contains(ko_mail_workflow)
            || lower.contains(ko_mail_work)
        {
            if !automation_allowed {
                return blocked_automation("email_ops_workflow");
            }

            let template_mode = lower.contains("template") || lower.contains(ko_template);
            let configured_query = std::env::var("EMAIL_WORKFLOW_GMAIL_QUERY")
                .unwrap_or_else(|_| "is:unread newer_than:7d category:primary".to_string());
            let hints = derive_email_workflow_hints(&configured_query);
            let gmail_query = hints.derived_query.clone();
            let emails = if template_mode {
                vec![
                    ("Q1 budget review request".to_string(), "finance@company.com".to_string(), "Please review budget by EOD.".to_string()),
                    ("Client escalation: delivery timeline".to_string(), "sales@company.com".to_string(), "Customer requested urgent timeline update.".to_string()),
                    ("Weekly project sync agenda".to_string(), "pm@company.com".to_string(), "Attached agenda for tomorrow morning.".to_string()),
                ]
            } else {
                match integrations::gmail::GmailClient::new().await {
                    Ok(client) => match client.list_messages_enriched(12, Some(&gmail_query)).await {
                        Ok(items) => items
                            .into_iter()
                            .map(|m| (m.subject, m.from, m.snippet))
                            .collect::<Vec<_>>(),
                        Err(e) => {
                            return Json(ChatResponse {
                                response: format!(
                                    "Email fetch failed: {}. You can still test with 'email workflow template run'.",
                                    e
                                ),
                                command: Some("error_missing_credentials".to_string()),
                            });
                        }
                    },
                    Err(e) => {
                        return Json(ChatResponse {
                            response: format!(
                                "Gmail connection failed: {}. You can still test with 'email workflow template run'.",
                                e
                            ),
                            command: Some("error_missing_credentials".to_string()),
                        });
                    }
                }
            };

            let mut ranked_emails: Vec<(i32, (String, String, String))> = emails
                .into_iter()
                .map(|(subject, from, snippet)| {
                    let score = score_email_priority(&subject, &snippet, &hints);
                    (score, (subject, from, snippet))
                })
                .collect();
            ranked_emails.sort_by(|a, b| b.0.cmp(&a.0));
            let emails: Vec<(String, String, String)> =
                ranked_emails.into_iter().map(|(_, item)| item).collect();

            if emails.is_empty() {
                return Json(ChatResponse {
                    response: "No recent emails found. Workflow artifacts were not created.".to_string(),
                    command: Some("email_workflow_prepare".to_string()),
                });
            }

            let mut summary = String::from("# Email Ops Summary\n\n");
            summary.push_str("Generated automatically from recent inbox messages.\n\n");
            summary.push_str(&format!("- Query: `{}`\n\n", gmail_query));
            if !hints.reasons.is_empty() {
                summary.push_str("## Why This Query\n");
                for reason in &hints.reasons {
                    summary.push_str(&format!("- {}\n", reason));
                }
                summary.push('\n');
            }
            summary.push_str("## Top Messages\n");
            for (idx, (subject, from, snippet)) in emails.iter().take(10).enumerate() {
                let short_snippet: String = snippet.chars().take(180).collect();
                let priority = score_email_priority(subject, snippet, &hints);
                summary.push_str(&format!(
                    "{}. {} ({})\n   - {}\n",
                    idx + 1,
                    subject,
                    from,
                    short_snippet
                ));
                summary.push_str(&format!("   - priority_score: {}\n", priority));
            }
            summary.push_str("\n## Suggested Next Steps\n");
            summary.push_str("- Review high-priority items and draft replies.\n");
            summary.push_str("- Update task tracker from tasks.csv.\n");
            summary.push_str("- Push final report to Notion/Word/Telegram if needed.\n");

            let execute_mode = lower.contains(" run")
                || lower.ends_with("run")
                || lower.contains(ko_execute);

            let task_rows: Vec<(String, String)> = emails
                .iter()
                .map(|(subject, from, _)| (subject.clone(), from.clone()))
                .collect();
            return match write_email_ops_artifacts(&summary, &task_rows) {
                Ok((summary_path, tasks_path)) => Json(ChatResponse {
                    response: {
                        if execute_mode {
                            let mut out = format!(
                                "Email workflow executed.\n- Summary: {}\n- Tasks CSV: {}",
                                summary_path.display(),
                                tasks_path.display()
                            );

                            let db_id = std::env::var("NOTION_DATABASE_ID").ok();
                            if let (Some(db), Ok(client)) = (db_id, integrations::notion::NotionClient::from_env()) {
                                if !is_valid_notion_database_id(&db) {
                                    out.push_str("\n- Notion skipped (invalid NOTION_DATABASE_ID format)");
                                } else {
                                    let title = format!("Email Ops {}", chrono::Utc::now().format("%Y-%m-%d %H:%M"));
                                    let page_content: String = summary.chars().take(1800).collect();
                                    match client.create_page(&db, &title, &page_content).await {
                                        Ok(page_id) => out.push_str(&format!("\n- Notion page created: {}", page_id)),
                                        Err(e) => out.push_str(&format!("\n- Notion create failed: {}", e)),
                                    }
                                }
                            } else {
                                out.push_str("\n- Notion skipped (NOTION_API_KEY/NOTION_DATABASE_ID missing)");
                            }

                            if let Ok(bot) = integrations::telegram::TelegramBot::from_env() {
                                let notion_line = if out.contains("Notion page created: ") {
                                    let page_id = out
                                        .split("Notion page created: ")
                                        .nth(1)
                                        .and_then(|s| s.lines().next())
                                        .unwrap_or("")
                                        .trim()
                                        .to_string();
                                    if page_id.is_empty() {
                                        "N/A".to_string()
                                    } else {
                                        format!("https://www.notion.so/{}", page_id.replace('-', ""))
                                    }
                                } else {
                                    "N/A".to_string()
                                };
                                let msg = format!(
                                    "Email workflow complete\n- Items: {}\n- Summary: {}\n- Notion: {}",
                                    emails.len(),
                                    summary_path.display(),
                                    notion_line
                                );
                                let _ = bot.send(&msg).await;
                            }

                            let _ = open_path_windows(&summary_path);
                            let _ = open_path_windows(&tasks_path);
                            if let Some(folder) = summary_path.parent() {
                                let _ = open_path_windows(folder);
                            }

                            out.push_str("\n- Local files opened. Continue in Excel/Word/Notion as needed.");
                            out
                        } else {
                            format!(
                                "Email workflow prepared.\n- Summary: {}\n- Tasks CSV: {}\nNext: run 'email workflow run' to execute open/export actions.",
                                summary_path.display(),
                                tasks_path.display()
                            )
                        }
                    },
                    command: Some(if execute_mode { "email_workflow_execute".to_string() } else { "email_workflow_prepare".to_string() }),
                }),
                Err(e) => Json(ChatResponse {
                    response: format!("Email workflow artifact write failed: {}", e),
                    command: Some("error_exec_failed".to_string()),
                }),
            };
        }

        if lower.contains("notepad") || lower.contains(ko_notepad) { return run_app("notepad", "Notepad"); }
        if lower.contains("calculator") || lower == "calc" || lower.contains(ko_calc) { return run_app("calc", "Calculator"); }
        if lower.contains("task manager") || lower == "taskmgr" { return run_app("taskmgr", "Task Manager"); }
        if lower.contains("windows settings") || lower == "settings" { return run_app("ms-settings:", "Windows Settings"); }
        if lower.contains("open chrome") || lower == "chrome" { return run_app("chrome", "Chrome"); }
        if lower.contains("open edge") || lower == "edge" { return run_app("msedge", "Edge"); }

        if lower.contains("explorer") || lower.contains("file explorer") || lower.contains(ko_explorer) {
            if !automation_allowed {
                return blocked_automation("explorer");
            }
            let status = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", "Start-Process explorer.exe"])
                .status();
            return match status {
                Ok(s) if s.success() => Json(ChatResponse {
                    response: "Opened File Explorer.".to_string(),
                    command: Some("open_explorer".to_string()),
                }),
                Ok(_) | Err(_) => Json(ChatResponse {
                    response: "Failed to open File Explorer.".to_string(),
                    command: None,
                }),
            };
        }

        if lower.contains("list files") {
            let target = if lower.contains("downloads") {
                dirs::download_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
            } else if lower.contains("desktop") {
                dirs::desktop_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
            } else {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            };
            return Json(ChatResponse {
                response: list_files_human(&target, 30),
                command: Some("list_files".to_string()),
            });
        }

        if let Some(rest) = lower.strip_prefix("open url ") {
            if !automation_allowed {
                return blocked_automation("open_url");
            }
            let url = rest.trim();
            if url.starts_with("http://") || url.starts_with("https://") {
                return match open_url_windows(url) {
                    Ok(_) => Json(ChatResponse {
                        response: format!("Opened URL: {}", url),
                        command: Some("open_url".to_string()),
                    }),
                    Err(e) => Json(ChatResponse {
                        response: format!("Failed to open URL: {}", e),
                        command: None,
                    }),
                };
            }
        }

        if let Some(rest) = lower.strip_prefix("search ") {
            if !automation_allowed {
                return blocked_automation("web_search");
            }
            let query = rest.trim().replace(' ', "+");
            if !query.is_empty() {
                let url = format!("https://www.google.com/search?q={}", query);
                return match open_url_windows(&url) {
                    Ok(_) => Json(ChatResponse {
                        response: format!("Searching web for '{}'.", rest.trim()),
                        command: Some("web_search".to_string()),
                    }),
                    Err(e) => Json(ChatResponse {
                        response: format!("Failed to open browser: {}", e),
                        command: None,
                    }),
                };
            }
        }
        if lower.contains(ko_search) && lower.contains(ko_open) {
            if !automation_allowed {
                return blocked_automation("web_search");
            }
            let query = lower.replace(ko_search, "").replace(ko_open, "").trim().replace(' ', "+");
            if !query.is_empty() {
                let url = format!("https://www.google.com/search?q={}", query);
                return match open_url_windows(&url) {
                    Ok(_) => Json(ChatResponse {
                        response: "Searching web.".to_string(),
                        command: Some("web_search".to_string()),
                    }),
                    Err(e) => Json(ChatResponse {
                        response: format!("Failed to open browser: {}", e),
                        command: None,
                    }),
                };
            }
        }

        let ko_run = "\u{C2E4}\u{D589} ";
        let raw_cmd = if let Some(c) = message.strip_prefix(ko_run) {
            Some(c.trim())
        } else if let Some(c) = message.strip_prefix("run ") {
            Some(c.trim())
        } else if let Some(c) = message.strip_prefix("cmd ") {
            Some(c.trim())
        } else {
            None
        };

        if let Some(cmd) = raw_cmd {
            if !automation_allowed {
                return blocked_automation("run_command");
            }

            let deny_patterns = [
                "format ",
                "diskpart",
                "bcdedit",
                "cipher /w",
                "remove-item -recurse -force c:\\",
                "del /s /q c:\\",
                "rd /s /q c:\\",
                "shutdown /s",
                "shutdown -s",
                "reg delete hk",
            ];
            let cmd_l = cmd.to_lowercase();
            if deny_patterns.iter().any(|p| cmd_l.contains(p)) {
                return Json(ChatResponse {
                    response: "Blocked potentially destructive command.".to_string(),
                    command: None,
                });
            }

            let cfg = crate::bash_executor::BashExecConfig {
                timeout_ms: 20_000,
                working_dir: None,
                env_vars: std::collections::HashMap::new(),
                background: false,
                approval_required: true,
            };

            return match crate::bash_executor::execute_bash(cmd, &cfg) {
                Ok(res) => {
                    let mut out = String::new();
                    if !res.stdout.trim().is_empty() {
                        out.push_str("STDOUT:\n");
                        out.push_str(res.stdout.trim());
                        out.push('\n');
                    }
                    if !res.stderr.trim().is_empty() {
                        out.push_str("STDERR:\n");
                        out.push_str(res.stderr.trim());
                        out.push('\n');
                    }
                    if out.is_empty() {
                        out = "Command executed with no output.".to_string();
                    }
                    Json(ChatResponse {
                        response: format!(
                            "[exit:{} | {}ms]\n{}",
                            res.exit_code,
                            res.duration_ms,
                            out.chars().take(2000).collect::<String>()
                        ),
                        command: Some("run_command".to_string()),
                    })
                }
                Err(e) => Json(ChatResponse {
                    response: format!("Command execution failed: {}", e),
                    command: None,
                }),
            };
        }
    }

    if state.llm_client.is_some() {
        match generate_free_chat_reply(&state, &message).await {
            Ok(response) if !response.is_empty() => {
                if let Err(e) = db::insert_chat_message("assistant", &response) {
                    eprintln!("Failed to save assistant chat: {}", e);
                }
                return Json(ChatResponse {
                    response,
                    command: Some("chat_reply".to_string()),
                });
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Free chat generation failed: {}", e);
                let lower = e.to_lowercase();
                if lower.contains("rate limit") || lower.contains("rate_limit_exceeded") {
                    return Json(ChatResponse {
                        response: "LLM rate limit reached. Please retry in a few seconds.".to_string(),
                        command: Some("error_rate_limited".to_string()),
                    });
                }
            }
        }
    }

    Json(ChatResponse {
        response: "I could not determine the request. Try: system status, calendar, email, open notepad, list files, search <query>.".to_string(),
        command: None,
    })
}


