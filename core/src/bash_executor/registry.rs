use anyhow::Result;
use std::collections::HashMap;
use std::process::Command;
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub id: String,
    pub pid: u32,
    pub command: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub status: ProcessStatus,
    pub output: Vec<String>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProcessStatus {
    Running,
    Completed,
    Failed,
    Killed,
}

lazy_static::lazy_static! {
    static ref PROCESS_REGISTRY: Mutex<HashMap<String, ProcessInfo>> = Mutex::new(HashMap::new());
}

pub fn register_process(id: &str, pid: u32, command: &str) {
    let info = ProcessInfo {
        id: id.to_string(),
        pid,
        command: command.to_string(),
        started_at: chrono::Utc::now(),
        status: ProcessStatus::Running,
        output: Vec::new(),
        exit_code: None,
    };

    if let Ok(mut registry) = PROCESS_REGISTRY.lock() {
        registry.insert(id.to_string(), info);
        println!("📋 [Process] Registered: {} (PID {})", id, pid);
    }
}

pub fn update_process(
    id: &str,
    status: ProcessStatus,
    exit_code: Option<i32>,
    output: Option<Vec<String>>,
) {
    if let Ok(mut registry) = PROCESS_REGISTRY.lock() {
        if let Some(info) = registry.get_mut(id) {
            info.status = status;
            info.exit_code = exit_code;
            if let Some(lines) = output {
                info.output = lines;
            }
        }
    }
}

pub fn get_process(id: &str) -> Option<ProcessInfo> {
    PROCESS_REGISTRY.lock().ok()?.get(id).cloned()
}

pub fn list_active_processes() -> Vec<ProcessInfo> {
    PROCESS_REGISTRY
        .lock()
        .ok()
        .map(|r| {
            r.values()
                .filter(|p| p.status == ProcessStatus::Running)
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

pub fn kill_process(id: &str) -> Result<bool> {
    let info = get_process(id).ok_or_else(|| anyhow::anyhow!("Process not found"))?;
    let kill_result = Command::new("kill")
        .args(["-15", &info.pid.to_string()])
        .status()?;

    if kill_result.success() {
        update_process(id, ProcessStatus::Killed, None, None);
        println!("🔪 [Process] Killed: {} (PID {})", id, info.pid);
        Ok(true)
    } else {
        Ok(false)
    }
}
