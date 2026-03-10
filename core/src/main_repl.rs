use crate::main_commands::{
    handle_ai_digest_command, handle_analyze_patterns_command, handle_approve_command,
    handle_approve_test_command, handle_build_workflow_command, handle_calendar_command,
    handle_control_command, handle_gmail_command, handle_ingest_handoff_command,
    handle_ingest_mock_workflow_command, handle_notion_command, handle_quality_command,
    handle_recommendations_command, handle_reject_command, handle_status_command,
    handle_surf_repl_command, handle_telegram_command, handle_telegram_listener_command,
};
use chrono::Utc;
use local_os_agent::schema::{AgentAction, EventEnvelope};
use local_os_agent::{
    applescript, bash_executor, db, env_flag, llm_gateway, macos, monitor, orchestrator, policy,
    security,
};
use serde_json::json;
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt};
use tracing::{error, info, warn};
use uuid::Uuid;

fn print_help() {
    println!("Commands:");
    println!("  snap [scope]          - Take UI snapshot");
    println!("  click <id>            - Click element by ID");
    println!("  type <text>           - Type text");
    println!("  unlock                - Unlock Write Policy");
    println!("  status                - Show system status");
    println!("  recommendations [N]   - List pending workflow recommendations");
    println!("  approve <id>          - Approve and create n8n workflow");
    println!("  approve_test <id>     - Test-only assumed approval then create workflow");
    println!("  reject <id>           - Reject recommendation");
    println!("  ingest_mock_workflow  - Ingest mock workflow as pending recommendation");
    println!("  ingest_handoff [cfg]  - Consume collector pending handoff into recommendation");
    println!("  analyze_patterns      - Detect behavior patterns and generate recommendations");
    println!("  quality               - Show workflow quality metrics");
    println!("  ai_digest [msg]       - Trigger n8n News Digest via program webhook");
    println!("  news_digest [msg]     - Alias of ai_digest (natural-language topic)");
    println!("  telegram <msg>        - Send Telegram message");
    println!("  telegram_listen       - Start Telegram natural-language listener");
    println!("  notion <title>|<body> - Create Notion page");
    println!("  gmail list [N]        - List recent N emails");
    println!("  gmail read <id>       - Read email by ID");
    println!("  gmail send <to>|<subj>|<body> - Send email");
    println!("  gmail digest [N]      - Summarize N emails -> Notion -> Telegram");
    println!("  calendar today        - Today's events");
    println!("  calendar week         - This week's events");
    println!("  calendar add <title>|<start>|<end> - Add event");
    println!("  exit                  - Quit");
}

async fn enter_headless_mode() -> ! {
    println!("📡 Running in headless mode (API only)...");
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}

