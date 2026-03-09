use rusqlite::{params, Connection, Result};

use super::get_db_lock;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CollectorHandoffReceiptRecord {
    pub id: i64,
    pub received_at: String,
    pub package_id: String,
    pub collector_row_id: Option<i64>,
    pub status: String,
    pub recommendation_id: Option<i64>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkflowProvisionOpRecord {
    pub id: i64,
    pub recommendation_id: i64,
    pub claim_token: Option<String>,
    pub status: String,
    pub workflow_id: Option<String>,
    pub workflow_json: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub fn record_collector_handoff_receipt(
    package_id: &str,
    collector_row_id: Option<i64>,
    status: &str,
    recommendation_id: Option<i64>,
    detail: Option<&str>,
) -> Result<()> {
    let package_id = package_id.trim();
    if package_id.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "package_id must not be empty".to_string(),
        ));
    }
    let status = status.trim().to_lowercase();
    if status.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "status must not be empty".to_string(),
        ));
    }

    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let received_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO collector_handoff_receipts (
                received_at, package_id, collector_row_id, status, recommendation_id, detail
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                received_at,
                package_id,
                collector_row_id,
                status,
                recommendation_id,
                detail
            ],
        )?;
    }
    Ok(())
}

pub fn list_collector_handoff_receipts(limit: i64) -> Result<Vec<CollectorHandoffReceiptRecord>> {
    let bounded_limit = limit.clamp(1, 500);
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, received_at, package_id, collector_row_id, status, recommendation_id, detail
             FROM collector_handoff_receipts
             ORDER BY id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![bounded_limit], |row| {
            Ok(CollectorHandoffReceiptRecord {
                id: row.get(0)?,
                received_at: row.get(1)?,
                package_id: row.get(2)?,
                collector_row_id: row.get(3).ok(),
                status: row.get(4)?,
                recommendation_id: row.get(5).ok(),
                detail: row.get(6).ok(),
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        return Ok(out);
    }
    Ok(Vec::new())
}

pub fn list_workflow_provision_ops(
    limit: i64,
    status: Option<&str>,
    recommendation_id: Option<i64>,
) -> Result<Vec<WorkflowProvisionOpRecord>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let capped = limit.clamp(1, 500);
        let mut rows_out = Vec::new();
        let status_filter = status.map(str::trim).filter(|s| !s.is_empty());
        match (recommendation_id, status_filter) {
            (Some(rec_id), Some(status_value)) => {
                let mut stmt = conn.prepare(
                    "SELECT id, recommendation_id, claim_token, status, workflow_id, workflow_json, error, created_at, updated_at
                     FROM workflow_provision_ops
                     WHERE recommendation_id = ?1
                       AND status = ?2
                     ORDER BY updated_at DESC
                     LIMIT ?3",
                )?;
                let rows = stmt.query_map(params![rec_id, status_value, capped], |row| {
                    Ok(WorkflowProvisionOpRecord {
                        id: row.get(0)?,
                        recommendation_id: row.get(1)?,
                        claim_token: row.get(2)?,
                        status: row.get(3)?,
                        workflow_id: row.get(4)?,
                        workflow_json: row.get(5)?,
                        error: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                })?;
                for row in rows {
                    rows_out.push(row?);
                }
            }
            (Some(rec_id), None) => {
                let mut stmt = conn.prepare(
                    "SELECT id, recommendation_id, claim_token, status, workflow_id, workflow_json, error, created_at, updated_at
                     FROM workflow_provision_ops
                     WHERE recommendation_id = ?1
                     ORDER BY updated_at DESC
                     LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![rec_id, capped], |row| {
                    Ok(WorkflowProvisionOpRecord {
                        id: row.get(0)?,
                        recommendation_id: row.get(1)?,
                        claim_token: row.get(2)?,
                        status: row.get(3)?,
                        workflow_id: row.get(4)?,
                        workflow_json: row.get(5)?,
                        error: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                })?;
                for row in rows {
                    rows_out.push(row?);
                }
            }
            (None, Some(status_value)) => {
                let mut stmt = conn.prepare(
                    "SELECT id, recommendation_id, claim_token, status, workflow_id, workflow_json, error, created_at, updated_at
                     FROM workflow_provision_ops
                     WHERE status = ?1
                     ORDER BY updated_at DESC
                     LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![status_value, capped], |row| {
                    Ok(WorkflowProvisionOpRecord {
                        id: row.get(0)?,
                        recommendation_id: row.get(1)?,
                        claim_token: row.get(2)?,
                        status: row.get(3)?,
                        workflow_id: row.get(4)?,
                        workflow_json: row.get(5)?,
                        error: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                })?;
                for row in rows {
                    rows_out.push(row?);
                }
            }
            (None, None) => {
                let mut stmt = conn.prepare(
                    "SELECT id, recommendation_id, claim_token, status, workflow_id, workflow_json, error, created_at, updated_at
                     FROM workflow_provision_ops
                     ORDER BY updated_at DESC
                     LIMIT ?1",
                )?;
                let rows = stmt.query_map(params![capped], |row| {
                    Ok(WorkflowProvisionOpRecord {
                        id: row.get(0)?,
                        recommendation_id: row.get(1)?,
                        claim_token: row.get(2)?,
                        status: row.get(3)?,
                        workflow_id: row.get(4)?,
                        workflow_json: row.get(5)?,
                        error: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                })?;
                for row in rows {
                    rows_out.push(row?);
                }
            }
        }
        return Ok(rows_out);
    }
    Ok(Vec::new())
}

pub fn latest_workflow_provision_op(
    recommendation_id: i64,
) -> Result<Option<WorkflowProvisionOpRecord>> {
    let rows = list_workflow_provision_ops(1, None, Some(recommendation_id))?;
    Ok(rows.into_iter().next())
}

pub fn release_recommendation_provisioning_claim(id: i64, claim_token: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "UPDATE recommendations
             SET workflow_id = NULL
             WHERE id = ?1 AND workflow_id = ?2",
            params![id, claim_token],
        )?;
    }
    Ok(())
}

pub fn mark_recommendation_approved(id: i64, workflow_id: &str, workflow_json: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let approved_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE recommendations
             SET status = 'approved', workflow_id = ?1, workflow_json = ?2, approved_at = ?3, snoozed_until = NULL
             WHERE id = ?4",
            params![workflow_id, workflow_json, approved_at, id],
        )?;
    }
    Ok(())
}

fn update_workflow_provision_op_status_with_conn(
    conn: &Connection,
    op_id: i64,
    status: &str,
    error: Option<&str>,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE workflow_provision_ops
         SET status = ?1,
             error = ?2,
             updated_at = ?3
         WHERE id = ?4",
        params![status, error, now, op_id],
    )?;
    Ok(())
}

fn commit_workflow_provision_success_with_conn(
    conn: &mut Connection,
    op_id: i64,
    recommendation_id: i64,
    workflow_id: &str,
    workflow_json: Option<&str>,
) -> Result<()> {
    let approved_at = chrono::Utc::now().to_rfc3339();
    let tx = conn.transaction()?;
    let rec_changed = tx.execute(
        "UPDATE recommendations
         SET status = 'approved',
             workflow_id = ?1,
             workflow_json = CASE
                WHEN ?2 IS NULL OR TRIM(?2) = '' THEN workflow_json
                ELSE ?2
             END,
             approved_at = ?3,
             snoozed_until = NULL,
             last_error = NULL
         WHERE id = ?4",
        params![workflow_id, workflow_json, approved_at, recommendation_id],
    )?;
    if rec_changed == 0 {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "recommendation {} not found while committing workflow provision op {}",
            recommendation_id, op_id
        )));
    }

    let op_changed = tx.execute(
        "UPDATE workflow_provision_ops
         SET status = 'committed',
             workflow_id = ?1,
             workflow_json = CASE
                WHEN ?2 IS NULL OR TRIM(?2) = '' THEN workflow_json
                ELSE ?2
             END,
             error = NULL,
             updated_at = ?3
         WHERE id = ?4",
        params![workflow_id, workflow_json, approved_at, op_id],
    )?;
    if op_changed == 0 {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "workflow provision op {} not found during commit",
            op_id
        )));
    }

    tx.commit()?;
    Ok(())
}

