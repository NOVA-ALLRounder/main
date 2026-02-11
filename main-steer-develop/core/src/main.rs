use local_os_agent::{
    analyzer, api_server, applescript, bash_executor, db, dependency_check, feedback_collector,
    integrations, llm_gateway, mcp_client, monitor, n8n_api, orchestrator, pattern_detector,
    policy, recommendation, scheduler, security, visual_driver,
};

use local_os_agent::env_flag;
use local_os_agent::singleton_lock;

#[cfg(target_os = "macos")]
use local_os_agent::macos;
#[cfg(target_os = "windows")]
use local_os_agent::windows;

use chrono::Utc;
use local_os_agent::schema::{AgentAction, EventEnvelope};
use serde_json::json;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt};
use tracing::{error, info, warn};
use uuid::Uuid;

fn mock_workflow_json(name: &str) -> serde_json::Value {
    json!({
        "name": name,
        "nodes": [{
            "id": "manual-trigger-1",
            "name": "Manual Trigger",
            "type": "n8n-nodes-base.manualTrigger",
            "typeVersion": 1,
            "position": [240, 300],
            "parameters": {}
        }],
        "connections": {},
        "settings": {},
        "meta": {
            "source": "steer-approve-assumed-test"
        }
    })
}

fn recommendation_fingerprint(title: &str, trigger: &str) -> String {
    format!(
        "{}::{}",
        title.trim().to_lowercase(),
        trigger.trim().to_lowercase()
    )
}

fn find_recommendation_id_by_fingerprint(target: &str) -> anyhow::Result<Option<i64>> {
    let rows = db::get_recommendations_with_filter(Some("all"))?;
    for rec in rows {
        let fp = recommendation_fingerprint(&rec.title, &rec.trigger);
        if fp == target {
            return Ok(Some(rec.id));
        }
    }
    Ok(None)
}

fn load_mock_workflow_proposal() -> anyhow::Result<recommendation::AutomationProposal> {
    let path = std::env::var("STEER_WORKFLOW_MOCK_FILE")
        .unwrap_or_else(|_| "core/mock/workflow_received_mock.json".to_string());
    let raw = std::fs::read_to_string(&path)?;
    let proposal = serde_json::from_str::<recommendation::AutomationProposal>(&raw)?;
    if proposal.n8n_prompt.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "mock workflow has empty n8n_prompt: {}",
            path
        ));
    }
    Ok(proposal)
}

fn maybe_assume_approved_for_test(id: i64) -> anyhow::Result<()> {
    if !env_flag("STEER_TEST_ASSUME_APPROVED") {
        return Err(anyhow::anyhow!(
            "STEER_TEST_ASSUME_APPROVED=1 is required for approve_test path"
        ));
    }

    let rec = db::get_recommendation(id)?
        .ok_or_else(|| anyhow::anyhow!("recommendation {} not found", id))?;
    if rec.status.eq_ignore_ascii_case("rejected") {
        return Err(anyhow::anyhow!(
            "recommendation {} is rejected and cannot be assumed approved",
            id
        ));
    }
    if !rec.status.eq_ignore_ascii_case("approved") {
        db::update_recommendation_status(id, "approved")?;
        println!("?㎦ [TEST] Assumed approval for recommendation {}.", id);
    }
    Ok(())
}

async fn execute_approved_recommendation(
    id: i64,
    llm_client: Option<std::sync::Arc<dyn llm_gateway::LLMClient>>,
) -> anyhow::Result<String> {
    let rec = db::get_recommendation(id)?
        .ok_or_else(|| anyhow::anyhow!("recommendation {} not found", id))?;
    if !rec.status.eq_ignore_ascii_case("approved") {
        return Err(anyhow::anyhow!(
            "recommendation {} is '{}' (approval required before creation)",
            id,
            rec.status
        ));
    }

    let use_mock_workflow_json = env_flag("STEER_TEST_ASSUME_APPROVED") && env_flag("STEER_N8N_MOCK");
    let workflow_json_str = if use_mock_workflow_json {
        serde_json::to_string(&mock_workflow_json(&rec.title))?
    } else {
        let brain = llm_client.ok_or_else(|| anyhow::anyhow!("LLM Client not available"))?;
        brain
            .build_n8n_workflow(&rec.n8n_prompt)
            .await
            .map_err(|e| anyhow::anyhow!("workflow generation failed: {}", e))?
    };

    let workflow_val = serde_json::from_str::<serde_json::Value>(&workflow_json_str).map_err(|e| {
        anyhow::anyhow!(
            "generated workflow JSON is invalid for recommendation {}: {}",
            id,
            e
        )
    })?;

    let n8n_url = std::env::var("N8N_API_URL").unwrap_or_else(|_| "http://localhost:5678".to_string());
    let n8n_key = std::env::var("N8N_API_KEY").unwrap_or_default();
    let n8n = n8n_api::N8nApi::new(&format!("{}/api/v1", n8n_url), &n8n_key);
    match n8n.create_workflow(&rec.title, &workflow_val, true).await {
        Ok(workflow_id) => {
            db::mark_recommendation_approved(id, &workflow_id, &workflow_json_str)?;
            Ok(workflow_id)
        }
        Err(e) => {
            let _ = db::mark_recommendation_failed(id, &e.to_string());
            Err(anyhow::anyhow!("workflow creation failed: {}", e))
        }
    }
}

