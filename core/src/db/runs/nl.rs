use std::collections::HashMap;

use rusqlite::{params, Result};

use super::super::{get_db_lock, list_launch_ops_events, truncate_text, LaunchOpsEventRecord};

#[derive(Debug, Clone, serde::Serialize)]
pub struct NLRun {
    pub id: i64,
    pub created_at: String,
    pub intent: String,
    pub prompt: String,
    pub status: String,
    pub summary: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NLRunMetrics {
    pub total: i64,
    pub completed: i64,
    pub manual_required: i64,
    pub approval_required: i64,
    pub blocked: i64,
    pub error: i64,
    pub success_rate: f64,
}
pub fn clear_nl_runs_for_tests() {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let _ = conn.execute("DELETE FROM nl_runs", []);
    }
}

pub fn insert_nl_run(
    intent: &str,
    prompt: &str,
    status: &str,
    summary: Option<&str>,
    details: Option<&str>,
) -> Result<()> {
    let _ = insert_nl_run_with_source_key(None, None, intent, prompt, status, summary, details)?;
    Ok(())
}

pub fn insert_nl_run_with_source_key(
    created_at: Option<&str>,
    source_key: Option<&str>,
    intent: &str,
    prompt: &str,
    status: &str,
    summary: Option<&str>,
    details: Option<&str>,
) -> Result<bool> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = created_at
            .map(|value| truncate_text(value, 64))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
        let source_key = source_key
            .map(|value| truncate_text(value, 190))
            .filter(|value| !value.is_empty());
        let changed = conn.execute(
            "INSERT INTO nl_runs (created_at, intent, prompt, status, summary, details, source_key)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(source_key) DO NOTHING",
            params![created_at, intent, prompt, status, summary, details, source_key],
        )?;
        return Ok(changed > 0);
    }
    Ok(false)
}

pub fn list_nl_runs(limit: i64) -> Result<Vec<NLRun>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, created_at, intent, prompt, status, summary, details
             FROM nl_runs
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(NLRun {
                id: row.get(0)?,
                created_at: row.get(1)?,
                intent: row.get(2)?,
                prompt: row.get(3)?,
                status: row.get(4)?,
                summary: row.get(5).ok(),
                details: row.get(6).ok(),
            })
        })?;
        let mut runs = Vec::new();
        for row in rows {
            runs.push(row?);
        }
        return Ok(runs);
    }
    Ok(Vec::new())
}

pub fn get_nl_run_metrics(limit: i64) -> Result<NLRunMetrics> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT
                COUNT(*) as total,
                COALESCE(SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END), 0) as completed,
                COALESCE(SUM(CASE WHEN status = 'manual_required' THEN 1 ELSE 0 END), 0) as manual_required,
                COALESCE(SUM(CASE WHEN status = 'approval_required' THEN 1 ELSE 0 END), 0) as approval_required,
                COALESCE(SUM(CASE WHEN status = 'blocked' THEN 1 ELSE 0 END), 0) as blocked,
                COALESCE(SUM(CASE WHEN status = 'error' THEN 1 ELSE 0 END), 0) as error_count
             FROM (
                SELECT status
                FROM nl_runs
                ORDER BY created_at DESC
                LIMIT ?1
             )",
        )?;
        let metrics = stmt.query_row(params![limit], |row| {
            let total: i64 = row.get(0)?;
            let completed: i64 = row.get(1)?;
            let manual_required: i64 = row.get(2)?;
            let approval_required: i64 = row.get(3)?;
            let blocked: i64 = row.get(4)?;
            let error: i64 = row.get(5)?;
            let success_rate = if total > 0 {
                (completed as f64) / (total as f64) * 100.0
            } else {
                0.0
            };
            Ok(NLRunMetrics {
                total,
                completed,
                manual_required,
                approval_required,
                blocked,
                error,
                success_rate,
            })
        })?;
        return Ok(metrics);
    }
    Ok(NLRunMetrics {
        total: 0,
        completed: 0,
        manual_required: 0,
        approval_required: 0,
        blocked: 0,
        error: 0,
        success_rate: 0.0,
    })
}

fn normalize_release_nl_prompt(prompt: &str) -> String {
    prompt
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn release_nl_details_value(run: &NLRun) -> Option<serde_json::Value> {
    let raw = run.details.as_deref()?.trim();
    if raw.is_empty() || !raw.starts_with('{') {
        return None;
    }
    serde_json::from_str(raw).ok()
}

fn release_nl_detail_str<'a>(details: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    details.get(key).and_then(|value| value.as_str())
}

fn is_release_nl_noise(run: &NLRun) -> bool {
    let prompt = normalize_release_nl_prompt(&run.prompt);
    if prompt.is_empty() || prompt.starts_with('/') {
        return true;
    }

    if let Some(details) = release_nl_details_value(run) {
        let route_kind = release_nl_detail_str(&details, "route_kind").unwrap_or_default();
        let command = release_nl_detail_str(&details, "command").unwrap_or_default();
        let source = release_nl_detail_str(&details, "source").unwrap_or_default();
        if source == "api.chat"
            && matches!(
                route_kind,
                "empty_message"
                    | "gate_blocked"
                    | "system_command"
                    | "local_command"
                    | "vision_demo"
            )
        {
            return true;
        }
        if matches!(
            command,
            "help_local"
                | "greeting_local"
                | "system_status"
                | "telegram_listener_start"
                | "telegram_listener_status"
                | "n8n_restart"
        ) {
            return true;
        }
    }

    let summary = run.summary.as_deref().unwrap_or_default().to_lowercase();
    let details = run.details.as_deref().unwrap_or_default().to_lowercase();
    let combined = format!("{}\n{}", summary, details);

    if combined.contains("launch eval")
        || combined.contains("launch.eval")
        || combined.contains("auto-finalized orphaned in-flight run")
    {
        return true;
    }

    let flight_like = prompt.contains("항공권")
        || prompt.contains("flight")
        || summary.contains("search flights")
        || details.contains("(flight_search)");
    if flight_like && combined.contains(" on unknown") {
        return true;
    }

    false
}

