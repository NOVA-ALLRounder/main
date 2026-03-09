use local_os_agent::{analyzer, api_server, db, env_flag, llm_gateway, mcp_client, monitor};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[cfg(target_os = "macos")]
use local_os_agent::macos;

pub(crate) fn install_panic_hook() {
    if !env_flag("STEER_PANIC_STD") {
        std::panic::set_hook(Box::new(|info| {
            let backtrace = std::backtrace::Backtrace::capture();
            let timestamp = chrono::Utc::now().to_rfc3339();

            let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
                *s
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                &**s
            } else {
                "Unknown panic"
            };

            let location = info
                .location()
                .map(|l| format!("{}:{}", l.file(), l.line()))
                .unwrap_or_else(|| "unknown".to_string());

            let log_entry = format!(
                "[{}] CRASH REPORT\nMessage: {}\nLocation: {}\nBacktrace:\n{:#?}\n--------------------------------------------------\n",
                timestamp, msg, location, backtrace
            );

            let home = std::env::var("HOME").unwrap_or("/".to_string());
            let log_dir = std::path::Path::new(&home).join(".steer/logs");
            if std::fs::create_dir_all(&log_dir).is_ok() {
                let log_file = log_dir.join("crash.log");
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log_file)
                {
                    use std::io::Write;
                    let _ = writeln!(file, "{}", log_entry);
                }
            }

            eprintln!("❌ FATAL ERROR: {}", msg);
            eprintln!("📄 Crash report saved to ~/.steer/logs/crash.log");
        }));
    } else {
        eprintln!("⚠️  Panic hook disabled (STEER_PANIC_STD=1).");
    }
}

pub(crate) async fn print_startup_banner_and_checks() {
    println!("🤖 Local OS Agent (Rust Native Mode) Started!");
    println!("🔍 Checking Accessibility Permissions...");
    let ax_check = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tokio::task::spawn_blocking(|| {
            std::process::Command::new("osascript")
                .arg("-e")
                .arg("tell application \"System Events\" to return name of first application process")
                .output()
        }),
    )
    .await;

    match ax_check {
        Ok(Ok(Ok(output))) if output.status.success() => {
            println!("✅ Accessibility Permissions: GRANTED.");
        }
        _ => {
            println!("\n\n################################################################");
            println!("❌ WARNING: ACCESSIBILITY PERMISSIONS MISSING OR REVOKED!");
            println!("   The agent can launch apps but CANNOT click or type.");
            println!("   FIX: Go to System Settings -> Privacy -> Accessibility");
            println!("   ACTION: Remove (-) and Re-add (+) your Terminal / Agent.");
            println!("################################################################\n\n");
        }
    }

    println!("--------------------------------------------------");
    let health = local_os_agent::dependency_check::SystemHealth::check_all();
    health.print_report();
    println!("Type 'help' for commands. (Needs Accessibility Permissions)");
    println!("--------------------------------------------------");
}

pub(crate) fn init_db_and_llm() -> Option<Arc<dyn llm_gateway::LLMClient>> {
    if let Err(e) = db::init() {
        eprintln!("Failed to init DB: {}", e);
    }

    match llm_gateway::OpenAILLMClient::new() {
        Ok(c) => Some(Arc::new(c)),
        Err(e) => {
            warn!("⚠️ Failed to init LLM Gateway: {}", e);
            None
        }
    }
}

pub(crate) async fn start_background_services(
    llm_client: Option<Arc<dyn llm_gateway::LLMClient>>,
) -> mpsc::Sender<String> {
    if let Some(llm) = &llm_client {
        let scheduler = local_os_agent::scheduler::Scheduler::new(llm.clone());
        scheduler.start();
        info!("🧠 Brain Routine Scheduler Active.");
    }

    match tokio::time::timeout(
        std::time::Duration::from_secs(8),
        tokio::task::spawn_blocking(mcp_client::init_mcp),
    )
    .await
    {
        Ok(Ok(Ok(()))) => info!("🔌 MCP System Initialized."),
        Ok(Ok(Err(e))) => warn!("⚠️ Failed to init MCP: {}", e),
        Ok(Err(e)) => warn!("⚠️ MCP init join error: {}", e),
        Err(_) => warn!("⚠️ MCP init timeout (continuing without blocking startup)"),
    }

    let (log_tx, log_rx) = mpsc::channel::<String>(1000);

    #[cfg(target_os = "macos")]
    {
        if env_flag("STEER_DISABLE_EVENT_TAP") {
            info!("⚠️  Event Tap disabled via STEER_DISABLE_EVENT_TAP.");
        } else if let Err(e) = macos::events::start_event_tap(log_tx.clone()) {
            error!("❌ Failed to start Event Tap: {}", e);
        }
    }

    let spawn_event_persist_only = |mut rx: mpsc::Receiver<String>| {
        tokio::spawn(async move {
            while let Some(log_json) = rx.recv().await {
                if let Err(e) = db::insert_event(&log_json) {
                    eprintln!("DB insert error: {}", e);
                }
            }
        });
    };

    if let Some(c) = llm_client.clone() {
        if env_flag("STEER_DISABLE_ANALYZER") {
            spawn_event_persist_only(log_rx);
            println!(
                "⚠️  Shadow Analyzer disabled via STEER_DISABLE_ANALYZER=1 (events still saved)"
            );
        } else {
            analyzer::spawn(log_rx, c);
        }
    } else {
        spawn_event_persist_only(log_rx);
        println!("⚠️  Running in lite mode (no LLM, events still saved)");
    }

    println!("🌐 Starting Desktop API Server...");
    let llm_for_api = llm_client.clone();
    tokio::spawn(async move {
        if let Err(e) = api_server::start_api_server(llm_for_api).await {
            eprintln!("⚠️  Desktop API Server failed to start: {}", e);
            eprintln!("   (Continuing without API server)");
        }
    });

    if env_flag("STEER_DISABLE_DOWNLOAD_WATCHER") {
        println!("ℹ️  Downloads watcher disabled via STEER_DISABLE_DOWNLOAD_WATCHER.");
    } else {
        let home = std::env::var("HOME").unwrap_or("/".to_string());
        let downloads = std::env::var("STEER_DOWNLOADS_DIR")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("{}/Downloads", home));

        if let Err(e) = monitor::spawn_file_watcher(downloads.clone(), log_tx.clone()) {
            println!("⚠️  Failed to watch {}: {}", downloads, e);
        } else {
            println!("👀 Watching for changes in {}", downloads);
        }
    }

    if env_flag("STEER_DISABLE_APP_WATCHER") {
        println!("ℹ️  App watcher disabled via STEER_DISABLE_APP_WATCHER.");
    } else {
        monitor::spawn_app_watcher(log_tx.clone());
        println!("👀 Watching for active application changes...");
    }

    log_tx
}