async fn ingest_mock_workflow_recommendation(
    llm_client: Option<std::sync::Arc<dyn llm_gateway::LLMClient>>,
) -> anyhow::Result<()> {
    let proposal = load_mock_workflow_proposal()?;
    let fp = proposal.fingerprint();
    let inserted = db::insert_recommendation(&proposal)?;
    let rec_id = find_recommendation_id_by_fingerprint(&fp)?
        .ok_or_else(|| anyhow::anyhow!("failed to resolve recommendation id after insert"))?;

    if inserted {
        println!(
            "?뱿 Mock workflow ingested as pending recommendation [{}] {}",
            rec_id, proposal.title
        );
    } else {
        println!(
            "?뱿 Mock workflow already exists; reusing recommendation [{}] {}",
            rec_id, proposal.title
        );
    }

    if env_flag("STEER_TEST_ASSUME_APPROVED") {
        maybe_assume_approved_for_test(rec_id)?;
        match execute_approved_recommendation(rec_id, llm_client).await {
            Ok(workflow_id) => println!(
                "??[TEST] approve-assumed pipeline completed. Workflow ID: {}",
                workflow_id
            ),
            Err(e) => println!("??[TEST] approve-assumed pipeline failed: {}", e),
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    local_os_agent::main_startup::print_env_diagnostics();

    // Initialize Tracing
    tracing_subscriber::fmt::init();

    local_os_agent::main_startup::install_panic_hook();

    // Fast-path: keep `rewrite` output clean (no startup banners/log noise).
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "rewrite" {
        let message = args[2..].join(" ");
        let refined = match llm_gateway::OpenAILLMClient::new() {
            Ok(c) => {
                let llm = std::sync::Arc::new(c) as std::sync::Arc<dyn llm_gateway::LLMClient>;
                if let Some(bot) = local_os_agent::telegram::TelegramBot::from_env(llm, None) {
                    bot.improve_message(&message).await
                } else {
                    message.clone()
                }
            }
            Err(_) => message.clone(),
        };
        println!("{}", refined);
        return Ok(());
    }

    let _lock = match singleton_lock::acquire_lock() {
        Ok(guard) => guard,
        Err(err) => {
            eprintln!("?뷂툘 {}", err);
            return Ok(());
        }
    };

    println!("?쨼 Local OS Agent (Rust Native Mode) Started!");
    local_os_agent::main_startup::print_ui_permission_hint();
    println!("--------------------------------------------------");

    // 0. System Health Check
    let health = dependency_check::SystemHealth::check_all();
    health.print_report();

    println!("Type 'help' for commands. (Needs Accessibility Permissions)");
    println!("--------------------------------------------------");

    // 0. Init Check
    if let Err(e) = db::init() {
        eprintln!("Failed to init DB: {}", e);
    }

    // 1. Init LLM
    let llm_client: Option<std::sync::Arc<dyn llm_gateway::LLMClient>> =
        match llm_gateway::OpenAILLMClient::new() {
            Ok(c) => Some(std::sync::Arc::new(c)),
            Err(e) => {
                warn!("?좑툘 Failed to init LLM Gateway: {}", e);
                None
            }
        };

    if llm_client.is_some() {
        println!("✅ LLM Client Initialized Successfully.");
    } else {
        println!("⚠️ LLM Client FAILED to initialize. Check OPENAI_API_KEY in .env");
    }

    // Fast-path CLI commands: run before background services (API/EventTap/Watchers)
    // so they are not blocked by API port conflicts.
    if args.len() >= 3 && args[1] == "notify" {
        let message = args[2..].join(" ");
        println!("?뵒 Sending Smart Notification: {}", message);
        if let Some(llm) = llm_client.clone() {
            if let Some(bot) = local_os_agent::telegram::TelegramBot::from_env(llm, None) {
                let chat_id_str =
                    std::env::var("TELEGRAM_CHAT_ID").unwrap_or_else(|_| "0".to_string());
                if let Ok(chat_id) = chat_id_str.parse::<i64>() {
                    if let Err(e) = bot.send_smart_notification(chat_id, &message).await {
                        eprintln!("??Failed to send notification: {}", e);
                    } else {
                        println!("??Notification sent successfully (Smart Mode).");
                    }
                } else {
                    eprintln!("??TELEGRAM_CHAT_ID not set or invalid.");
                }
            } else {
                eprintln!("??Telegram Bot configuration missing (TELEGRAM_BOT_TOKEN).");
            }
        } else {
            eprintln!("??LLM Client not available for smart notification.");
        }
        return Ok(());
    }

    if args.len() >= 3 && args[1] == "surf" {
        let goal = args[2..].join(" ");
        println!("?렞 [CLI] Direct surf mode: {}", goal);
        if let Some(llm) = llm_client.clone() {
            let mut cli_policy = policy::PolicyEngine::new();
            cli_policy.unlock();
            let planner = local_os_agent::controller::planner::Planner::new(llm, None);
            match planner.run_goal(&goal, None).await {
                Ok(_) => println!("??Surf completed successfully!"),
                Err(e) => println!("??Surf failed: {}", e),
            }
        } else {
            println!("??LLM not available for surf mode");
        }
        return Ok(());
    }

    // 2. Start Scheduler (Brain)
    if let Some(llm) = &llm_client {
        let scheduler = scheduler::Scheduler::new(llm.clone());
        scheduler.start();
        info!("?쭬 Brain Routine Scheduler Active.");
    }

    // 2.5 Init MCP
    if let Err(e) = mcp_client::init_mcp() {
        warn!("?좑툘 Failed to init MCP: {}", e);
    } else {
        info!("?뵆 MCP System Initialized.");
    }

    // 1. Start Native Event Tap (replaces IPC Adapter)
    // [Paranoid Audit] Increased capacity to 1000 to prevent dropping mouse bursts
    let (log_tx, mut log_rx) = tokio::sync::mpsc::channel::<String>(1000);

    #[cfg(target_os = "macos")]
    {
        if env_flag("STEER_DISABLE_EVENT_TAP") {
            info!("?좑툘  Event Tap disabled via STEER_DISABLE_EVENT_TAP.");
        } else if let Err(e) = macos::events::start_event_tap(log_tx.clone()) {
            error!("??Failed to start Event Tap: {}", e);
        }
    }
    #[cfg(target_os = "windows")]
    {
        if env_flag("STEER_DISABLE_EVENT_TAP") {
            info!("Event Tap disabled via STEER_DISABLE_EVENT_TAP.");
        } else if let Err(e) = windows::events::start_event_tap(log_tx.clone()) {
            error!("Failed to start Windows event tap: {}", e);
        }
    }

    // 2. Start "Shadow Analyzer" (Decoupled Module)
    // CRITICAL FIX: Always consume log_rx, even without LLM
    if let Some(c) = llm_client.clone() {
        let disable_analyzer = std::env::var("STEER_DISABLE_ANALYZER")
            .ok()
            .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        if disable_analyzer {
            println!("Analyzer disabled via STEER_DISABLE_ANALYZER. Logging events only.");
            tokio::spawn(async move {
                while let Some(log_json) = log_rx.recv().await {
                    if let Err(e) = db::insert_event(&log_json) {
                        eprintln!("DB insert error: {}", e);
                    }
                }
            });
        } else {
            analyzer::spawn(log_rx, c);
        }
    } else {
        // Fallback: Just save events to DB without LLM analysis
        tokio::spawn(async move {
            while let Some(log_json) = log_rx.recv().await {
                if let Err(e) = db::insert_event(&log_json) {
                    eprintln!("DB insert error: {}", e);
                }
            }
        });
        println!("?좑툘  Running in lite mode (no LLM, events still saved)");
    }

    // 4. Start HTTP API Server for Desktop GUI
    println!("?뙋 Starting Desktop API Server...");
    let llm_for_api = llm_client.clone();
    tokio::spawn(async move {
        if let Err(e) = api_server::start_api_server(llm_for_api).await {
            eprintln!("?좑툘  Desktop API Server failed to start: {}", e);
            eprintln!("   (Continuing without API server)");
        }
    });

    // 5. Start File Watcher
    // Watch Downloads folder
    let downloads = dirs::download_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());

    // We reuse log_tx to send file events to Analyzer
    if let Err(e) = monitor::spawn_file_watcher(downloads.clone(), log_tx.clone()) {
        println!("?좑툘  Failed to watch {}: {}", downloads, e);
    } else {
        println!("?? Watching for changes in {}", downloads);
    }

    // 6. Start App Watcher (Active Window Poller)
    monitor::spawn_app_watcher(log_tx.clone());
    println!("?? Watching for active application changes...");

    let mut policy = policy::PolicyEngine::new(); // Starts LOCKED
    let mut res_mon = monitor::ResourceMonitor::new();

    if env_flag("STEER_DAEMON") {
        info!("Running in daemon mode (no interactive REPL).");
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    }

    // 5. User Input Loop (REPL)
    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin);
    let mut buffer = String::new();

    loop {
        buffer.clear();
        print!("> ");
        if let Err(e) = io::stdout().flush().await {
            eprintln!("?좑툘 Flush failed: {}", e);
        }

        if reader.read_line(&mut buffer).await? == 0 {
            // EOF - keep server running (headless mode)
            println!("?뱻 Running in headless mode (API only)...");
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            }
        }

        let input = buffer.trim();
        if input.is_empty() {
            continue;
        }

        let parts: Vec<&str> = input.split_whitespace().collect();
        match parts[0] {
            "help" => {
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
                println!("  analyze_patterns      - Detect behavior patterns and generate recommendations");
                println!("  quality               - Show workflow quality metrics");
                println!("  telegram <msg>        - Send Telegram message");
                println!("  notion <title>|<body> - Create Notion page");
                println!("  gmail list [N]        - List recent N emails");
                println!("  gmail read <id>       - Read email by ID");
                println!("  gmail send <to>|<subj>|<body> - Send email");
                println!("  calendar today        - Today's events");
                println!("  calendar week         - This week's events");
                println!("  calendar add <title>|<start>|<end> - Add event");
                println!("  exit                  - Quit");
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
                println!("[UI] Snapshotting...");
                #[cfg(target_os = "macos")]
                {
                    let tree = macos::accessibility::snapshot(scope);
                    println!("?뱞 Snapshot:\n{}", serde_json::to_string_pretty(&tree)?);
                }
                #[cfg(target_os = "windows")]
                {
                    let tree = windows::accessibility::snapshot(scope);
                    println!("Snapshot:\n{}", serde_json::to_string_pretty(&tree)?);
                }
            }
            "type" => {
                if parts.len() < 2 {
                    println!("Usage: type <text>");
                    continue;
                }
                let text = parts[1..].join(" ");
                // Policy Check
                match policy.check(&AgentAction::UiType { text: text.clone() }) {
                    Ok(_) => {
                        println!("??Policy Passed");
                        #[cfg(target_os = "macos")]
                        if let Err(e) = macos::actions::type_text(&text) {
                            println!("??Type failed: {}", e);
                        }
                        #[cfg(target_os = "windows")]
                        if let Err(e) = windows::actions::type_text(&text) {
                            println!("Type failed: {}", e);
                        }
                    }
                    Err(e) => println!("?뷂툘 Policy Blocked: {}", e),
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
                        println!("??Policy Passed");
                        #[cfg(target_os = "macos")]
                        if let Err(e) = macos::actions::click_element(id) {
                            println!("??Click failed: {}", e);
                        }
                        #[cfg(target_os = "windows")]
                        if let Err(e) = windows::actions::click_element(id) {
                            println!("Click failed: {}", e);
                        }
                    }
                    Err(e) => println!("?뷂툘 Policy Blocked: {}", e),
                }
            }
            "exec" => {
                if parts.len() < 2 {
                    println!("Usage: exec <command>");
                    continue;
                }
                let cmd = parts[1..].join(" ");

                // [Phase 8] Security Sandboxing
                match security::CommandClassifier::classify(&cmd) {
                    security::SafetyLevel::Critical => {
                        println!("?뷂툘 CRITICAL WARNING: This command is flagged as DANGEROUS.");
                        println!("   Command: {}", cmd);
                        println!("   To execute, type 'CONFIRM':");

                        buffer.clear();
                        if reader.read_line(&mut buffer).await? == 0 {
                            break;
                        }
                        if buffer.trim() != "CONFIRM" {
                            println!("??Aborted.");
                            continue;
                        }
                    }
                    security::SafetyLevel::Warning => {
                        println!("?좑툘  WARNING: This command may modify your system.");
                        println!("   Command: {}", cmd);
                        println!("   Execute? (y/n):");

                        buffer.clear();
                        if reader.read_line(&mut buffer).await? == 0 {
                            break;
                        }
                        if buffer.trim().to_lowercase() != "y" {
                            println!("??Aborted.");
                            continue;
                        }
                    }
                    security::SafetyLevel::Safe => {
                        // Safe to proceed automatically
                    }
                }

                let cwd = std::env::current_dir()
                    .ok()
                    .map(|p| p.to_string_lossy().to_string());
                let action = AgentAction::ShellExecution {
                    command: cmd.clone(),
                };
                match policy.check_with_context(&action, cwd.as_deref()) {
                    Ok(_) => {
                        println!("?숋툘  Executing: '{}'", cmd);
                        match bash_executor::exec(&cmd) {
                            Ok(out) => println!("Output:\n{}", out),
                            Err(e) => println!("??Exec failed: {}", e),
                        }
                    }
                    Err(e) => {
                        if let Ok(Some(_approval)) =
                            db::find_valid_exec_approval(&cmd, cwd.as_deref())
                        {
                            println!("??Approved command found. Executing: '{}'", cmd);
                            match bash_executor::exec(&cmd) {
                                Ok(out) => println!("Output:\n{}", out),
                                Err(e) => println!("??Exec failed: {}", e),
                            }
                        } else {
                            let approval =
                                db::create_exec_approval(&cmd, cwd.as_deref(), 3600).ok();
                            if let Some(approval) = approval {
                                println!("?뷂툘 Policy Blocked: {}", e);
                                println!("?뱷 Exec approval requested: {}", approval.id);
                                println!(
                                    "   Approve once: POST /api/exec-approvals/{}/approve",
                                    approval.id
                                );
                                println!("   Approve always: POST /api/exec-approvals/{}/approve ({{\"decision\":\"allow-always\"}})", approval.id);
                            } else {
                                println!("?뷂툘 Policy Blocked: {}", e);
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
                println!("?뙋 Opening URL: {}", url);
                #[cfg(target_os = "macos")]
                if let Err(e) = crate::applescript::open_url(url).map(|_| ()) {
                    println!("??Open failed: {}", e);
                }
                #[cfg(target_os = "windows")]
                {
                    let status = std::process::Command::new("cmd")
                        .args(["/C", "start", "", url])
                        .status();
                    if let Err(e) = status {
                        println!("Open failed: {}", e);
                    }
                }
            }
            "fake_log" => {
                // Simulate log
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
                    println!("??Simulated Log Sent");
                }
            }
            "routine" => {
                if let Some(brain) = &llm_client {
                    println!("?쭬 Analyzing daily routine (last 24h)...");
                    match db::get_recent_events(24) {
                        Ok(logs) => {
                            if logs.is_empty() {
                                println!("   (No events found in DB to analyze)");
                            } else {
                                println!("   Found {} events. Asking LLM...", logs.len());
                                match brain.analyze_routine(&logs).await {
                                    Ok(summary) => {
                                        println!("\n?뱤 Routine Analysis:\n{}", summary);
                                    }
                                    Err(e) => println!("??Analysis failed: {}", e),
                                }
                            }
                        }
                        Err(e) => println!("??DB Query failed: {}", e),
                    }
                } else {
                    println!("?좑툘  LLM Client not available.");
                }
            }
            "recommend" => {
                if let Some(brain) = &llm_client {
                    println!("?쨼 Generating automation recommendation...");
                    match db::get_recent_events(24) {
                        Ok(logs) => {
                            if logs.is_empty() {
                                println!("   (No events found in DB)");
                            } else {
                                match brain.recommend_automation(&logs).await {
                                    Ok(script) => {
                                        println!("\n??Recommendation:\n{}", script);
                                        println!("\n?뮕 Tip: Save code to a file and run with 'exec <file>'");
                                    }
                                    Err(e) => println!("??Recommendation failed: {}", e),
                                }
                            }
                        }
                        Err(e) => println!("??DB Query failed: {}", e),
                    }
                } else {
                    println!("?좑툘  LLM Client not available.");
                }
            }
            "analyze_patterns" | "detect" => {
                println!("?뵇 Analyzing behavior patterns...");
                let detector = pattern_detector::PatternDetector::new();
                let patterns = detector.analyze();

                if patterns.is_empty() {
                    println!("   (No significant patterns detected yet)");
                    println!("   Keep using your computer - patterns will be detected over time.");
                } else {
                    println!("   Found {} patterns:", patterns.len());
                    for pattern in &patterns {
                        println!(
                            "   ?뱤 {} ({} occurrences, {:.0}% similarity)",
                            pattern.description,
                            pattern.occurrences,
                            pattern.similarity_score * 100.0
                        );
                    }

                    // Generate recommendations if LLM available
                    if let Some(brain) = &llm_client {
                        println!("\n?쨼 Generating workflow recommendations...");
                        for pattern in patterns {
                            if pattern.occurrences >= 3 && pattern.similarity_score >= 0.8 {
                                match brain
                                    .generate_recommendation_from_pattern(
                                        &pattern.description,
                                        &pattern.sample_events,
                                    )
                                    .await
                                {
                                    Ok(mut proposal) => {
                                        // [Explainability] Inject hard evidence manually
                                        proposal
                                            .evidence
                                            .push(format!("Pattern: {}", pattern.description));
                                        proposal.evidence.push(format!(
                                            "Frequency: {} occurrences in last 7 days",
                                            pattern.occurrences
                                        ));

                                        if proposal.confidence >= 0.7 {
                                            if let Ok(true) = db::insert_recommendation(&proposal) {
                                                println!("   ??New recommendation: {} (confidence: {:.0}%)", 
                                                    proposal.title, proposal.confidence * 100.0);
                                            }
                                        }
                                    }
                                    Err(e) => println!("   ?좑툘  Skipped pattern: {}", e),
                                }
                            }
                        }
                        println!("\nRun 'recommendations' to see pending recommendations.");
                    }
                }
            }
            "quality" | "metrics" => {
                let collector = feedback_collector::FeedbackCollector::new();
                let metrics = collector.get_quality_metrics();
                println!("?뱢 Workflow Quality Metrics:");
                println!("   {}", metrics);
            }
            "status" => {
                println!("?뱤 System Status:");
                println!("   {}", res_mon.get_status());
                println!("   Top Apps:");
                for (name, usage) in res_mon.get_high_usage_apps() {
                    println!("   - {}: {:.1}%", name, usage);
                }
            }
            "recommendations" | "recs" => {
                let limit = parts
                    .get(1)
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(5);
                match db::list_recommendations("pending", limit) {
                    Ok(recs) => {
                        if recs.is_empty() {
                            println!("(No pending recommendations)");
                        } else {
                            println!("?㎥ Pending recommendations:");
                            for rec in recs {
                                println!(
                                    "  [{}] {} (confidence {:.2})",
                                    rec.id, rec.title, rec.confidence
                                );
                                println!("       Trigger: {}", rec.trigger);
                                println!("       Summary: {}", rec.summary);
                            }
                        }
                    }
                    Err(e) => println!("??Failed to load recommendations: {}", e),
                }
            }
            "approve" => {
                if parts.len() < 2 {
                    println!("Usage: approve <id>");
                    continue;
                }
                let id: i64 = match parts[1].parse() {
                    Ok(v) => v,
                    Err(_) => {
                        println!("Usage: approve <id>");
                        continue;
                    }
                };
                let rec = match db::get_recommendation(id) {
                    Ok(Some(r)) => r,
                    Ok(None) => {
                        println!("No recommendation found for id {}", id);
                        continue;
                    }
                    Err(e) => {
                        println!("??Failed to read recommendation: {}", e);
                        continue;
                    }
                };

                if rec.status.eq_ignore_ascii_case("rejected") {
                    println!("??Recommendation {} is rejected and cannot be approved.", id);
                    continue;
                }
                if !rec.status.eq_ignore_ascii_case("approved") {
                    if let Err(e) = db::update_recommendation_status(id, "approved") {
                        println!("??Failed to update recommendation status: {}", e);
                        continue;
                    }
                }

                println!("?룛截? Building n8n workflow for '{}'...", rec.title);
                match execute_approved_recommendation(id, llm_client.clone()).await {
                    Ok(workflow_id) => {
                        println!("??Workflow created! ID: {}", workflow_id);
                    }
                    Err(e) => {
                        println!("??Approval pipeline failed: {}", e);
                    }
                }
            }
            "approve_test" => {
                if parts.len() < 2 {
                    println!("Usage: approve_test <id>");
                    continue;
                }
                let id: i64 = match parts[1].parse() {
                    Ok(v) => v,
                    Err(_) => {
                        println!("Usage: approve_test <id>");
                        continue;
                    }
                };

                if let Err(e) = maybe_assume_approved_for_test(id) {
                    println!("??approve_test precheck failed: {}", e);
                    continue;
                }
                match execute_approved_recommendation(id, llm_client.clone()).await {
                    Ok(workflow_id) => {
                        println!("??[TEST] Workflow created! ID: {}", workflow_id);
                    }
                    Err(e) => {
                        println!("??[TEST] Approval pipeline failed: {}", e);
                    }
                }
            }
            "ingest_mock_workflow" => {
                if let Err(e) = ingest_mock_workflow_recommendation(llm_client.clone()).await {
                    println!("??Mock workflow ingest failed: {}", e);
                }
            }
            "reject" => {
                if parts.len() < 2 {
                    println!("Usage: reject <id>");
                    continue;
                }
                let id: i64 = match parts[1].parse() {
                    Ok(v) => v,
                    Err(_) => {
                        println!("Usage: reject <id>");
                        continue;
                    }
                };
                match db::update_recommendation_status(id, "rejected") {
                    Ok(()) => println!("?뿊截? Recommendation {} rejected.", id),
                    Err(e) => println!("??Failed to reject recommendation: {}", e),
                }
            }
            "control" => {
                if parts.len() < 3 {
                    println!("Usage: control <app> <action> (e.g., control Music play)");
                    continue;
                }
                let app = parts[1];
                let command = parts[2];
                println!("?렜 Controlling {} with '{}'...", app, command);
                match applescript::control_app(app, command) {
                    Ok(out) => {
                        if !out.is_empty() {
                            println!("Output: {}", out);
                        }
                        println!("??Command sent.");
                    }
                    Err(e) => println!("??Control failed: {}", e),
                }
            }
            "build_workflow" => {
                if parts.len() < 2 {
                    println!("Usage: build_workflow <prompt>");
                    continue;
                }
                let prompt = parts[1..].join(" ");

                if let Some(brain) = &llm_client {
                    println!("?룛截? Designing n8n workflow for: '{}'...", prompt);
                    // 1. Generate JSON
                    match brain.build_n8n_workflow(&prompt).await {
                        Ok(json_str) => {
                            println!("?쨼 Blueprint generated. Importing to n8n...");
                            // 2. Import to n8n
                            let n8n_url = std::env::var("N8N_API_URL")
                                .unwrap_or_else(|_| "http://localhost:5678".to_string());
                            let n8n_key = std::env::var("N8N_API_KEY").unwrap_or_default();
                            let n8n =
                                n8n_api::N8nApi::new(&format!("{}/api/v1", n8n_url), &n8n_key);

                            // Parse JSON string to Value
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                                match n8n.create_workflow("Agent Generated Workflow", &val, true).await {
                                    Ok(id) => println!("??Workflow Created! ID: {}\n   (Check your n8n dashboard)", id),
                                    Err(e) => {
                                        println!("??API Import failed: {}", e);
                                        println!("?뫛 Activating Visual Fallback (Phantom Hand)...");
                                        // Trigger visual fallback
                                        let fallback = visual_driver::n8n_fallback_create_workflow();
                                        if let Err(ve) = fallback.execute(None).await {
                                            println!("??Visual Fallback also failed: {}", ve);
                                        }
                                    },
                                }
                            } else {
                                println!("??LLM produced invalid JSON.");
                            }
                        }
                        Err(e) => println!("??Generation failed: {}", e),
                    }
                } else {
                    println!("?좑툘  LLM Client not available.");
                }
            }
            "telegram" => {
                if parts.len() < 2 {
                    println!("Usage: telegram <message>");
                    continue;
                }
                let message = parts[1..].join(" ");
                println!("?벑 Sending to Telegram...");
                match integrations::telegram::TelegramBot::from_env() {
                    Ok(bot) => match bot.send(&message).await {
                        Ok(_) => println!("??Message sent!"),
                        Err(e) => println!("??Failed: {}", e),
                    },
                    Err(e) => println!("?좑툘  Telegram not configured: {}", e),
                }
            }
            "notion" => {
                // Usage: notion <title> | <content>
                if parts.len() < 2 {
                    println!("Usage: notion <title> | <content>");
                    continue;
                }
                let full_text = parts[1..].join(" ");
                let split: Vec<&str> = full_text.splitn(2, '|').collect();
                let title = split.first().unwrap_or(&"Untitled").trim();
                let content = split.get(1).unwrap_or(&"").trim();

                let db_id = std::env::var("NOTION_DATABASE_ID").unwrap_or_default();
                if db_id.is_empty() {
                    println!("?좑툘  NOTION_DATABASE_ID not set in .env");
                    continue;
                }

                println!("?뱷 Creating Notion page: '{}'...", title);
                match integrations::notion::NotionClient::from_env() {
                    Ok(client) => match client.create_page(&db_id, title, content).await {
                        Ok(page_id) => println!("??Page created! ID: {}", page_id),
                        Err(e) => println!("??Failed: {}", e),
                    },
                    Err(e) => println!("?좑툘  Notion not configured: {}", e),
                }
            }
            "gmail" => {
                if parts.len() < 2 {
                    println!(
                        "Usage: gmail list [N] | gmail read <id> | gmail send <to>|<subj>|<body>"
                    );
                    continue;
                }
                match parts[1] {
                    "list" => {
                        let count = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);
                        println!("?벁 Fetching {} recent emails...", count);
                        match integrations::gmail::GmailClient::new().await {
                            Ok(client) => match client.list_messages(count).await {
                                Ok(messages) => {
                                    if messages.is_empty() {
                                        println!("   (No messages found)");
                                    } else {
                                        for (id, subject, from) in messages {
                                            println!(
                                                "  ?벃 [{}] {} ??{}",
                                                &id[..8.min(id.len())],
                                                subject,
                                                from
                                            );
                                        }
                                    }
                                }
                                Err(e) => println!("??Failed: {}", e),
                            },
                            Err(e) => println!("?좑툘  Gmail auth failed: {}", e),
                        }
                    }
                    "read" => {
                        if parts.len() < 3 {
                            println!("Usage: gmail read <id>");
                            continue;
                        }
                        let id = parts[2];
                        println!("?뱰 Reading email {}...", id);
                        match integrations::gmail::GmailClient::new().await {
                            Ok(client) => match client.get_message(id).await {
                                Ok(content) => println!("\n{}", content),
                                Err(e) => println!("??Failed: {}", e),
                            },
                            Err(e) => println!("?좑툘  Gmail auth failed: {}", e),
                        }
                    }
                    "send" => {
                        let full_text = parts[2..].join(" ");
                        let split: Vec<&str> = full_text.splitn(3, '|').collect();
                        if split.len() < 3 {
                            println!("Usage: gmail send <to>|<subject>|<body>");
                            continue;
                        }
                        let to = split[0].trim();
                        let subject = split[1].trim();
                        let body = split[2].trim();

                        println!("?됵툘  Sending email to {}...", to);
                        match integrations::gmail::GmailClient::new().await {
                            Ok(client) => match client.send_message(to, subject, body).await {
                                Ok(id) => println!("??Email sent! ID: {}", id),
                                Err(e) => println!("??Failed: {}", e),
                            },
                            Err(e) => println!("?좑툘  Gmail auth failed: {}", e),
                        }
                    }
                    _ => println!("Unknown gmail subcommand. Use: list, read, send"),
                }
            }
            "calendar" => {
                if parts.len() < 2 {
                    println!("Usage: calendar today | week | add <title>|<start>|<end>");
                    continue;
                }
                match parts[1] {
                    "today" => {
                        println!("?뱟 Fetching today's events...");
                        match integrations::calendar::CalendarClient::new().await {
                            Ok(client) => match client.list_today().await {
                                Ok(events) => {
                                    if events.is_empty() {
                                        println!("   (No events today)");
                                    } else {
                                        for (_, summary, time) in events {
                                            println!("  ?뿎截? {} ??{}", time, summary);
                                        }
                                    }
                                }
                                Err(e) => println!("??Failed: {}", e),
                            },
                            Err(e) => println!("?좑툘  Calendar auth failed: {}", e),
                        }
                    }
                    "week" => {
                        println!("?뱟 Fetching this week's events...");
                        match integrations::calendar::CalendarClient::new().await {
                            Ok(client) => match client.list_week().await {
                                Ok(events) => {
                                    if events.is_empty() {
                                        println!("   (No events this week)");
                                    } else {
                                        for (_, summary, time) in events {
                                            println!("  ?뿎截? {} ??{}", time, summary);
                                        }
                                    }
                                }
                                Err(e) => println!("??Failed: {}", e),
                            },
                            Err(e) => println!("?좑툘  Calendar auth failed: {}", e),
                        }
                    }
                    "add" => {
                        let full_text = parts[2..].join(" ");
                        let split: Vec<&str> = full_text.splitn(3, '|').collect();
                        if split.len() < 3 {
                            println!("Usage: calendar add <title>|<start ISO>|<end ISO>");
                            println!("Example: calendar add Meeting|2026-01-25T14:00:00+09:00|2026-01-25T15:00:00+09:00");
                            continue;
                        }
                        let title = split[0].trim();
                        let start = split[1].trim();
                        let end = split[2].trim();

                        info!("??Adding event: '{}'...", title);
                        match integrations::calendar::CalendarClient::new().await {
                            Ok(client) => match client.create_event(title, start, end).await {
                                Ok(id) => info!("??Event created! ID: {}", id),
                                Err(e) => error!("??Failed: {}", e),
                            },
                            Err(e) => warn!("?좑툘  Calendar auth failed: {}", e),
                        }
                    }
                    _ => warn!("Unknown calendar subcommand. Use: today, week, add"),
                }
            }
            "surf" => {
                if parts.len() < 2 {
                    warn!("Usage: surf <goal>");
                    continue;
                }
                let goal = parts[1..].join(" ");

                if let Some(brain) = &llm_client {
                    let planner =
                        local_os_agent::controller::planner::Planner::new(brain.clone(), None);
                    // Run concurrently to allow Ctrl+C? For now blocking is fine as it has internal timeout/loop
                    if let Err(e) = planner.run_goal(&goal, None).await {
                        error!("??Surf failed: {}", e);
                    }
                } else {
                    warn!("?좑툘  LLM Client not available.");
                }
            }
            // Super Agent Mode (Unified Orchestrator)
            _ => {
                if let Ok(orch) = orchestrator::Orchestrator::new().await {
                    info!("?쨼 Super Agent: Processing '{}'...", input);
                    match orch.handle_request(input).await {
                        Ok(resp) => info!("{}", resp),
                        Err(e) => error!("??Super Agent Error: {}", e),
                    }
                } else {
                    warn!("?좑툘  Orchestrator could not initialization.");
                }
            }
        }
    }

    Ok(())
}
