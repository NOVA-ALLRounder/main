use super::*;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecApproval {
    pub id: String,
    pub command: String,
    pub cwd: Option<String>,
    pub created_at: String,
    pub expires_at: String,
    pub status: String,
    pub decision: Option<String>,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApprovalPolicy {
    pub policy_key: String,
    pub decision: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecAllowlistEntry {
    pub id: i64,
    pub pattern: String,
    pub cwd: Option<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub uses_count: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecResult {
    pub id: String,
    pub command: String,
    pub cwd: Option<String>,
    pub status: String,
    pub output: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

pub fn create_exec_approval(
    command: &str,
    cwd: Option<&str>,
    expires_in_secs: i64,
) -> Result<ExecApproval> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let now = chrono::Utc::now();
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = now.to_rfc3339();
        let expires_at = (now + chrono::Duration::seconds(expires_in_secs)).to_rfc3339();

        conn.execute(
            "INSERT INTO exec_approvals (id, command, cwd, created_at, expires_at, status, decision)
             VALUES (?1, ?2, ?3, ?4, ?5, 'pending', NULL)",
            params![id, command, cwd, created_at, expires_at],
        )?;

        return Ok(ExecApproval {
            id,
            command: command.to_string(),
            cwd: cwd.map(|c| c.to_string()),
            created_at,
            expires_at,
            status: "pending".to_string(),
            decision: None,
            resolved_at: None,
            resolved_by: None,
        });
    }
    Ok(ExecApproval {
        id: "".to_string(),
        command: command.to_string(),
        cwd: cwd.map(|c| c.to_string()),
        created_at: "".to_string(),
        expires_at: "".to_string(),
        status: "pending".to_string(),
        decision: None,
        resolved_at: None,
        resolved_by: None,
    })
}

pub fn resolve_exec_approval(
    id: &str,
    status: &str,
    resolved_by: Option<&str>,
    decision: Option<&str>,
) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let resolved_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE exec_approvals
             SET status = ?1, resolved_at = ?2, resolved_by = ?3, decision = ?4
             WHERE id = ?5",
            params![status, resolved_at, resolved_by, decision, id],
        )?;
    }
    Ok(())
}

pub fn get_exec_approval_status(id: &str) -> Result<String> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let status: String = conn.query_row(
            "SELECT status FROM exec_approvals WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        return Ok(status);
    }
    Err(rusqlite::Error::QueryReturnedNoRows)
}

pub fn list_exec_approvals(status_filter: Option<&str>, limit: i64) -> Result<Vec<ExecApproval>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let sql = match status_filter {
            Some(_) => "SELECT id, command, cwd, created_at, expires_at, status, decision, resolved_at, resolved_by FROM exec_approvals WHERE status = ?1 ORDER BY created_at DESC LIMIT ?2",
            None => "SELECT id, command, cwd, created_at, expires_at, status, decision, resolved_at, resolved_by FROM exec_approvals ORDER BY created_at DESC LIMIT ?1",
        };

        let mut approvals = Vec::new();
        if let Some(s) = status_filter {
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map(params![s, limit], |row| {
                Ok(ExecApproval {
                    id: row.get(0)?,
                    command: row.get(1)?,
                    cwd: row.get(2).ok(),
                    created_at: row.get(3)?,
                    expires_at: row.get(4)?,
                    status: row.get(5)?,
                    decision: row.get(6).ok(),
                    resolved_at: row.get(7).ok(),
                    resolved_by: row.get(8).ok(),
                })
            })?;
            for r in rows {
                approvals.push(r?);
            }
        } else {
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map(params![limit], |row| {
                Ok(ExecApproval {
                    id: row.get(0)?,
                    command: row.get(1)?,
                    cwd: row.get(2).ok(),
                    created_at: row.get(3)?,
                    expires_at: row.get(4)?,
                    status: row.get(5)?,
                    decision: row.get(6).ok(),
                    resolved_at: row.get(7).ok(),
                    resolved_by: row.get(8).ok(),
                })
            })?;
            for r in rows {
                approvals.push(r?);
            }
        }

        return Ok(approvals);
    }
    Ok(Vec::new())
}

