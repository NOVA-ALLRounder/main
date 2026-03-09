use rusqlite::{params, Result};

use super::super::super::get_db_lock;
use super::TaskRunRecord;
pub fn create_task_run(run_id: &str, intent: &str, prompt: &str, status: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO task_runs (
                run_id, plan_id, created_at, finished_at, intent, prompt,
                planner_complete, execution_complete, business_complete,
                status, summary, details
             ) VALUES (?1, NULL, ?2, NULL, ?3, ?4, 0, 0, 0, ?5, NULL, NULL)",
            params![run_id, created_at, intent, prompt, status],
        )?;
    }
    Ok(())
}

fn parse_task_run_stale_minutes() -> i64 {
    std::env::var("STEER_TASK_RUN_STALE_MINUTES")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value >= 1)
        .unwrap_or(120)
}

pub fn mark_stale_running_task_runs_finished() -> Result<usize> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let stale_window = format!("-{} minutes", parse_task_run_stale_minutes());
        let finished_at = chrono::Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE task_runs
             SET status = 'business_failed',
                 finished_at = COALESCE(finished_at, ?1),
                 summary = COALESCE(summary, 'auto-finalized stale running run'),
                 details = COALESCE(details, '{\"source\":\"db.mark_stale_running_task_runs_finished\",\"reason\":\"stale_running_run\"}')
             WHERE status = 'running'
               AND finished_at IS NULL
               AND julianday(created_at) < julianday('now', ?2)",
            params![finished_at, stale_window],
        )?;
        return Ok(rows);
    }
    Ok(0)
}

pub fn mark_orphaned_inflight_task_runs_failed() -> Result<usize> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let finished_at = chrono::Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE task_runs
             SET status = 'business_failed',
                 finished_at = COALESCE(finished_at, ?1),
                 summary = COALESCE(summary, 'auto-finalized orphaned in-flight run after core restart'),
                 details = COALESCE(details, '{\"source\":\"db.mark_orphaned_inflight_task_runs_failed\",\"reason\":\"orphaned_inflight_run\"}')
             WHERE finished_at IS NULL
               AND lower(status) IN ('queued', 'running', 'accepted', 'started', 'retrying', 'business_incomplete')",
            params![finished_at],
        )?;
        return Ok(rows);
    }
    Ok(0)
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

pub fn claim_task_run(
    plan_id: &str,
    run_id: &str,
    intent: &str,
    prompt: &str,
    status: &str,
) -> Result<bool> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        let stale_window = format!("-{} minutes", parse_task_run_stale_minutes());
        let tx = conn.transaction()?;
        let inflight_count: i64 = tx.query_row(
            "SELECT COUNT(1)
             FROM task_runs
             WHERE plan_id = ?1
               AND status = 'running'
               AND (
                 finished_at IS NULL
                 OR julianday(created_at) >= julianday('now', ?2)
               )",
            params![plan_id, stale_window],
            |row| row.get(0),
        )?;
        if inflight_count > 0 {
            tx.rollback()?;
            return Ok(false);
        }
        tx.execute(
            "INSERT INTO task_runs (
                run_id, plan_id, created_at, finished_at, intent, prompt,
                planner_complete, execution_complete, business_complete,
                status, summary, details
             ) VALUES (?1, ?2, ?3, NULL, ?4, ?5, 0, 0, 0, ?6, NULL, NULL)",
            params![run_id, plan_id, created_at, intent, prompt, status],
        )?;
        tx.commit()?;
    }
    Ok(true)
}

pub fn update_task_run_outcome(
    run_id: &str,
    planner_complete: bool,
    execution_complete: bool,
    business_complete: bool,
    status: &str,
    summary: Option<&str>,
    details: Option<&str>,
) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let finished_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE task_runs
             SET finished_at = ?1,
                 planner_complete = ?2,
                 execution_complete = ?3,
                 business_complete = ?4,
                 status = ?5,
                 summary = ?6,
                 details = ?7
             WHERE run_id = ?8",
            params![
                finished_at,
                planner_complete as i64,
                execution_complete as i64,
                business_complete as i64,
                status,
                summary,
                details,
                run_id
            ],
        )?;
    }
    Ok(())
}

