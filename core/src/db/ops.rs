use rusqlite::{params, Result};

use super::{get_db_lock, normalize_admin_limit, normalize_memory_scope, row_bool, truncate_text};

#[derive(Debug, Clone, serde::Serialize)]
pub struct MemoryOpsMetrics {
    pub request_active: i64,
    pub request_suppressed: i64,
    pub execution_active: i64,
    pub execution_suppressed: i64,
    pub last_request_used_at: Option<String>,
    pub last_execution_used_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LaunchOpsRouteBreakdown {
    pub route_kind: String,
    pub count: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LaunchOpsEventRecord {
    pub id: i64,
    pub created_at: String,
    pub channel: Option<String>,
    pub memory_scope: Option<String>,
    pub message_preview: String,
    pub route_kind: String,
    pub command: Option<String>,
    pub outcome: String,
    pub confidence: Option<f64>,
    pub freshness_bypassed: bool,
    pub intent_memory_hit: bool,
    pub request_memory_hit: bool,
    pub execution_memory_hit: bool,
    pub deterministic_used: bool,
    pub llm_used: bool,
    pub ai_digest_used: bool,
    pub local_only: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryAdminEventRecord {
    pub id: i64,
    pub created_at: String,
    pub kind: String,
    pub action: String,
    pub memory_scope: Option<String>,
    pub target_key: String,
    pub reason: Option<String>,
    pub actor: Option<String>,
    pub ok: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LaunchOpsMetrics {
    pub window_size: i64,
    pub total_requests: i64,
    pub blocked_requests: i64,
    pub intent_memory_hits: i64,
    pub request_memory_hits: i64,
    pub execution_memory_hits: i64,
    pub cached_response_hit_rate: f64,
    pub deterministic_routes: i64,
    pub llm_routes: i64,
    pub ai_digest_routes: i64,
    pub ai_digest_auto_routes: i64,
    pub local_routes: i64,
    pub freshness_bypasses: i64,
    pub low_confidence_routes: i64,
    pub unknown_routes: i64,
    pub error_routes: i64,
    pub last_event_at: Option<String>,
    pub route_breakdown: Vec<LaunchOpsRouteBreakdown>,
}

#[cfg(test)]
pub fn clear_launch_ops_events_for_tests() {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let _ = conn.execute("DELETE FROM launch_ops_events", []);
    }
}

#[cfg(test)]
pub fn clear_memory_admin_events_for_tests() {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let _ = conn.execute("DELETE FROM memory_admin_events", []);
    }
}

fn map_launch_ops_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LaunchOpsEventRecord> {
    Ok(LaunchOpsEventRecord {
        id: row.get(0)?,
        created_at: row.get(1)?,
        channel: row.get(2).ok(),
        memory_scope: row.get(3).ok(),
        message_preview: row.get(4)?,
        route_kind: row.get(5)?,
        command: row.get(6).ok(),
        outcome: row.get(7)?,
        confidence: row.get(8).ok(),
        freshness_bypassed: row_bool(row, 9, false),
        intent_memory_hit: row_bool(row, 10, false),
        request_memory_hit: row_bool(row, 11, false),
        execution_memory_hit: row_bool(row, 12, false),
        deterministic_used: row_bool(row, 13, false),
        llm_used: row_bool(row, 14, false),
        ai_digest_used: row_bool(row, 15, false),
        local_only: row_bool(row, 16, false),
        note: row.get(17).ok(),
    })
}

fn map_memory_admin_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryAdminEventRecord> {
    Ok(MemoryAdminEventRecord {
        id: row.get(0)?,
        created_at: row.get(1)?,
        kind: row.get(2)?,
        action: row.get(3)?,
        memory_scope: row.get(4).ok(),
        target_key: row.get(5)?,
        reason: row.get(6).ok(),
        actor: row.get(7).ok(),
        ok: row_bool(row, 8, false),
        message: row.get(9).ok(),
    })
}

pub fn get_memory_ops_metrics() -> Result<MemoryOpsMetrics> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let request_active: i64 = conn.query_row(
            "SELECT COUNT(*) FROM request_memory WHERE suppressed = 0",
            [],
            |row| row.get(0),
        )?;
        let request_suppressed: i64 = conn.query_row(
            "SELECT COUNT(*) FROM request_memory WHERE suppressed != 0",
            [],
            |row| row.get(0),
        )?;
        let execution_active: i64 = conn.query_row(
            "SELECT COUNT(*) FROM execution_memory WHERE suppressed = 0",
            [],
            |row| row.get(0),
        )?;
        let execution_suppressed: i64 = conn.query_row(
            "SELECT COUNT(*) FROM execution_memory WHERE suppressed != 0",
            [],
            |row| row.get(0),
        )?;
        let last_request_used_at = conn
            .query_row(
                "SELECT last_used_at FROM request_memory ORDER BY last_used_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();
        let last_execution_used_at = conn
            .query_row(
                "SELECT last_used_at FROM execution_memory ORDER BY last_used_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();
        return Ok(MemoryOpsMetrics {
            request_active,
            request_suppressed,
            execution_active,
            execution_suppressed,
            last_request_used_at,
            last_execution_used_at,
        });
    }
    Ok(MemoryOpsMetrics {
        request_active: 0,
        request_suppressed: 0,
        execution_active: 0,
        execution_suppressed: 0,
        last_request_used_at: None,
        last_execution_used_at: None,
    })
}

fn normalize_launch_ops_window_limit(limit: i64) -> i64 {
    limit.clamp(20, 1000)
}

fn normalize_launch_ops_list_limit(limit: i64) -> i64 {
    limit.clamp(1, 100)
}

pub fn record_launch_ops_event(
    channel: Option<&str>,
    memory_scope: Option<&str>,
    message_preview: &str,
    route_kind: &str,
    command: Option<&str>,
    outcome: &str,
    confidence: Option<f64>,
    freshness_bypassed: bool,
    intent_memory_hit: bool,
    request_memory_hit: bool,
    execution_memory_hit: bool,
    deterministic_used: bool,
    llm_used: bool,
    ai_digest_used: bool,
    local_only: bool,
    note: Option<&str>,
) -> Result<()> {
    let created_at = chrono::Utc::now().to_rfc3339();
    let preview = truncate_text(message_preview.trim(), 240);
    let route_kind = truncate_text(route_kind.trim(), 64);
    let outcome = truncate_text(outcome.trim(), 32);
    let command = command
        .map(|value| truncate_text(value.trim(), 64))
        .filter(|value| !value.is_empty());
    let channel = channel
        .map(|value| truncate_text(value.trim(), 32))
        .filter(|value| !value.is_empty());
    let memory_scope = memory_scope
        .map(|value| truncate_text(value.trim(), 160))
        .filter(|value| !value.is_empty());
    let note = note
        .map(|value| truncate_text(value.trim(), 500))
        .filter(|value| !value.is_empty());

    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "INSERT INTO launch_ops_events (
                created_at, channel, memory_scope, message_preview, route_kind, command, outcome,
                confidence, freshness_bypassed, intent_memory_hit, request_memory_hit,
                execution_memory_hit, deterministic_used, llm_used, ai_digest_used, local_only,
                note
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                created_at,
                channel,
                memory_scope,
                preview,
                route_kind,
                command,
                outcome,
                confidence,
                freshness_bypassed as i64,
                intent_memory_hit as i64,
                request_memory_hit as i64,
                execution_memory_hit as i64,
                deterministic_used as i64,
                llm_used as i64,
                ai_digest_used as i64,
                local_only as i64,
                note,
            ],
        )?;
    }
    Ok(())
}

