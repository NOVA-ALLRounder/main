use axum::Json;
use serde::Serialize;
use sysinfo::System;

#[derive(Serialize)]
pub struct SystemStatus {
    pub cpu_usage: f32,
    pub memory_used: u64,
    pub memory_total: u64,
}

#[derive(Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

pub async fn get_system_status() -> Json<SystemStatus> {
    let mut sys = System::new_all();
    sys.refresh_cpu();
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_cpu();
    sys.refresh_memory();

    let cpu_usage = sys.global_cpu_info().cpu_usage();
    let memory_used = sys.used_memory() as f32 / 1024.0 / 1024.0;
    let memory_total = sys.total_memory() as f32 / 1024.0 / 1024.0;

    Json(SystemStatus {
        cpu_usage,
        memory_used: memory_used as u64,
        memory_total: memory_total as u64,
    })
}

pub async fn get_recent_logs() -> Json<Vec<LogEntry>> {
    match crate::db::get_all_routines() {
        Ok(routines) => {
            let mut logs = Vec::new();
            for r in routines {
                if let Some(last) = r.last_run {
                    logs.push(LogEntry {
                        timestamp: last,
                        level: "INFO".to_string(),
                        message: format!("Routine Executed: {}", r.name),
                    });
                }
            }
            logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            Json(logs)
        }
        Err(_) => Json(vec![]),
    }
}
