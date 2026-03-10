use rusqlite::{params, Result};

use crate::quality_scorer::QualityScore;

use super::{with_read_conn, with_write_conn_if_available};

#[derive(Debug, Clone, serde::Serialize)]
pub struct QualityScoreRecord {
    pub created_at: String,
    pub overall: f64,
    pub breakdown: serde_json::Value,
    pub issues: Vec<String>,
    pub strengths: Vec<String>,
    pub recommendation: String,
    pub summary: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct JudgmentState {
    pub last_hash: Option<String>,
    pub consecutive_no_progress: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReleaseBaselineRecord {
    pub created_at: String,
    pub baseline_json: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VerificationRun {
    pub id: i64,
    pub created_at: String,
    pub kind: String,
    pub ok: bool,
    pub summary: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RoutineRun {
    pub id: i64,
    pub routine_id: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub error: Option<String>,
}

pub fn insert_quality_score(score: &QualityScore) -> Result<()> {
    with_write_conn_if_available(|conn| {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO quality_scores (created_at, overall, breakdown, issues, strengths, recommendation, summary)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                created_at,
                score.overall,
                serde_json::to_string(&score.breakdown).unwrap_or_else(|_| "{}".to_string()),
                serde_json::to_string(&score.issues).unwrap_or_else(|_| "[]".to_string()),
                serde_json::to_string(&score.strengths).unwrap_or_else(|_| "[]".to_string()),
                score.recommendation,
                score.summary
            ],
        )?;
        Ok(())
    })?;
    Ok(())
}

pub fn get_latest_quality_score() -> Result<Option<QualityScoreRecord>> {
    with_read_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT created_at, overall, breakdown, issues, strengths, recommendation, summary
             FROM quality_scores
             ORDER BY created_at DESC
             LIMIT 1",
        )?;
        let row = stmt.query_row([], |row| {
            let breakdown_str: String = row.get(2)?;
            let issues_str: String = row.get(3)?;
            let strengths_str: String = row.get(4)?;
            Ok(QualityScoreRecord {
                created_at: row.get(0)?,
                overall: row.get(1)?,
                breakdown: serde_json::from_str(&breakdown_str)
                    .unwrap_or_else(|_| serde_json::json!({})),
                issues: serde_json::from_str(&issues_str).unwrap_or_else(|_| Vec::new()),
                strengths: serde_json::from_str(&strengths_str).unwrap_or_else(|_| Vec::new()),
                recommendation: row.get(5)?,
                summary: row.get(6)?,
            })
        });
        return match row {
            Ok(record) => Ok(Some(record)),
            Err(_) => Ok(None),
        };
    })
}

pub fn get_judgment_state() -> Result<Option<JudgmentState>> {
    with_read_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT last_hash, consecutive_no_progress, updated_at
             FROM judgment_states
             WHERE id = 1",
        )?;
        let row = stmt.query_row([], |row| {
            Ok(JudgmentState {
                last_hash: row.get(0)?,
                consecutive_no_progress: row.get(1)?,
                updated_at: row.get(2)?,
            })
        });
        return match row {
            Ok(state) => Ok(Some(state)),
            Err(_) => Ok(None),
        };
    })
}

pub fn upsert_judgment_state(last_hash: Option<&str>, consecutive_no_progress: i64) -> Result<()> {
    with_write_conn_if_available(|conn| {
        let updated_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO judgment_states (id, last_hash, consecutive_no_progress, updated_at)
             VALUES (1, ?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET
                last_hash = excluded.last_hash,
                consecutive_no_progress = excluded.consecutive_no_progress,
                updated_at = excluded.updated_at",
            params![last_hash, consecutive_no_progress, updated_at],
        )?;
        Ok(())
    })?;
    Ok(())
}

pub fn get_release_baseline_json() -> Result<Option<ReleaseBaselineRecord>> {
    with_read_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT created_at, baseline_json
             FROM release_baseline
             WHERE id = 1",
        )?;
        let row = stmt.query_row([], |row| {
            Ok(ReleaseBaselineRecord {
                created_at: row.get(0)?,
                baseline_json: row.get(1)?,
            })
        });
        return match row {
            Ok(record) => Ok(Some(record)),
            Err(_) => Ok(None),
        };
    })
}

pub fn upsert_release_baseline_json(created_at: &str, baseline_json: &str) -> Result<()> {
    with_write_conn_if_available(|conn| {
        conn.execute(
            "INSERT INTO release_baseline (id, created_at, baseline_json)
             VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET
                created_at = excluded.created_at,
                baseline_json = excluded.baseline_json",
            params![created_at, baseline_json],
        )?;
        Ok(())
    })?;
    Ok(())
}

pub fn insert_verification_run(
    kind: &str,
    ok: bool,
    summary: &str,
    details: Option<&str>,
) -> Result<()> {
    with_write_conn_if_available(|conn| {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO verification_runs (created_at, kind, ok, summary, details)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![created_at, kind, ok, summary, details],
        )?;
        Ok(())
    })?;
    Ok(())
}

pub fn list_verification_runs(limit: i64) -> Result<Vec<VerificationRun>> {
    with_read_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, created_at, kind, ok, summary, details
             FROM verification_runs
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(VerificationRun {
                id: row.get(0)?,
                created_at: row.get(1)?,
                kind: row.get(2)?,
                ok: row.get::<_, i64>(3)? != 0,
                summary: row.get(4)?,
                details: row.get(5).ok(),
            })
        })?;
        let mut runs = Vec::new();
        for r in rows {
            runs.push(r?);
        }
        Ok(runs)
    })
}

pub fn create_routine_run(routine_id: i64) -> Result<i64> {
    if let Some(id) = with_write_conn_if_available(|conn| {
        let started_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO routine_runs (routine_id, started_at, status) VALUES (?1, ?2, 'running')",
            params![routine_id, started_at],
        )?;
        Ok(conn.last_insert_rowid())
    })? {
        return Ok(id);
    }
    Ok(0)
}

pub fn finish_routine_run(run_id: i64, status: &str, error: Option<&str>) -> Result<()> {
    with_write_conn_if_available(|conn| {
        let finished_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE routine_runs SET status = ?1, error = ?2, finished_at = ?3 WHERE id = ?4",
            params![status, error, finished_at, run_id],
        )?;
        Ok(())
    })?;
    Ok(())
}

pub fn list_routine_runs(limit: i64) -> Result<Vec<RoutineRun>> {
    with_read_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, routine_id, started_at, finished_at, status, error
             FROM routine_runs ORDER BY started_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |row| {
            Ok(RoutineRun {
                id: row.get(0)?,
                routine_id: row.get(1)?,
                started_at: row.get(2)?,
                finished_at: row.get(3).ok(),
                status: row.get(4)?,
                error: row.get(5).ok(),
            })
        })?;

        let mut runs = Vec::new();
        for r in rows {
            runs.push(r?);
        }
        Ok(runs)
    })
}