pub fn list_launch_ops_events(limit: i64) -> Result<Vec<LaunchOpsEventRecord>> {
    let limit = normalize_launch_ops_list_limit(limit);
    let mut out = Vec::new();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, created_at, channel, memory_scope, message_preview, route_kind, command,
                    outcome, confidence, freshness_bypassed, intent_memory_hit, request_memory_hit,
                    execution_memory_hit, deterministic_used, llm_used, ai_digest_used, local_only,
                    note
             FROM launch_ops_events
             ORDER BY created_at DESC, id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], map_launch_ops_event_row)?;
        for row in rows.flatten() {
            out.push(row);
        }
    }
    Ok(out)
}

pub fn record_memory_admin_event(
    kind: &str,
    action: &str,
    memory_scope: Option<&str>,
    target_key: &str,
    reason: Option<&str>,
    actor: Option<&str>,
    ok: bool,
    message: Option<&str>,
) -> Result<()> {
    let created_at = chrono::Utc::now().to_rfc3339();
    let kind = truncate_text(kind.trim(), 32);
    let action = truncate_text(action.trim(), 32);
    if kind.is_empty() || action.is_empty() {
        return Ok(());
    }
    let target_key = {
        let trimmed = truncate_text(target_key.trim(), 255);
        if trimmed.is_empty() {
            "<empty>".to_string()
        } else {
            trimmed
        }
    };
    let memory_scope = memory_scope
        .map(|value| normalize_memory_scope(Some(value)))
        .filter(|value| !value.is_empty());
    let reason = reason
        .map(|value| truncate_text(value.trim(), 500))
        .filter(|value| !value.is_empty());
    let actor = actor
        .map(|value| truncate_text(value.trim(), 64))
        .filter(|value| !value.is_empty());
    let message = message
        .map(|value| truncate_text(value.trim(), 500))
        .filter(|value| !value.is_empty());

    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "INSERT INTO memory_admin_events (
                created_at, kind, action, memory_scope, target_key, reason, actor, ok, message
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                created_at,
                kind,
                action,
                memory_scope,
                target_key,
                reason,
                actor,
                if ok { 1 } else { 0 },
                message,
            ],
        )?;
    }
    Ok(())
}

