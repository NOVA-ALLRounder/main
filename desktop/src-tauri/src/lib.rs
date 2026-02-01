use tauri_plugin_shell::ShellExt;
use tauri::{AppHandle, Manager, Emitter, State};
use std::sync::Arc;
use local_os_agent::llm_gateway::LLMClient;
use local_os_agent::dynamic_controller::DynamicController;

// Simple state to hold the controller? 
// No, Controller is created per run usually, or we hold one globally.
// Let's create it per run for now, but we need LLMClient.
struct AgentState {
    llm: Arc<LLMClient>,
}

#[tauri::command]
async fn run_agent_task(goal: String, _app: AppHandle, state: State<'_, AgentState>) -> Result<(), String> {
    // We want to capture logs.
    // Since DynamicController now uses `log::info!`, we can rely on `tauri-plugin-log` 
    // to forward those to the frontend IF we configured it?
    // BUT `tauri-plugin-log` writes to a file or console. It doesn't auto-emit "log-event" to frontend JavaScript.
    // Wait, tauri-plugin-log DOES have a `attachConsole` option which logs to webview console!
    // That is perfect.
    
    // So we just run the controller.
    let controller = DynamicController::new((*state.llm).clone());
    
    match controller.surf(&goal).await {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
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
        let llm = LLMClient::new().map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())) as Box<dyn std::error::Error>)?;
        app.manage(AgentState { llm: Arc::new(llm) });
        Ok(())
    })
    .invoke_handler(tauri::generate_handler![run_agent_task])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