pub fn find_valid_exec_approval(command: &str, cwd: Option<&str>) -> Result<Option<ExecApproval>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT id, command, cwd, created_at, expires_at, status, decision, resolved_at, resolved_by
             FROM exec_approvals
             WHERE status = 'approved'
               AND command = ?1
               AND expires_at > ?2
               AND (?3 IS NULL OR IFNULL(cwd, '') = ?3)
             ORDER BY resolved_at DESC
             LIMIT 1",
        )?;
        let row = stmt.query_row(params![command, now, cwd], |row| {
            Ok(ExecApproval {
                id: row.get(0)?,
                command: row.get(1)?,
                cwd: row.get(2).ok(),
                created_at: row.get(3)?,
                expires_at: row.get(4)?,
                status: row.get(5)?,
                decision: row.get(6).ok(),
                resolved_at: row.get(7).ok(),
                resolved_by: row.get(8).ok(),
            })
        });
        return match row {
            Ok(found) => Ok(Some(found)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        };
    }
    Ok(None)
}

pub fn get_exec_approval(id: &str) -> Result<Option<ExecApproval>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, command, cwd, created_at, expires_at, status, decision, resolved_at, resolved_by
             FROM exec_approvals
             WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(ExecApproval {
                id: row.get(0)?,
                command: row.get(1)?,
                cwd: row.get(2).ok(),
                created_at: row.get(3)?,
                expires_at: row.get(4)?,
                status: row.get(5)?,
                decision: row.get(6).ok(),
                resolved_at: row.get(7).ok(),
                resolved_by: row.get(8).ok(),
            }));
        }
    }
    Ok(None)
}

pub fn upsert_approval_policy(policy_key: &str, decision: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let updated_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO nl_approval_policies (policy_key, decision, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(policy_key) DO UPDATE SET decision = excluded.decision, updated_at = excluded.updated_at",
            params![policy_key, decision, updated_at],
        )?;
    }
    Ok(())
}

pub fn delete_approval_policy(policy_key: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "DELETE FROM nl_approval_policies WHERE policy_key = ?1",
            params![policy_key],
        )?;
    }
    Ok(())
}

pub fn get_approval_policy_decision(policy_key: &str) -> Result<Option<String>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt =
            conn.prepare("SELECT decision FROM nl_approval_policies WHERE policy_key = ?1")?;
        let mut rows = stmt.query(params![policy_key])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row.get(0)?));
        }
    }
    Ok(None)
}

pub fn list_approval_policies(limit: i64) -> Result<Vec<ApprovalPolicy>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT policy_key, decision, updated_at
             FROM nl_approval_policies
             ORDER BY updated_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(ApprovalPolicy {
                policy_key: row.get(0)?,
                decision: row.get(1)?,
                updated_at: row.get(2)?,
            })
        })?;
        let mut policies = Vec::new();
        for r in rows {
            policies.push(r?);
        }
        Ok(policies)
    } else {
        Ok(Vec::new())
    }
}

pub fn add_exec_allowlist(pattern: &str, cwd: Option<&str>) -> Result<i64> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO exec_allowlist (pattern, cwd, created_at) VALUES (?1, ?2, ?3)",
            params![pattern, cwd, created_at],
        )?;
        return Ok(conn.last_insert_rowid());
    }
    Ok(0)
}

pub fn list_exec_allowlist(limit: i64) -> Result<Vec<ExecAllowlistEntry>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, pattern, cwd, created_at, last_used_at, uses_count
             FROM exec_allowlist ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |row| {
            Ok(ExecAllowlistEntry {
                id: row.get(0)?,
                pattern: row.get(1)?,
                cwd: row.get(2).ok(),
                created_at: row.get(3)?,
                last_used_at: row.get(4).ok(),
                uses_count: row.get(5)?,
            })
        })?;
        let mut entries = Vec::new();
        for r in rows {
            entries.push(r?);
        }
        Ok(entries)
    } else {
        Ok(Vec::new())
    }
}

pub fn remove_exec_allowlist(id: i64) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute("DELETE FROM exec_allowlist WHERE id = ?1", params![id])?;
    }
    Ok(())
}

pub fn create_exec_result(command: &str, cwd: Option<&str>) -> Result<ExecResult> {
    let mut lock = get_db_lock();
    let id = uuid::Uuid::new_v4().to_string();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO exec_results (id, command, cwd, status, output, error, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'pending', NULL, NULL, ?4, NULL)",
            params![id, command, cwd, created_at],
        )?;
        return Ok(ExecResult {
            id,
            command: command.to_string(),
            cwd: cwd.map(|c| c.to_string()),
            status: "pending".to_string(),
            output: None,
            error: None,
            created_at,
            updated_at: None,
        });
    }
    Ok(ExecResult {
        id,
        command: command.to_string(),
        cwd: cwd.map(|c| c.to_string()),
        status: "pending".to_string(),
        output: None,
        error: None,
        created_at: "".to_string(),
        updated_at: None,
    })
}