pub fn list_memory_admin_events(limit: i64) -> Result<Vec<MemoryAdminEventRecord>> {
    let limit = normalize_admin_limit(limit);
    let mut out = Vec::new();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, created_at, kind, action, memory_scope, target_key, reason, actor, ok, message
             FROM memory_admin_events
             ORDER BY created_at DESC, id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], map_memory_admin_event_row)?;
        for row in rows.flatten() {
            out.push(row);
        }
    }
    Ok(out)
}

pub fn get_launch_ops_metrics(limit: i64) -> Result<LaunchOpsMetrics> {
    let limit = normalize_launch_ops_window_limit(limit);
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let metrics = conn.query_row(
            "SELECT
                COUNT(*) as total_requests,
                COALESCE(SUM(CASE WHEN route_kind = 'gate_blocked' THEN 1 ELSE 0 END), 0) as blocked_requests,
                COALESCE(SUM(intent_memory_hit), 0) as intent_memory_hits,
                COALESCE(SUM(request_memory_hit), 0) as request_memory_hits,
                COALESCE(SUM(execution_memory_hit), 0) as execution_memory_hits,
                COALESCE(SUM(deterministic_used), 0) as deterministic_routes,
                COALESCE(SUM(llm_used), 0) as llm_routes,
                COALESCE(SUM(ai_digest_used), 0) as ai_digest_routes,
                COALESCE(SUM(CASE WHEN route_kind = 'ai_digest_auto' THEN 1 ELSE 0 END), 0) as ai_digest_auto_routes,
                COALESCE(SUM(local_only), 0) as local_routes,
                COALESCE(SUM(freshness_bypassed), 0) as freshness_bypasses,
                COALESCE(SUM(CASE WHEN route_kind = 'low_confidence' THEN 1 ELSE 0 END), 0) as low_confidence_routes,
                COALESCE(SUM(CASE WHEN route_kind = 'unknown' THEN 1 ELSE 0 END), 0) as unknown_routes,
                COALESCE(SUM(CASE WHEN outcome = 'error' THEN 1 ELSE 0 END), 0) as error_routes,
                MAX(created_at) as last_event_at
             FROM (
                SELECT created_at, route_kind, outcome, freshness_bypassed, intent_memory_hit,
                       request_memory_hit, execution_memory_hit, deterministic_used, llm_used,
                       ai_digest_used, local_only
                FROM launch_ops_events
                ORDER BY created_at DESC, id DESC
                LIMIT ?1
             )",
            params![limit],
            |row| {
                let total_requests: i64 = row.get(0)?;
                let request_memory_hits: i64 = row.get(3)?;
                let execution_memory_hits: i64 = row.get(4)?;
                let cached_response_hit_rate = if total_requests > 0 {
                    ((request_memory_hits + execution_memory_hits) as f64 / total_requests as f64)
                        * 100.0
                } else {
                    0.0
                };
                Ok(LaunchOpsMetrics {
                    window_size: limit,
                    total_requests,
                    blocked_requests: row.get(1)?,
                    intent_memory_hits: row.get(2)?,
                    request_memory_hits,
                    execution_memory_hits,
                    cached_response_hit_rate,
                    deterministic_routes: row.get(5)?,
                    llm_routes: row.get(6)?,
                    ai_digest_routes: row.get(7)?,
                    ai_digest_auto_routes: row.get(8)?,
                    local_routes: row.get(9)?,
                    freshness_bypasses: row.get(10)?,
                    low_confidence_routes: row.get(11)?,
                    unknown_routes: row.get(12)?,
                    error_routes: row.get(13)?,
                    last_event_at: row.get(14).ok(),
                    route_breakdown: Vec::new(),
                })
            },
        )?;

        let mut breakdown_stmt = conn.prepare(
            "SELECT route_kind, COUNT(*) as count
             FROM (
                SELECT route_kind
                FROM launch_ops_events
                ORDER BY created_at DESC, id DESC
                LIMIT ?1
             )
             GROUP BY route_kind
             ORDER BY count DESC, route_kind ASC
             LIMIT 6",
        )?;
        let breakdown_rows = breakdown_stmt.query_map(params![limit], |row| {
            Ok(LaunchOpsRouteBreakdown {
                route_kind: row.get(0)?,
                count: row.get(1)?,
            })
        })?;

        let mut with_breakdown = metrics;
        for row in breakdown_rows.flatten() {
            with_breakdown.route_breakdown.push(row);
        }
        return Ok(with_breakdown);
    }

    Ok(LaunchOpsMetrics {
        window_size: limit,
        total_requests: 0,
        blocked_requests: 0,
        intent_memory_hits: 0,
        request_memory_hits: 0,
        execution_memory_hits: 0,
        cached_response_hit_rate: 0.0,
        deterministic_routes: 0,
        llm_routes: 0,
        ai_digest_routes: 0,
        ai_digest_auto_routes: 0,
        local_routes: 0,
        freshness_bypasses: 0,
        low_confidence_routes: 0,
        unknown_routes: 0,
        error_routes: 0,
        last_event_at: None,
        route_breakdown: Vec::new(),
    })
}