pub fn get_task_run(run_id: &str) -> Result<Option<TaskRunRecord>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT run_id, created_at, finished_at, intent, prompt,
                    planner_complete, execution_complete, business_complete,
                    status, summary, details
             FROM task_runs
             WHERE run_id = ?1
             LIMIT 1",
        )?;

        let mut rows = stmt.query(params![run_id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(TaskRunRecord {
                run_id: row.get(0)?,
                created_at: row.get(1)?,
                finished_at: row.get(2).ok(),
                intent: row.get(3)?,
                prompt: row.get(4)?,
                planner_complete: row.get::<_, i64>(5)? != 0,
                execution_complete: row.get::<_, i64>(6)? != 0,
                business_complete: row.get::<_, i64>(7)? != 0,
                status: row.get(8)?,
                summary: row.get(9).ok(),
                details: row.get(10).ok(),
            }));
        }
    }
    Ok(None)
}

pub fn list_task_runs(limit: i64, status: Option<&str>) -> Result<Vec<TaskRunRecord>> {
    let bounded_limit = limit.clamp(1, 500);
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let normalized_status = status
            .map(|value| value.trim())
            .filter(|value| !value.is_empty());
        let mut out = Vec::new();

        if let Some(status_filter) = normalized_status {
            let mut stmt = conn.prepare(
                "SELECT run_id, created_at, finished_at, intent, prompt,
                        planner_complete, execution_complete, business_complete,
                        status, summary, details
                 FROM task_runs
                 WHERE status = ?1
                 ORDER BY created_at DESC
                 LIMIT ?2",
            )?;

            let rows = stmt.query_map(params![status_filter, bounded_limit], |row| {
                Ok(TaskRunRecord {
                    run_id: row.get(0)?,
                    created_at: row.get(1)?,
                    finished_at: row.get(2).ok(),
                    intent: row.get(3)?,
                    prompt: row.get(4)?,
                    planner_complete: row.get::<_, i64>(5)? != 0,
                    execution_complete: row.get::<_, i64>(6)? != 0,
                    business_complete: row.get::<_, i64>(7)? != 0,
                    status: row.get(8)?,
                    summary: row.get(9).ok(),
                    details: row.get(10).ok(),
                })
            })?;

            for row in rows {
                out.push(row?);
            }
            return Ok(out);
        }

        let mut stmt = conn.prepare(
            "SELECT run_id, created_at, finished_at, intent, prompt,
                    planner_complete, execution_complete, business_complete,
                    status, summary, details
             FROM task_runs
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![bounded_limit], |row| {
            Ok(TaskRunRecord {
                run_id: row.get(0)?,
                created_at: row.get(1)?,
                finished_at: row.get(2).ok(),
                intent: row.get(3)?,
                prompt: row.get(4)?,
                planner_complete: row.get::<_, i64>(5)? != 0,
                execution_complete: row.get::<_, i64>(6)? != 0,
                business_complete: row.get::<_, i64>(7)? != 0,
                status: row.get(8)?,
                summary: row.get(9).ok(),
                details: row.get(10).ok(),
            })
        })?;

        for row in rows {
            out.push(row?);
        }
        return Ok(out);
    }
    Ok(Vec::new())
}

pub fn get_latest_inflight_task_run() -> Result<Option<TaskRunRecord>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT run_id, created_at, finished_at, intent, prompt,
                    planner_complete, execution_complete, business_complete,
                    status, summary, details
             FROM task_runs
             WHERE finished_at IS NULL
               AND lower(status) IN ('queued', 'running', 'accepted', 'started', 'retrying', 'business_incomplete')
             ORDER BY created_at DESC
             LIMIT 1",
        )?;

        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(TaskRunRecord {
                run_id: row.get(0)?,
                created_at: row.get(1)?,
                finished_at: row.get(2).ok(),
                intent: row.get(3)?,
                prompt: row.get(4)?,
                planner_complete: row.get::<_, i64>(5)? != 0,
                execution_complete: row.get::<_, i64>(6)? != 0,
                business_complete: row.get::<_, i64>(7)? != 0,
                status: row.get(8)?,
                summary: row.get(9).ok(),
                details: row.get(10).ok(),
            }));
        }
    }
    Ok(None)
}
