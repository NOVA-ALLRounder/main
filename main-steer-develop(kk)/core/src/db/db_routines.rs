use super::*;
use std::str::FromStr;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Routine {
    pub id: i64,
    pub name: String,
    pub cron_expression: String,
    pub prompt: String,
    pub enabled: bool,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
    pub created_at: String,
}

pub fn create_routine(name: &str, cron: &str, prompt: &str) -> Result<i64> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        let next_run = match cron::Schedule::from_str(cron) {
            Ok(s) => s
                .upcoming(chrono::Utc)
                .next()
                .map(|d: chrono::DateTime<chrono::Utc>| d.to_rfc3339()),
            Err(_) => None,
        };

        conn.execute(
            "INSERT INTO routines (name, cron_expression, prompt, created_at, next_run) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![name, cron, prompt, created_at, next_run],
        )?;
        Ok(conn.last_insert_rowid())
    } else {
        Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(1),
            Some("DB not initialized".to_string()),
        ))
    }
}

pub fn get_due_routines() -> Result<Vec<Routine>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = conn.prepare("SELECT id, name, cron_expression, prompt, enabled, last_run, next_run, created_at FROM routines WHERE enabled = 1 AND next_run <= ?1")?;
        let rows = stmt.query_map(params![now], |row| {
            Ok(Routine {
                id: row.get(0)?,
                name: row.get(1)?,
                cron_expression: row.get(2)?,
                prompt: row.get(3)?,
                enabled: row.get(4)?,
                last_run: row.get(5)?,
                next_run: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;

        let mut routines = Vec::new();
        for routine in rows {
            routines.push(routine?);
        }
        Ok(routines)
    } else {
        Ok(Vec::new())
    }
}

pub fn update_routine_execution(id: i64, next: Option<String>) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE routines SET last_run = ?1, next_run = ?2 WHERE id = ?3",
            params![now, next, id],
        )?;
        Ok(())
    } else {
        Ok(())
    }
}

pub fn get_active_routines() -> Result<Vec<Routine>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare("SELECT id, name, cron_expression, prompt, enabled, last_run, next_run, created_at FROM routines WHERE enabled = 1")?;
        let rows = stmt.query_map([], |row| {
            Ok(Routine {
                id: row.get(0)?,
                name: row.get(1)?,
                cron_expression: row.get(2)?,
                prompt: row.get(3)?,
                enabled: row.get(4)?,
                last_run: row.get(5)?,
                next_run: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        let mut routines = Vec::new();
        for routine in rows {
            routines.push(routine?);
        }
        Ok(routines)
    } else {
        Ok(Vec::new())
    }
}

pub fn get_all_routines() -> Result<Vec<Routine>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare("SELECT id, name, cron_expression, prompt, enabled, last_run, next_run, created_at FROM routines ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| {
            Ok(Routine {
                id: row.get(0)?,
                name: row.get(1)?,
                cron_expression: row.get(2)?,
                prompt: row.get(3)?,
                enabled: row.get(4)?,
                last_run: row.get(5)?,
                next_run: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        let mut routines = Vec::new();
        for routine in rows {
            routines.push(routine?);
        }
        Ok(routines)
    } else {
        Ok(Vec::new())
    }
}

pub fn toggle_routine(id: i64, enabled: bool) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "UPDATE routines SET enabled = ?1 WHERE id = ?2",
            params![enabled, id],
        )?;
        Ok(())
    } else {
        Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(1),
            Some("DB not initialized".to_string()),
        ))
    }
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

pub fn create_routine_run(routine_id: i64) -> Result<i64> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let started_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO routine_runs (routine_id, started_at, status) VALUES (?1, ?2, 'running')",
            params![routine_id, started_at],
        )?;
        return Ok(conn.last_insert_rowid());
    }
    Ok(0)
}

pub fn finish_routine_run(run_id: i64, status: &str, error: Option<&str>) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let finished_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE routine_runs SET status = ?1, error = ?2, finished_at = ?3 WHERE id = ?4",
            params![status, error, finished_at, run_id],
        )?;
    }
    Ok(())
}

pub fn list_routine_runs(limit: i64) -> Result<Vec<RoutineRun>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
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
    } else {
        Ok(Vec::new())
    }
}

pub fn insert_routine_candidate(pattern: &crate::pattern_detector::DetectedPattern) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        let samples_json = serde_json::to_string(&pattern.sample_events).unwrap_or_default();

        conn.execute(
            "INSERT OR IGNORE INTO routine_candidates (
                candidate_id, created_at, pattern_type, description, frequency, score, sample_events
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                pattern.pattern_id,
                created_at,
                pattern.pattern_type.as_str(),
                pattern.description,
                pattern.occurrences,
                pattern.similarity_score,
                samples_json
            ],
        )?;
    }
    Ok(())
}