pub fn update_exec_result(
    id: &str,
    status: &str,
    output: Option<&str>,
    error: Option<&str>,
) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let updated_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE exec_results
             SET status = ?1, output = ?2, error = ?3, updated_at = ?4
             WHERE id = ?5",
            params![status, output, error, updated_at, id],
        )?;
    }
    Ok(())
}

pub fn list_pending_exec_results(limit: i64) -> Result<Vec<ExecResult>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, command, cwd, status, output, error, created_at, updated_at
             FROM exec_results
             WHERE status = 'pending'
             ORDER BY created_at ASC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(ExecResult {
                id: row.get(0)?,
                command: row.get(1)?,
                cwd: row.get(2).ok(),
                status: row.get(3)?,
                output: row.get(4).ok(),
                error: row.get(5).ok(),
                created_at: row.get(6)?,
                updated_at: row.get(7).ok(),
            })
        })?;
        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    } else {
        Ok(Vec::new())
    }
}

pub fn list_exec_results(status_filter: Option<&str>, limit: i64) -> Result<Vec<ExecResult>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let sql = match status_filter {
            Some(_) => "SELECT id, command, cwd, status, output, error, created_at, updated_at FROM exec_results WHERE status = ?1 ORDER BY created_at DESC LIMIT ?2",
            None => "SELECT id, command, cwd, status, output, error, created_at, updated_at FROM exec_results ORDER BY created_at DESC LIMIT ?1",
        };

        let mut results = Vec::new();
        if let Some(status) = status_filter {
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map(params![status, limit], |row| {
                Ok(ExecResult {
                    id: row.get(0)?,
                    command: row.get(1)?,
                    cwd: row.get(2).ok(),
                    status: row.get(3)?,
                    output: row.get(4).ok(),
                    error: row.get(5).ok(),
                    created_at: row.get(6)?,
                    updated_at: row.get(7).ok(),
                })
            })?;
            for r in rows {
                results.push(r?);
            }
        } else {
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map(params![limit], |row| {
                Ok(ExecResult {
                    id: row.get(0)?,
                    command: row.get(1)?,
                    cwd: row.get(2).ok(),
                    status: row.get(3)?,
                    output: row.get(4).ok(),
                    error: row.get(5).ok(),
                    created_at: row.get(6)?,
                    updated_at: row.get(7).ok(),
                })
            })?;
            for r in rows {
                results.push(r?);
            }
        }
        Ok(results)
    } else {
        Ok(Vec::new())
    }
}

pub fn is_exec_allowlisted(command: &str, cwd: Option<&str>) -> Result<bool> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt =
            conn.prepare("SELECT id, pattern, cwd FROM exec_allowlist ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| {
            let cwd: Option<String> = row.get(2)?;
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, cwd))
        })?;
        for r in rows {
            let (id, pattern, entry_cwd) = r?;
            if let Some(ref required_cwd) = entry_cwd {
                if let Some(cwd_val) = cwd {
                    if required_cwd != cwd_val {
                        continue;
                    }
                } else {
                    continue;
                }
            }
            if exec_pattern_match(&pattern, command) {
                let now = chrono::Utc::now().to_rfc3339();
                let _ = conn.execute(
                    "UPDATE exec_allowlist SET last_used_at = ?1, uses_count = uses_count + 1 WHERE id = ?2",
                    params![now, id],
                );
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn exec_pattern_match(pattern: &str, command: &str) -> bool {
    let trimmed = pattern.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed == "*" || trimmed.eq_ignore_ascii_case("all") {
        return true;
    }
    if let Some(rest) = trimmed.strip_prefix("re:") {
        if let Ok(re) = regex::Regex::new(rest) {
            return re.is_match(command);
        }
    }
    if trimmed.starts_with('/') && trimmed.ends_with('/') && trimmed.len() > 2 {
        let body = &trimmed[1..trimmed.len() - 1];
        if let Ok(re) = regex::Regex::new(body) {
            return re.is_match(command);
        }
    }
    if trimmed.ends_with('*') {
        let prefix = trimmed.trim_end_matches('*');
        return command.starts_with(prefix);
    }
    command == trimmed
}
