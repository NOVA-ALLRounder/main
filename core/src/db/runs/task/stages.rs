use rusqlite::{params, params_from_iter, types::Value, Connection, Result};

use super::super::super::{with_read_conn, with_write_conn_if_available};
use super::{TaskRunArtifactRecord, TaskStageAssertionRecord, TaskStageRunRecord};

#[derive(Debug, Clone, Default)]
pub struct TaskStageAssertionListOptions {
    pub stage_name: Option<String>,
    pub failed_only: bool,
    pub limit: Option<i64>,
    pub offset: i64,
}

#[derive(Debug, Clone, Default)]
pub struct TaskRunArtifactListOptions {
    pub artifact_type: Option<String>,
    pub limit: Option<i64>,
    pub offset: i64,
}

fn parse_stage_max_retries() -> i64 {
    std::env::var("STEER_STAGE_MAX_RETRIES")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .map(|value| value.clamp(0, 16))
        .unwrap_or(2)
}

fn parse_stage_retry_backoff_base_seconds() -> i64 {
    std::env::var("STEER_STAGE_RETRY_BACKOFF_BASE_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .map(|value| value.clamp(1, 120))
        .unwrap_or(5)
}
fn canonical_stage_status(raw: &str) -> String {
    let normalized = raw.trim().to_lowercase();
    match normalized.as_str() {
        "" | "queued" => "pending".to_string(),
        "pending" => "pending".to_string(),
        "running" | "in_progress" | "in-progress" | "started" => "running".to_string(),
        "retry" | "retrying" => "retrying".to_string(),
        "completed" | "success" | "ok" | "done" => "completed".to_string(),
        "failed" | "error" => "failed".to_string(),
        "blocked" | "manual_required" | "approval_required" => "blocked".to_string(),
        other => other.to_string(),
    }
}

fn is_known_stage_status(status: &str) -> bool {
    matches!(
        status,
        "pending" | "running" | "retrying" | "completed" | "failed" | "blocked"
    )
}

fn stage_transition_allowed(prev: &str, next: &str) -> bool {
    if prev == next {
        return true;
    }
    if !is_known_stage_status(prev) || !is_known_stage_status(next) {
        return true;
    }
    matches!(
        (prev, next),
        ("pending", "running")
            | ("pending", "retrying")
            | ("pending", "completed")
            | ("pending", "failed")
            | ("pending", "blocked")
            | ("running", "retrying")
            | ("running", "completed")
            | ("running", "failed")
            | ("running", "blocked")
            | ("retrying", "running")
            | ("retrying", "completed")
            | ("retrying", "failed")
            | ("retrying", "blocked")
            | ("failed", "retrying")
            | ("failed", "running")
            | ("blocked", "retrying")
            | ("blocked", "running")
    )
}

pub fn record_task_stage_run(
    run_id: &str,
    stage_name: &str,
    stage_order: i64,
    status: &str,
    details: Option<&str>,
) -> Result<()> {
    with_write_conn_if_available(|conn| {
        let now = chrono::Utc::now().to_rfc3339();
        let status = canonical_stage_status(status);
        let details_clean = details.map(str::trim).filter(|value| !value.is_empty());
        let stage_max_retries_default = parse_stage_max_retries();
        let stage_backoff_base = parse_stage_retry_backoff_base_seconds();

        let mut stmt = conn.prepare(
            "SELECT status, started_at, details, retry_count, max_retries
             FROM task_stage_runs
             WHERE run_id = ?1
               AND stage_name = ?2
               AND stage_order = ?3
             ORDER BY id DESC
             LIMIT 1",
        )?;
        let previous = stmt.query_row(params![run_id, stage_name, stage_order], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2).ok().flatten(),
                row.get::<_, i64>(3).unwrap_or(0),
                row.get::<_, i64>(4).unwrap_or(0),
            ))
        });

        let mut started_at = now.clone();
        let mut retry_count: i64 = 0;
        let mut max_retries: i64 = stage_max_retries_default;
        let mut next_retry_at: Option<String> = None;
        if let Ok((
            prev_status_raw,
            prev_started_at,
            prev_details,
            prev_retry_count,
            prev_max_retries,
        )) = previous
        {
            let prev_status = canonical_stage_status(&prev_status_raw);
            let prev_detail_text = prev_details.unwrap_or_default();
            let next_detail_text = details_clean.unwrap_or("");
            retry_count = prev_retry_count.max(0);
            if prev_max_retries > 0 {
                max_retries = prev_max_retries;
            }
            if prev_status == status && prev_detail_text.trim() == next_detail_text {
                return Ok(());
            }
            if !stage_transition_allowed(prev_status.as_str(), status.as_str()) {
                eprintln!(
                    "⚠️ Invalid stage transition ignored: run_id={} stage={} order={} {} -> {}",
                    run_id, stage_name, stage_order, prev_status, status
                );
                return Err(rusqlite::Error::InvalidQuery);
            }
            let restart_attempt = matches!(
                (prev_status.as_str(), status.as_str()),
                ("failed", "running")
                    | ("failed", "retrying")
                    | ("blocked", "running")
                    | ("blocked", "retrying")
            );
            started_at = if restart_attempt {
                now.clone()
            } else {
                prev_started_at
            };
        }

        if status == "retrying" {
            retry_count += 1;
            let shift = (retry_count.saturating_sub(1)).clamp(0, 6) as u32;
            let multiplier = 1_i64.checked_shl(shift).unwrap_or(64);
            let backoff_secs = (stage_backoff_base * multiplier).clamp(1, 600);
            next_retry_at =
                Some((chrono::Utc::now() + chrono::Duration::seconds(backoff_secs)).to_rfc3339());
        }

        conn.execute(
            "INSERT INTO task_stage_runs (
                run_id, stage_name, stage_order, status, started_at, finished_at, details,
                retry_count, max_retries, next_retry_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                run_id,
                stage_name,
                stage_order,
                status,
                started_at,
                now,
                details_clean,
                retry_count,
                max_retries,
                next_retry_at
            ],
        )?;
        Ok(())
    })?;
    Ok(())
}

