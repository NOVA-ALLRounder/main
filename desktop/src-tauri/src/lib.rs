use tauri_plugin_shell::ShellExt;
use tauri::{AppHandle, Manager, Emitter, State};
use std::sync::Arc;
use local_os_agent::llm_gateway::LLMClient;
use local_os_agent::dynamic_controller::DynamicController;
use local_os_agent::analyzer;
use local_os_agent::telegram::TelegramBot;
use local_os_agent::db::{self, DashboardStats, Recommendation, LearnedRoutine};
use local_os_agent::architect::ArchitectSession;
use local_os_agent::config_manager::ConfigManager;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use std::collections::HashMap;


// Simple state to hold the controller? 
// No, Controller is created per run usually, or we hold one globally.
// Let's create it per run for now, but we need LLMClient.
struct AgentState {
    llm: Arc<LLMClient>,
    tx: mpsc::Sender<String>,
    architect: Arc<Mutex<Option<ArchitectSession>>>,
}

#[tauri::command]
async fn run_agent_task(goal: String, _app: AppHandle, state: State<'_, AgentState>) -> Result<String, String> {
    
    // Check if Architect Mode is active
    let mut arch_lock = state.architect.lock().await;
    if let Some(session) = arch_lock.as_mut() {
        log::info!("🧠 Routing to Architect: {}", goal);
        return session.chat(&goal).await;
    }
    drop(arch_lock);

    // Default: Dynamic Agent
    // Clone sender for the controller
    let tx = state.tx.clone();
    let controller = DynamicController::new((*state.llm).clone(), Some(tx));
    
    match controller.surf(&goal).await {
        Ok(_) => Ok("Task Completed.".to_string()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn start_architect_mode(rec_id: i64, state: State<'_, AgentState>) -> Result<String, String> {
    let rec = db::get_recommendation(rec_id).map_err(|e| e.to_string())?
        .ok_or("Recommendation not found")?;
    
    // Create Session
    let mut session = ArchitectSession::new(state.llm.clone(), rec);
    let opening = session.start().await.map_err(|e| e.to_string())?;

    // Save to State
    let mut lock = state.architect.lock().await;
    *lock = Some(session);

    Ok(opening)
}

#[tauri::command]
async fn get_dashboard_data() -> Result<DashboardStats, String> {
    db::get_dashboard_stats().map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_recommendations() -> Result<Vec<Recommendation>, String> {
    db::get_recent_recommendations(10).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_config() -> Result<HashMap<String, String>, String> {
    let cm = ConfigManager::new();
    Ok(cm.get_all())
}

#[tauri::command]
async fn set_config(key: String, value: String) -> Result<(), String> {
    let cm = ConfigManager::new();
    cm.update(&key, &value)
}

#[tauri::command]
async fn list_routines() -> Result<Vec<LearnedRoutine>, String> {
    db::list_learned_routines().map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_routine(id: i64) -> Result<(), String> {
    db::delete_learned_routine(id).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_shell::init())
    .plugin(tauri_plugin_updater::Builder::new().build())
    .plugin(tauri_plugin_log::Builder::default()
        .target(tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout))
        .target(tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir { file_name: None }))
        .target(tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview))
        .level(log::LevelFilter::Info)
        .build())
    .setup(|app| {
        // LOAD .ENV FROM RESOURCES (Fix for Release Build)
        let resource_path = app.path().resource_dir()
            .map(|path| path.join(".env"))
            .unwrap_or_else(|_| std::path::PathBuf::from(".env"));

        if resource_path.exists() {
             dotenv::from_path(&resource_path).ok();
        } else {
             // Fallback for dev mode
             dotenv::dotenv().ok();
        }

        // Init LLM Client
        let llm = Arc::new(LLMClient::new().map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())) as Box<dyn std::error::Error>)?);
        
        // Init Analyzer Channel
        let (tx, rx) = mpsc::channel(100);
        
        // Spawn Analyzer
        let llm_clone = llm.clone();
        analyzer::spawn(rx, llm_clone);

        // Spawn Telegram Bot (if configured)
        let llm_for_bot = llm.clone();
        let tx_for_bot = tx.clone();
        if let Some(bot) = TelegramBot::from_env(llm_for_bot, Some(tx_for_bot)) {
             let bot_arc = Arc::new(bot);
             tauri::async_runtime::spawn(async move {
                 bot_arc.start_polling().await;
             });
        }

        app.manage(AgentState { llm, tx, architect: Arc::new(Mutex::new(None)) });
        Ok(())
    })
    })
    .invoke_handler(tauri::generate_handler![
        run_agent_task, 
        get_dashboard_data, 
        get_recommendations, 
        start_architect_mode,
        get_config,
        set_config,
        list_routines,
        delete_routine
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