fn should_backfill_launch_ops_event(event: &LaunchOpsEventRecord) -> bool {
    if normalize_release_nl_prompt(&event.message_preview).is_empty() {
        return false;
    }

    if matches!(
        event.route_kind.as_str(),
        "empty_message" | "gate_blocked" | "system_command" | "local_command" | "vision_demo"
    ) {
        return false;
    }

    !matches!(
        event.command.as_deref().unwrap_or(""),
        "help_local"
            | "greeting_local"
            | "system_status"
            | "telegram_listener_start"
            | "telegram_listener_status"
            | "n8n_restart"
    )
}

fn backfilled_nl_status_from_launch_ops(event: &LaunchOpsEventRecord) -> &'static str {
    if event.outcome == "blocked" || event.route_kind == "gate_blocked" {
        return "blocked";
    }
    if event.command.as_deref() == Some("build_workflow") {
        return "approval_required";
    }
    if event.note.as_deref().is_some_and(|note| {
        note.contains("approval") || note.contains("승인") || note.contains("manual")
    }) {
        return if event.note.as_deref().unwrap_or_default().contains("manual") {
            "manual_required"
        } else {
            "approval_required"
        };
    }
    if event.outcome == "success" {
        "completed"
    } else {
        "error"
    }
}

pub fn sync_release_nl_runs_from_launch_ops(limit: i64) -> Result<i64> {
    let events = list_launch_ops_events(limit)?;
    let mut inserted = 0i64;

    for event in events.into_iter().rev() {
        if !should_backfill_launch_ops_event(&event) {
            continue;
        }

        let status = backfilled_nl_status_from_launch_ops(&event);
        let summary = Some(truncate_text(&event.message_preview, 160));
        let details = serde_json::json!({
            "source": "api.chat",
            "route_kind": event.route_kind,
            "command": event.command,
            "channel": event.channel,
            "memory_scope": event.memory_scope,
            "outcome": event.outcome,
            "confidence": event.confidence,
            "note": event.note,
            "flags": {
                "freshness_bypassed": event.freshness_bypassed,
                "intent_memory_hit": event.intent_memory_hit,
                "request_memory_hit": event.request_memory_hit,
                "execution_memory_hit": event.execution_memory_hit,
                "deterministic_used": event.deterministic_used,
                "llm_used": event.llm_used,
                "ai_digest_used": event.ai_digest_used,
                "local_only": event.local_only
            },
            "backfilled_from": "launch_ops_events"
        });
        let details_json = serde_json::to_string(&details).ok();
        let source_key = format!("launch_ops_event:{}", event.id);
        if insert_nl_run_with_source_key(
            Some(&event.created_at),
            Some(&source_key),
            event.command.as_deref().unwrap_or(&event.route_kind),
            &event.message_preview,
            status,
            summary.as_deref(),
            details_json.as_deref(),
        )? {
            inserted += 1;
        }
    }

    Ok(inserted)
}

fn aggregate_nl_runs<'a, I>(runs: I) -> NLRunMetrics
where
    I: IntoIterator<Item = &'a NLRun>,
{
    let mut total = 0i64;
    let mut completed = 0i64;
    let mut manual_required = 0i64;
    let mut approval_required = 0i64;
    let mut blocked = 0i64;
    let mut error = 0i64;

    for run in runs {
        total += 1;
        match run.status.as_str() {
            "completed" => completed += 1,
            "manual_required" => manual_required += 1,
            "approval_required" => approval_required += 1,
            "blocked" => blocked += 1,
            "error" => error += 1,
            _ => {}
        }
    }

    let success_rate = if total > 0 {
        (completed as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    NLRunMetrics {
        total,
        completed,
        manual_required,
        approval_required,
        blocked,
        error,
        success_rate,
    }
}

pub fn get_release_nl_run_metrics(limit: i64) -> Result<NLRunMetrics> {
    let runs = list_nl_runs(limit)?;
    let max_runs_per_prompt = std::env::var("RELEASE_NL_MAX_RUNS_PER_PROMPT")
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .unwrap_or(3)
        .max(1);
    let mut prompt_counts = HashMap::new();
    let mut filtered = Vec::new();

    for run in &runs {
        if is_release_nl_noise(run) {
            continue;
        }
        let prompt_key = normalize_release_nl_prompt(&run.prompt);
        if prompt_key.is_empty() {
            continue;
        }
        let entry = prompt_counts.entry(prompt_key).or_insert(0usize);
        if *entry >= max_runs_per_prompt {
            continue;
        }
        *entry += 1;
        filtered.push(run.clone());
    }

    Ok(aggregate_nl_runs(filtered.iter()))
}
