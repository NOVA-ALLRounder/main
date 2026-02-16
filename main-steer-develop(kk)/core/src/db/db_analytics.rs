use super::*;

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
pub struct NLRun {
    pub id: i64,
    pub created_at: String,
    pub intent: String,
    pub prompt: String,
    pub status: String,
    pub summary: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NLRunMetrics {
    pub total: i64,
    pub completed: i64,
    pub manual_required: i64,
    pub approval_required: i64,
    pub blocked: i64,
    pub error: i64,
    pub success_rate: f64,
}

pub fn insert_quality_score(score: &QualityScore) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
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
    }
    Ok(())
}

pub fn get_latest_quality_score() -> Result<Option<QualityScoreRecord>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
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
    }
    Ok(None)
}

pub fn get_judgment_state() -> Result<Option<JudgmentState>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
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
    }
    Ok(None)
}

pub fn upsert_judgment_state(last_hash: Option<&str>, consecutive_no_progress: i64) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
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
    }
    Ok(())
}

pub fn get_release_baseline_json() -> Result<Option<ReleaseBaselineRecord>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
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
    }
    Ok(None)
}

pub fn upsert_release_baseline_json(created_at: &str, baseline_json: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "INSERT INTO release_baseline (id, created_at, baseline_json)
             VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET
                created_at = excluded.created_at,
                baseline_json = excluded.baseline_json",
            params![created_at, baseline_json],
        )?;
    }
    Ok(())
}

pub fn insert_verification_run(
    kind: &str,
    ok: bool,
    summary: &str,
    details: Option<&str>,
) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO verification_runs (created_at, kind, ok, summary, details)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![created_at, kind, ok, summary, details],
        )?;
    }
    Ok(())
}

pub fn list_verification_runs(limit: i64) -> Result<Vec<VerificationRun>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
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
        return Ok(runs);
    }
    Ok(Vec::new())
}

pub fn insert_nl_run(
    intent: &str,
    prompt: &str,
    status: &str,
    summary: Option<&str>,
    details: Option<&str>,
) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO nl_runs (created_at, intent, prompt, status, summary, details)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![created_at, intent, prompt, status, summary, details],
        )?;
    }
    Ok(())
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
        for r in rows {
            runs.push(r?);
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