pub fn record_task_stage_assertion(
    run_id: &str,
    stage_name: &str,
    assertion_key: &str,
    expected: &str,
    actual: &str,
    passed: bool,
    evidence: Option<&str>,
) -> Result<()> {
    with_write_conn_if_available(|conn| {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO task_stage_assertions (
                run_id, stage_name, assertion_key, expected, actual, passed, evidence, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run_id,
                stage_name,
                assertion_key,
                expected,
                actual,
                passed as i64,
                evidence,
                created_at
            ],
        )?;
        Ok(())
    })?;
    Ok(())
}

pub fn upsert_task_run_artifact(
    run_id: &str,
    artifact_type: &str,
    artifact_key: &str,
    value: &str,
    metadata: Option<&str>,
) -> Result<()> {
    let run_id = run_id.trim();
    let artifact_type = artifact_type.trim();
    let artifact_key = artifact_key.trim();
    if run_id.is_empty() || artifact_type.is_empty() || artifact_key.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "run_id/artifact_type/artifact_key must not be empty".to_string(),
        ));
    }

    with_write_conn_if_available(|conn| {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO task_run_artifacts (
                run_id, artifact_type, artifact_key, value, metadata, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(run_id, artifact_type, artifact_key) DO UPDATE SET
                value = excluded.value,
                metadata = excluded.metadata,
                created_at = excluded.created_at",
            params![
                run_id,
                artifact_type,
                artifact_key,
                value,
                metadata,
                created_at
            ],
        )?;
        Ok(())
    })?;
    Ok(())
}

fn normalized_limit(limit: Option<i64>) -> Option<i64> {
    limit.and_then(|value| {
        if value > 0 {
            Some(value.min(500))
        } else {
            None
        }
    })
}

fn normalized_offset(offset: i64) -> i64 {
    offset.max(0)
}