pub(crate) async fn run_repl_loop(
    llm_client: Option<Arc<dyn llm_gateway::LLMClient>>,
    log_tx: tokio::sync::mpsc::Sender<String>,
) -> anyhow::Result<()> {
    let mut policy = policy::PolicyEngine::new();
    let mut res_mon = monitor::ResourceMonitor::new();
    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin);
    let mut buffer = String::new();

    loop {
        buffer.clear();
        print!("> ");
        if let Err(e) = io::stdout().flush().await {
            eprintln!("⚠️ Flush failed: {}", e);
        }

        if reader.read_line(&mut buffer).await? == 0 {
            if env_flag("STEER_EXIT_ON_EOF") {
                println!("👋 EOF received. Exiting by STEER_EXIT_ON_EOF=1.");
                break;
            }

            enter_headless_mode().await;
        }

        let input = buffer.trim();
        if input.is_empty() {
            continue;
        }

        let parts: Vec<&str> = input.split_whitespace().collect();
        match parts[0] {
            "help" => {
                print_help();
            }
            "exit" | "quit" => break,
            "unlock" => {
                policy.unlock();
                println!("[Policy] Write Lock UNLOCKED.");
            }
            "lock" => {
                policy.lock();
                println!("[Policy] Write Lock LOCKED.");
            }
            "snap" => {
                let scope = if parts.len() > 1 {
                    Some(parts[1].to_string())
                } else {
                    None
                };
                println!("[Platform] Snapshotting...");
                let tree = local_os_agent::platform::current_platform().ui_snapshot(scope)?;
                println!("📄 Snapshot:\n{}", serde_json::to_string_pretty(&tree)?);
            }
            "type" => {
                if parts.len() < 2 {
                    println!("Usage: type <text>");
                    continue;
                }
                let text = parts[1..].join(" ");
                match policy.check(&AgentAction::UiType { text: text.clone() }) {
                    Ok(_) => {
                        println!("✅ Policy Passed");
                        #[cfg(target_os = "macos")]
                        if let Err(e) = macos::actions::type_text(&text) {
                            println!("❌ Type failed: {}", e);
                        }
                    }
                    Err(e) => println!("⛔️ Policy Blocked: {}", e),
                }
            }
            "click" => {
                if parts.len() < 2 {
                    println!("Usage: click <id>");
                    continue;
                }
                let id = parts[1];
                match policy.check(&AgentAction::UiClick {
                    element_id: id.to_string(),
                    double_click: false,
                }) {
                    Ok(_) => {
                        println!("✅ Policy Passed");
                        #[cfg(target_os = "macos")]
                        if let Err(e) = macos::actions::click_element(id) {
                            println!("❌ Click failed: {}", e);
                        }
                    }
                    Err(e) => println!("⛔️ Policy Blocked: {}", e),
                }
            }
            "exec" => {
                if parts.len() < 2 {
                    println!("Usage: exec <command>");
                    continue;
                }
                let cmd = parts[1..].join(" ");

                match security::CommandClassifier::classify(&cmd) {
                    security::SafetyLevel::Critical => {
                        println!("⛔️ CRITICAL WARNING: This command is flagged as DANGEROUS.");
                        println!("   Command: {}", cmd);
                        println!("   To execute, type 'CONFIRM':");

                        buffer.clear();
                        if reader.read_line(&mut buffer).await? == 0 {
                            break;
                        }
                        if buffer.trim() != "CONFIRM" {
                            println!("❌ Aborted.");
                            continue;
                        }
                    }
                    security::SafetyLevel::Warning => {
                        println!("⚠️  WARNING: This command may modify your system.");
                        println!("   Command: {}", cmd);
                        println!("   Execute? (y/n):");

                        buffer.clear();
                        if reader.read_line(&mut buffer).await? == 0 {
                            break;
                        }
                        if buffer.trim().to_lowercase() != "y" {
                            println!("❌ Aborted.");
                            continue;
                        }
                    }
                    security::SafetyLevel::Safe => {}
                }

                let cwd = std::env::current_dir()
                    .ok()
                    .map(|p| p.to_string_lossy().to_string());
                let action = AgentAction::ShellExecution {
                    command: cmd.clone(),
                };
                match policy.check_with_context(&action, cwd.as_deref()) {
                    Ok(_) => {
                        println!("⚙️  Executing: '{}'", cmd);
                        match bash_executor::exec(&cmd) {
                            Ok(out) => println!("Output:\n{}", out),
                            Err(e) => println!("❌ Exec failed: {}", e),
                        }
                    }
                    Err(e) => {
                        if let Ok(Some(_approval)) =
                            db::find_valid_exec_approval(&cmd, cwd.as_deref())
                        {
                            println!("✅ Approved command found. Executing: '{}'", cmd);
                            match bash_executor::exec(&cmd) {
                                Ok(out) => println!("Output:\n{}", out),
                                Err(e) => println!("❌ Exec failed: {}", e),
                            }
                        } else {
                            let approval =
                                db::create_exec_approval(&cmd, cwd.as_deref(), 3600).ok();
                            if let Some(approval) = approval {
                                println!("⛔️ Policy Blocked: {}", e);
                                println!("📝 Exec approval requested: {}", approval.id);
                                println!(
                                    "   Approve once: POST /api/exec-approvals/{}/approve",
                                    approval.id
                                );
                                println!("   Approve always: POST /api/exec-approvals/{}/approve ({{\"decision\":\"allow-always\"}})", approval.id);
                            } else {
                                println!("⛔️ Policy Blocked: {}", e);
                            }
                        }
                    }
                }
            }
            "open" => {
                if parts.len() < 2 {
                    println!("Usage: open <url>");
                    continue;
                }
                let url = parts[1];
                println!("🌐 Opening URL: {}", url);
                if let Err(e) = applescript::open_url(url).map(|_| ()) {
                    println!("❌ Open failed: {}", e);
                }
            }
            "fake_log" => {
                #[cfg(target_os = "macos")]
                {
                    let event = EventEnvelope {
                        schema_version: "1.0".to_string(),
                        event_id: Uuid::new_v4().to_string(),
                        ts: Utc::now().to_rfc3339(),
                        source: "debug".to_string(),
                        app: "FakeApp".to_string(),
                        event_type: "simulated".to_string(),
                        priority: "P2".to_string(),
                        resource: None,
                        payload: json!({"note": "simulated"}),
                        privacy: None,
                        pid: None,
                        window_id: None,
                        window_title: None,
                        browser_url: None,
                        raw: None,
                    };
                    if let Ok(log) = serde_json::to_string(&event) {
                        let _ = log_tx.send(log).await;
                    }
                    println!("✅ Simulated Log Sent");
                }
            }
            "routine" => {
                if let Some(brain) = &llm_client {
                    println!("🧠 Analyzing daily routine (last 24h)...");
                    match db::get_recent_events(24) {
                        Ok(logs) => {
                            if logs.is_empty() {
                                println!("   (No events found in DB to analyze)");
                            } else {
                                println!("   Found {} events. Asking LLM...", logs.len());
                                match brain.analyze_routine(&logs).await {
                                    Ok(summary) => {
                                        println!("\n📊 Routine Analysis:\n{}", summary);
                                    }
                                    Err(e) => println!("❌ Analysis failed: {}", e),
                                }
                            }
                        }
                        Err(e) => println!("❌ DB Query failed: {}", e),
                    }
                } else {
                    println!("⚠️  LLM Client not available.");
                }
            }
            "recommend" => {
                if let Some(brain) = &llm_client {
                    println!("🤖 Generating automation recommendation...");
                    match db::get_recent_events(24) {
                        Ok(logs) => {
                            if logs.is_empty() {
                                println!("   (No events found in DB)");
                            } else {
                                match brain.recommend_automation(&logs).await {
                                    Ok(script) => {
                                        println!("\n✨ Recommendation:\n{}", script);
                                        println!(
                                            "\n💡 Tip: Save code to a file and run with 'exec <file>'"
                                        );
                                    }
                                    Err(e) => println!("❌ Recommendation failed: {}", e),
                                }
                            }
                        }
                        Err(e) => println!("❌ DB Query failed: {}", e),
                    }
                } else {
                    println!("⚠️  LLM Client not available.");
                }
            }
            "analyze_patterns" | "detect" => {
                handle_analyze_patterns_command(llm_client.as_ref()).await;
            }
            "quality" | "metrics" => {
                handle_quality_command();
            }
            "status" => {
                handle_status_command(&mut res_mon);
            }
            "recommendations" | "recs" => {
                handle_recommendations_command(&parts);
            }
            "approve" => {
                handle_approve_command(&parts, llm_client.clone()).await;
            }
            "approve_test" => {
                handle_approve_test_command(&parts, llm_client.clone()).await;
            }
            "ingest_mock_workflow" => {
                handle_ingest_mock_workflow_command(llm_client.clone()).await;
            }
            "ingest_handoff" => {
                handle_ingest_handoff_command(&parts);
            }
            "reject" => {
                handle_reject_command(&parts);
            }
            "control" => {
                handle_control_command(&parts);
            }
            "build_workflow" => {
                handle_build_workflow_command(&parts);
            }
            "ai_digest" | "ai-digest" | "news_digest" | "news-digest" | "digest" => {
                handle_ai_digest_command(&parts).await;
            }
            "telegram" => {
                handle_telegram_command(&parts).await;
            }
            "telegram_listen" | "telegram-listen" => {
                handle_telegram_listener_command(llm_client.clone());
            }
            "notion" => {
                handle_notion_command(&parts).await;
            }
            "gmail" => {
                handle_gmail_command(&parts, llm_client.as_deref()).await;
            }
            "calendar" => {
                handle_calendar_command(&parts).await;
            }
            "surf" => {
                handle_surf_repl_command(&parts, llm_client.as_ref()).await;
            }
            _ => {
                if let Ok(orch) = orchestrator::Orchestrator::new().await {
                    info!("🤖 Super Agent: Processing '{}'...", input);
                    match orch.handle_request(input).await {
                        Ok(resp) => {
                            println!("{}", resp);
                            info!("{}", resp);
                        }
                        Err(e) => error!("❌ Super Agent Error: {}", e),
                    }
                } else {
                    warn!("⚠️  Orchestrator could not initialization.");
                }
            }
        }
    }

    Ok(())
}