pub fn create_workflow_provision_op(
    recommendation_id: i64,
    claim_token: Option<&str>,
) -> Result<i64> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO workflow_provision_ops (
                recommendation_id, claim_token, status, workflow_id, workflow_json, error, created_at, updated_at
            ) VALUES (?1, ?2, 'requested', NULL, NULL, NULL, ?3, ?3)",
            params![recommendation_id, claim_token, now],
        )?;
        return Ok(conn.last_insert_rowid());
    }
    Err(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(1),
        Some("DB not initialized".to_string()),
    ))
}

pub fn mark_workflow_provision_created(
    op_id: i64,
    workflow_id: &str,
    workflow_json: Option<&str>,
) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE workflow_provision_ops
             SET status = 'created',
                 workflow_id = ?1,
                 workflow_json = CASE
                    WHEN ?2 IS NULL OR TRIM(?2) = '' THEN workflow_json
                    ELSE ?2
                 END,
                 error = NULL,
                 updated_at = ?3
             WHERE id = ?4",
            params![workflow_id, workflow_json, now, op_id],
        )?;
    }
    Ok(())
}

pub fn mark_workflow_provision_in_progress(op_id: i64, detail: Option<&str>) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        update_workflow_provision_op_status_with_conn(conn, op_id, "provisioning", detail)?;
    }
    Ok(())
}

