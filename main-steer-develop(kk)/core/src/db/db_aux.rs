use super::*;

// Memory System: Chat History
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub created_at: String,
}

pub fn insert_chat_message(role: &str, content: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO chat_history (role, content, created_at) VALUES (?1, ?2, ?3)",
            params![role, content, created_at],
        )?;
    }
    Ok(())
}

pub fn get_recent_chat_history(limit: i64) -> Result<Vec<ChatMessage>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt =
            conn.prepare("SELECT role, content, created_at FROM chat_history ORDER BY created_at DESC LIMIT ?1")?;

        let rows = stmt.query_map([limit], |row| {
            Ok(ChatMessage {
                role: row.get(0)?,
                content: row.get(1)?,
                created_at: row.get(2)?,
            })
        })?;

        let mut history = Vec::new();
        for row in rows {
            history.push(row?);
        }
        history.reverse();
        Ok(history)
    } else {
        Ok(Vec::new())
    }
}

// --- Learned Routines (Macro Recorder) ---
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LearnedRoutine {
    pub id: i64,
    pub name: String,
    pub steps_json: String,
    pub created_at: String,
}

pub fn save_learned_routine(name: &str, steps_json: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO learned_routines (name, steps_json, created_at) VALUES (?1, ?2, ?3)",
            params![name, steps_json, created_at],
        )?;
        Ok(())
    } else {
        Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(1),
            Some("DB not initialized".to_string()),
        ))
    }
}

pub fn get_learned_routine(name: &str) -> Result<Option<LearnedRoutine>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt =
            conn.prepare("SELECT id, name, steps_json, created_at FROM learned_routines WHERE name = ?1")?;
        let mut rows = stmt.query(params![name])?;

        if let Some(row) = rows.next()? {
            Ok(Some(LearnedRoutine {
                id: row.get(0)?,
                name: row.get(1)?,
                steps_json: row.get(2)?,
                created_at: row.get(3)?,
            }))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

pub fn list_learned_routines() -> Result<Vec<LearnedRoutine>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt =
            conn.prepare("SELECT id, name, steps_json, created_at FROM learned_routines ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| {
            Ok(LearnedRoutine {
                id: row.get(0)?,
                name: row.get(1)?,
                steps_json: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    } else {
        Ok(Vec::new())
    }
}

pub fn delete_learned_routine(id: i64) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute("DELETE FROM learned_routines WHERE id = ?1", params![id])?;
    }
    Ok(())
}

// --- Dashboard Stats ---
#[derive(Debug, Clone, serde::Serialize)]
pub struct DashboardStats {
    pub total_sessions: i64,
    pub total_time_mins: i64,
    pub top_apps: Vec<(String, i64)>,
    pub rec_pending: i64,
}

pub fn get_dashboard_stats() -> Result<DashboardStats> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let (total_sessions, total_time_mins): (i64, i64) = match conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(duration_sec)/60, 0) FROM sessions_v2",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ) {
            Ok(res) => res,
            Err(_) => (0, 0),
        };

        let rec_pending: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM recommendations WHERE status = 'pending'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let mut top_apps = Vec::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT app_name, count(*) as c FROM (select app as app_name from events_v2) GROUP BY app_name ORDER BY c DESC LIMIT 3",
        ) {
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            });
            if let Ok(iter) = rows {
                for r in iter {
                    if let Ok(val) = r {
                        top_apps.push(val);
                    }
                }
            }
        }

        Ok(DashboardStats {
            total_sessions,
            total_time_mins,
            top_apps,
            rec_pending,
        })
    } else {
        Ok(DashboardStats {
            total_sessions: 0,
            total_time_mins: 0,
            top_apps: vec![],
            rec_pending: 0,
        })
    }
}

pub fn get_recent_recommendations(limit: i64) -> Result<Vec<Recommendation>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, status, title, summary, trigger, actions, n8n_prompt, confidence, workflow_id, workflow_json, evidence, pattern_id, last_error
             FROM recommendations
             WHERE status NOT IN ('dismissed', 'completed')
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], super::db_recommendation::map_row)?;

        let mut recs = Vec::new();
        for r in rows {
            recs.push(r?);
        }
        Ok(recs)
    } else {
        Ok(Vec::new())
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PolicyConfigReport {
    pub tool_allowlist: Vec<String>,
    pub tool_denylist: Vec<String>,
    pub shell_allowlist: Vec<String>,
    pub shell_denylist: Vec<String>,
    pub write_lock_default: bool,
}

pub fn get_active_policy_config() -> PolicyConfigReport {
    PolicyConfigReport {
        tool_allowlist: std::env::var("TOOL_ALLOWLIST")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        tool_denylist: std::env::var("TOOL_DENYLIST")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        shell_allowlist: std::env::var("SHELL_ALLOWLIST")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        shell_denylist: std::env::var("SHELL_DENYLIST")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        write_lock_default: true,
    }
}