fn query_task_stage_runs(conn: &Connection, run_id: &str) -> Result<Vec<TaskStageRunRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, run_id, stage_name, stage_order, status, started_at, finished_at, details,
                retry_count, max_retries, next_retry_at,
                COALESCE((
                    SELECT COUNT(*)
                    FROM task_stage_assertions AS assertions
                    WHERE assertions.run_id = task_stage_runs.run_id
                      AND assertions.stage_name = task_stage_runs.stage_name
                ), 0) AS assertion_total,
                COALESCE((
                    SELECT COUNT(*)
                    FROM task_stage_assertions AS assertions
                    WHERE assertions.run_id = task_stage_runs.run_id
                      AND assertions.stage_name = task_stage_runs.stage_name
                      AND assertions.passed = 0
                ), 0) AS assertion_failed
         FROM task_stage_runs
         WHERE run_id = ?1
         ORDER BY stage_order ASC, id ASC",
    )?;

    let rows = stmt.query_map(params![run_id], |row| {
        Ok(TaskStageRunRecord {
            id: row.get(0)?,
            run_id: row.get(1)?,
            stage_name: row.get(2)?,
            stage_order: row.get(3)?,
            status: row.get(4)?,
            started_at: row.get(5)?,
            finished_at: row.get(6)?,
            details: row.get(7).ok(),
            retry_count: row.get::<_, i64>(8).unwrap_or(0),
            max_retries: row.get::<_, i64>(9).unwrap_or(0),
            next_retry_at: row.get(10).ok(),
            assertion_total: row.get::<_, i64>(11).unwrap_or(0),
            assertion_failed: row.get::<_, i64>(12).unwrap_or(0),
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn query_task_stage_assertions(
    conn: &Connection,
    run_id: &str,
    options: &TaskStageAssertionListOptions,
) -> Result<Vec<TaskStageAssertionRecord>> {
    let mut sql = String::from(
        "SELECT id, run_id, stage_name, assertion_key, expected, actual, passed, evidence, created_at
         FROM task_stage_assertions
         WHERE run_id = ?",
    );
    let mut params: Vec<Value> = vec![run_id.to_string().into()];

    if let Some(stage_name) = options.stage_name.as_ref().map(|value| value.trim()) {
        if !stage_name.is_empty() {
            sql.push_str(" AND stage_name = ?");
            params.push(stage_name.to_string().into());
        }
    }
    if options.failed_only {
        sql.push_str(" AND passed = 0");
    }
    sql.push_str(" ORDER BY id ASC");

    if let Some(limit) = normalized_limit(options.limit) {
        sql.push_str(" LIMIT ?");
        params.push(limit.into());
        let offset = normalized_offset(options.offset);
        if offset > 0 {
            sql.push_str(" OFFSET ?");
            params.push(offset.into());
        }
    }

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(params.iter()), |row| {
        Ok(TaskStageAssertionRecord {
            id: row.get(0)?,
            run_id: row.get(1)?,
            stage_name: row.get(2)?,
            assertion_key: row.get(3)?,
            expected: row.get(4)?,
            actual: row.get(5)?,
            passed: row.get::<_, i64>(6)? != 0,
            evidence: row.get(7).ok(),
            created_at: row.get(8)?,
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn query_task_run_artifacts(
    conn: &Connection,
    run_id: &str,
    options: &TaskRunArtifactListOptions,
) -> Result<Vec<TaskRunArtifactRecord>> {
    let mut sql = String::from(
        "SELECT id, run_id, artifact_type, artifact_key, value, metadata, created_at
         FROM task_run_artifacts
         WHERE run_id = ?",
    );
    let mut params: Vec<Value> = vec![run_id.to_string().into()];

    if let Some(artifact_type) = options.artifact_type.as_ref().map(|value| value.trim()) {
        if !artifact_type.is_empty() {
            sql.push_str(" AND artifact_type = ?");
            params.push(artifact_type.to_string().into());
        }
    }

    sql.push_str(" ORDER BY created_at DESC, id DESC");

    if let Some(limit) = normalized_limit(options.limit) {
        sql.push_str(" LIMIT ?");
        params.push(limit.into());
        let offset = normalized_offset(options.offset);
        if offset > 0 {
            sql.push_str(" OFFSET ?");
            params.push(offset.into());
        }
    }

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(params.iter()), |row| {
        Ok(TaskRunArtifactRecord {
            id: row.get(0)?,
            run_id: row.get(1)?,
            artifact_type: row.get(2)?,
            artifact_key: row.get(3)?,
            value: row.get(4)?,
            metadata: row.get(5).ok(),
            created_at: row.get(6)?,
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn list_task_stage_runs(run_id: &str) -> Result<Vec<TaskStageRunRecord>> {
    match with_read_conn(|conn| query_task_stage_runs(conn, run_id)) {
        Ok(rows) => Ok(rows),
        Err(rusqlite::Error::InvalidQuery) => Ok(Vec::new()),
        Err(err) => Err(err),
    }
}

pub fn list_task_stage_assertions(run_id: &str) -> Result<Vec<TaskStageAssertionRecord>> {
    list_task_stage_assertions_with_options(run_id, &TaskStageAssertionListOptions::default())
}

pub fn list_task_stage_assertions_with_options(
    run_id: &str,
    options: &TaskStageAssertionListOptions,
) -> Result<Vec<TaskStageAssertionRecord>> {
    match with_read_conn(|conn| query_task_stage_assertions(conn, run_id, options)) {
        Ok(rows) => Ok(rows),
        Err(rusqlite::Error::InvalidQuery) => Ok(Vec::new()),
        Err(err) => Err(err),
    }
}

pub fn list_task_run_artifacts(run_id: &str) -> Result<Vec<TaskRunArtifactRecord>> {
    list_task_run_artifacts_with_options(run_id, &TaskRunArtifactListOptions::default())
}

pub fn list_task_run_artifacts_with_options(
    run_id: &str,
    options: &TaskRunArtifactListOptions,
) -> Result<Vec<TaskRunArtifactRecord>> {
    match with_read_conn(|conn| query_task_run_artifacts(conn, run_id, options)) {
        Ok(rows) => Ok(rows),
        Err(rusqlite::Error::InvalidQuery) => Ok(Vec::new()),
        Err(err) => Err(err),
    }
}