pub fn mark_workflow_provision_failed(op_id: i64, error: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        update_workflow_provision_op_status_with_conn(conn, op_id, "failed", Some(error))?;
    }
    Ok(())
}

pub fn mark_workflow_provision_reconcile_needed(op_id: i64, error: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        update_workflow_provision_op_status_with_conn(
            conn,
            op_id,
            "reconcile_needed",
            Some(error),
        )?;
    }
    Ok(())
}

pub fn commit_workflow_provision_success(
    op_id: i64,
    recommendation_id: i64,
    workflow_id: &str,
    workflow_json: Option<&str>,
) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        return commit_workflow_provision_success_with_conn(
            conn,
            op_id,
            recommendation_id,
            workflow_id,
            workflow_json,
        );
    }
    Err(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(1),
        Some("DB not initialized".to_string()),
    ))
}

pub fn reconcile_workflow_provision_ops(limit: i64) -> Result<Vec<String>> {
    let capped = limit.clamp(1, 200);
    let requested_timeout_secs = std::env::var("STEER_WORKFLOW_PROVISION_REQUESTED_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .map(|v| v.clamp(15, 86_400))
        .unwrap_or(180);
    let provisioning_timeout_secs =
        std::env::var("STEER_WORKFLOW_PROVISION_IN_PROGRESS_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.trim().parse::<i64>().ok())
            .map(|v| v.clamp(30, 172_800))
            .unwrap_or(900);
    let mut lock = get_db_lock();
    let mut outcomes = Vec::new();
    if let Some(conn) = lock.as_mut() {
        let mut stale_stmt = conn.prepare(
            "SELECT id, recommendation_id, claim_token, status, updated_at
             FROM workflow_provision_ops
             WHERE status IN ('requested', 'provisioning')
             ORDER BY updated_at ASC
             LIMIT ?1",
        )?;
        let stale_rows = stale_stmt.query_map(params![capped], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;

        for stale in stale_rows {
            let (op_id, recommendation_id, claim_token, status, updated_at) = stale?;
            let updated = match chrono::DateTime::parse_from_rfc3339(&updated_at) {
                Ok(ts) => ts.with_timezone(&chrono::Utc),
                Err(_) => continue,
            };
            let age_secs = chrono::Utc::now()
                .signed_duration_since(updated)
                .num_seconds();
            let timeout_secs = if status.eq_ignore_ascii_case("provisioning") {
                provisioning_timeout_secs
            } else {
                requested_timeout_secs
            };
            if age_secs < timeout_secs {
                continue;
            }
            let msg = format!(
                "workflow provisioning {} timed out after {}s (age={}s)",
                status, timeout_secs, age_secs
            );
            let _ =
                update_workflow_provision_op_status_with_conn(conn, op_id, "failed", Some(&msg));
            if let Some(token) = claim_token
                .as_deref()
                .map(str::trim)
                .filter(|v| v.starts_with("provisioning:"))
            {
                let _ = conn.execute(
                    "UPDATE recommendations
                     SET workflow_id = NULL,
                         last_error = ?1
                     WHERE id = ?2
                       AND workflow_id = ?3",
                    params![msg, recommendation_id, token],
                );
            } else {
                let _ = conn.execute(
                    "UPDATE recommendations
                     SET last_error = ?1
                     WHERE id = ?2",
                    params![msg, recommendation_id],
                );
            }
            outcomes.push(format!(
                "op {} timed out from {} -> failed (recommendation {})",
                op_id, status, recommendation_id
            ));
        }
        drop(stale_stmt);

        let mut stmt = conn.prepare(
            "SELECT id, recommendation_id, claim_token, status, workflow_id, workflow_json, error, created_at, updated_at
             FROM workflow_provision_ops
             WHERE status IN ('created', 'reconcile_needed')
             ORDER BY updated_at ASC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![capped], |row| {
            Ok(WorkflowProvisionOpRecord {
                id: row.get(0)?,
                recommendation_id: row.get(1)?,
                claim_token: row.get(2)?,
                status: row.get(3)?,
                workflow_id: row.get(4)?,
                workflow_json: row.get(5)?,
                error: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?;

        let mut candidates = Vec::new();
        for row in rows {
            candidates.push(row?);
        }
        drop(stmt);

        for op in candidates {
            let workflow_id = match op
                .workflow_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                Some(v) => v.to_string(),
                None => {
                    let msg = format!("op {} has no workflow_id", op.id);
                    let _ = update_workflow_provision_op_status_with_conn(
                        conn,
                        op.id,
                        "failed",
                        Some(&msg),
                    );
                    outcomes.push(msg);
                    continue;
                }
            };

            let current_workflow_id = {
                let mut rec_stmt =
                    conn.prepare("SELECT workflow_id FROM recommendations WHERE id = ?1")?;
                let mut rec_rows = rec_stmt.query(params![op.recommendation_id])?;
                let Some(rec_row) = rec_rows.next()? else {
                    let msg = format!(
                        "op {} recommendation {} not found",
                        op.id, op.recommendation_id
                    );
                    let _ = update_workflow_provision_op_status_with_conn(
                        conn,
                        op.id,
                        "failed",
                        Some(&msg),
                    );
                    outcomes.push(msg);
                    continue;
                };
                rec_row.get::<_, Option<String>>(0)?
            };

            if let Some(existing) = current_workflow_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                if existing != workflow_id && !existing.starts_with("provisioning:") {
                    let msg = format!(
                        "op {} skipped due workflow mismatch recommendation={} existing={} op={}",
                        op.id, op.recommendation_id, existing, workflow_id
                    );
                    let _ = update_workflow_provision_op_status_with_conn(
                        conn,
                        op.id,
                        "failed",
                        Some(&msg),
                    );
                    outcomes.push(msg);
                    continue;
                }
            }

            match commit_workflow_provision_success_with_conn(
                conn,
                op.id,
                op.recommendation_id,
                &workflow_id,
                op.workflow_json.as_deref(),
            ) {
                Ok(()) => outcomes.push(format!(
                    "op {} committed recommendation {}",
                    op.id, op.recommendation_id
                )),
                Err(e) => {
                    let msg = format!("op {} reconcile failed: {}", op.id, e);
                    let _ = update_workflow_provision_op_status_with_conn(
                        conn,
                        op.id,
                        "reconcile_needed",
                        Some(&msg),
                    );
                    outcomes.push(msg);
                }
            }
        }
    }
    Ok(outcomes)
}
