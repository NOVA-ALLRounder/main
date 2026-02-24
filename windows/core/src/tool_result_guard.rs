use crate::db;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultGuardRequest {
    pub max_age_secs: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultGuardResult {
    pub ok: bool,
    pub scanned: usize,
    pub timed_out: usize,
    pub warnings: Vec<String>,
    pub template: String,
}

pub fn guard_exec_results(req: ToolResultGuardRequest) -> ToolResultGuardResult {
    let max_age_secs = req.max_age_secs.unwrap_or(300);
    let limit = req.limit.unwrap_or(200);
    let mut warnings = Vec::new();

    let pending = match db::list_pending_exec_results(limit) {
        Ok(rows) => rows,
        Err(err) => {
            return ToolResultGuardResult {
                ok: false,
                scanned: 0,
                timed_out: 0,
                warnings: vec![format!("Failed to list pending exec results: {}", err)],
                template: "tool_result_guard".to_string(),
            };
        }
    };

    let now = Utc::now();
    let mut timed_out = 0usize;

    for row in &pending {
        let created_at = match DateTime::parse_from_rfc3339(&row.created_at) {
            Ok(ts) => ts.with_timezone(&Utc),
            Err(_) => {
                warnings.push(format!("Invalid created_at for exec result {}", row.id));
                continue;
            }
        };
        let age = now.signed_duration_since(created_at).num_seconds();
        if age >= max_age_secs {
            let reason = format!("Timed out after {}s without tool result", age);
            let _ = db::update_exec_result(&row.id, "timeout", None, Some(&reason));
            timed_out += 1;
        }
    }

    ToolResultGuardResult {
        ok: true,
        scanned: pending.len(),
        timed_out,
        warnings,
        template: "tool_result_guard".to_string(),
    }
}
