use super::*;

pub fn load_handoff_privacy_rules(path: &Path) -> HandoffPrivacyRules {
    let raw = match fs::read_to_string(path) {
        Ok(v) => v,
        Err(_) => return HandoffPrivacyRules::default(),
    };
    let yaml: serde_yaml::Value = match serde_yaml::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return HandoffPrivacyRules::default(),
    };

    let mut rules = HandoffPrivacyRules::default();

    if let Some(list) = yaml.get("denylist_apps").and_then(|v| v.as_sequence()) {
        for app in list {
            if let Some(s) = app.as_str() {
                rules.denylist_apps.insert(s.to_lowercase());
            }
        }
    }

    if let Some(limit) = yaml
        .get("length_limits")
        .and_then(|v| v.as_mapping())
        .and_then(|m| m.get(serde_yaml::Value::String("window_title".to_string())))
        .and_then(|v| v.as_i64())
    {
        if limit > 0 {
            rules.window_title_limit = limit as usize;
        }
    }

    if let Some(patterns) = yaml.get("redaction_patterns").and_then(|v| v.as_sequence()) {
        for item in patterns {
            let regex_str = if let Some(m) = item.as_mapping() {
                m.get(serde_yaml::Value::String("regex".to_string()))
                    .and_then(|v| v.as_str())
            } else {
                item.as_str()
            };

            if let Some(s) = regex_str {
                if let Ok(re) = Regex::new(s) {
                    rules.redaction_patterns.push(re);
                }
            }
        }
    }

    rules
}

pub fn build_handoff_with_size_guard(
    conn: &Connection,
    rules: &HandoffPrivacyRules,
    options: &HandoffBuildOptions,
) -> Result<HandoffPayload> {
    let package_id = uuid::Uuid::new_v4().to_string();
    let created_at = format_utc_ts(Utc::now());

    let profiles = [
        (
            options.recent_sessions,
            options.recent_routines,
            options.max_resources,
        ),
        (
            2usize.min(options.recent_sessions),
            options.recent_routines,
            options.max_resources,
        ),
        (
            1,
            5usize.min(options.recent_routines),
            5usize.min(options.max_resources),
        ),
        (
            1,
            3usize.min(options.recent_routines),
            3usize.min(options.max_resources),
        ),
        (1, 1, 1),
    ];

    let mut last_payload = json!({});
    let mut last_size = 0usize;

    for (session_limit, routine_limit, resource_limit) in profiles {
        let payload = build_handoff_payload(
            conn,
            rules,
            &package_id,
            &created_at,
            session_limit,
            routine_limit,
            resource_limit,
            options.max_evidence,
            options.redaction_scan_limit,
        )?;

        let scrubbed = scrub_value(payload);
        let bytes = serde_json::to_vec(&scrubbed)?.len();
        last_payload = scrubbed;
        last_size = bytes;

        if bytes <= options.max_size_bytes {
            return Ok(HandoffPayload {
                payload: last_payload,
                size_bytes: last_size,
            });
        }
    }

    Ok(HandoffPayload {
        payload: last_payload,
        size_bytes: last_size,
    })
}

pub fn fetch_latest_pending_handoff(conn: &Connection) -> Result<Option<PendingHandoffRow>> {
    let row = conn
        .query_row(
            "SELECT id, package_id, created_at, status,
                    COALESCE(attempt_count, 0), next_retry_at, lease_until, claimed_by, payload_json
             FROM collector_handoff_queue
             WHERE status = 'pending'
             ORDER BY created_at DESC
             LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )
        .optional()?;

    let Some((
        id,
        package_id,
        created_at,
        status,
        attempt_count,
        next_retry_at,
        lease_until,
        claimed_by,
        payload_json,
    )) = row
    else {
        return Ok(None);
    };
    let payload = serde_json::from_str(&payload_json).unwrap_or_else(|_| json!({}));
    Ok(Some(PendingHandoffRow {
        id,
        package_id,
        created_at,
        status,
        attempt_count,
        next_retry_at,
        lease_until,
        claimed_by,
        payload,
    }))
}

pub fn claim_retryable_handoff(
    conn: &mut Connection,
    max_attempts: i64,
    consumer_id: &str,
    lease_secs: i64,
) -> Result<Option<PendingHandoffRow>> {
    let capped_attempts = max_attempts.max(1);
    let lease_secs = lease_secs.max(10);
    let tx = conn.transaction()?;
    let mut stmt = tx.prepare(
        "SELECT id, package_id, created_at, status,
                COALESCE(attempt_count, 0), next_retry_at, lease_until, claimed_by, payload_json
         FROM collector_handoff_queue
         WHERE status IN ('pending', 'failed', 'processing')
           AND COALESCE(attempt_count, 0) < ?1
         ORDER BY created_at DESC
         LIMIT 100",
    )?;

    let rows = stmt.query_map([capped_attempts], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, String>(8)?,
        ))
    })?;

    let now = Utc::now();
    let now_iso = format_utc_ts(now);
    let new_lease_until = format_utc_ts(now + Duration::seconds(lease_secs));
    let mut claimed: Option<PendingHandoffRow> = None;

    for row in rows {
        let (
            id,
            package_id,
            created_at,
            status,
            attempt_count,
            next_retry_at,
            lease_until,
            _claimed_by,
            payload_json,
        ) = row?;
        let status_norm = status.trim().to_lowercase();
        let due = if status_norm == "pending" {
            true
        } else if status_norm == "failed" {
            match next_retry_at.as_deref() {
                Some(ts) => parse_iso_ts(ts).map(|at| at <= now).unwrap_or(true),
                None => true,
            }
        } else if status_norm == "processing" {
            match lease_until.as_deref() {
                Some(ts) => parse_iso_ts(ts).map(|at| at <= now).unwrap_or(true),
                None => true,
            }
        } else {
            false
        };
        if !due {
            continue;
        }

        let updated = tx.execute(
            "UPDATE collector_handoff_queue
             SET status = 'processing',
                 claimed_by = ?1,
                 lease_until = ?2,
                 last_attempt_at = ?3
             WHERE id = ?4
               AND COALESCE(attempt_count, 0) < ?5
               AND (
                    status = 'pending'
                    OR (status = 'failed' AND (next_retry_at IS NULL OR next_retry_at <= ?6))
                    OR (status = 'processing' AND (lease_until IS NULL OR lease_until <= ?6))
               )",
            params![
                consumer_id,
                new_lease_until,
                now_iso,
                id,
                capped_attempts,
                now_iso
            ],
        )?;
        if updated == 0 {
            continue;
        }

        let payload = serde_json::from_str(&payload_json).unwrap_or_else(|_| json!({}));
        claimed = Some(PendingHandoffRow {
            id,
            package_id,
            created_at,
            status: "processing".to_string(),
            attempt_count,
            next_retry_at,
            lease_until: Some(new_lease_until.clone()),
            claimed_by: Some(consumer_id.to_string()),
            payload,
        });
        break;
    }

    drop(stmt);
    tx.commit()?;
    Ok(claimed)
}

pub fn fetch_latest_retryable_handoff(
    conn: &Connection,
    max_attempts: i64,
) -> Result<Option<PendingHandoffRow>> {
    let capped_attempts = max_attempts.max(1);
    let mut stmt = conn.prepare(
        "SELECT id, package_id, created_at, status,
                COALESCE(attempt_count, 0), next_retry_at, lease_until, claimed_by, payload_json
         FROM collector_handoff_queue
         WHERE status IN ('pending', 'failed', 'processing')
           AND COALESCE(attempt_count, 0) < ?1
         ORDER BY created_at DESC
         LIMIT 50",
    )?;

    let rows = stmt.query_map([capped_attempts], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, String>(8)?,
        ))
    })?;

    let now = Utc::now();
    for row in rows {
        let (
            id,
            package_id,
            created_at,
            status,
            attempt_count,
            next_retry_at,
            lease_until,
            claimed_by,
            payload_json,
        ) = row?;
        let status_norm = status.trim().to_lowercase();
        let due = if status_norm == "pending" {
            true
        } else if status_norm == "failed" {
            match next_retry_at.as_deref() {
                Some(ts) => parse_iso_ts(ts).map(|at| at <= now).unwrap_or(true),
                None => true,
            }
        } else if status_norm == "processing" {
            match lease_until.as_deref() {
                Some(ts) => parse_iso_ts(ts).map(|at| at <= now).unwrap_or(true),
                None => true,
            }
        } else {
            false
        };
        if !due {
            continue;
        }
        let payload = serde_json::from_str(&payload_json).unwrap_or_else(|_| json!({}));
        return Ok(Some(PendingHandoffRow {
            id,
            package_id,
            created_at,
            status,
            attempt_count,
            next_retry_at,
            lease_until,
            claimed_by,
            payload,
        }));
    }

    Ok(None)
}

pub fn fetch_latest_pending_handoff_payload(conn: &Connection) -> Result<Option<Value>> {
    Ok(fetch_latest_pending_handoff(conn)?.map(|row| row.payload))
}

pub fn clear_pending_handoff(conn: &Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM collector_handoff_queue WHERE status = 'pending'",
        [],
    )?;
    Ok(())
}

pub fn update_handoff_status(
    conn: &Connection,
    id: i64,
    status: &str,
    error: Option<&str>,
) -> Result<()> {
    let now = format_utc_ts(Utc::now());
    conn.execute(
        "UPDATE collector_handoff_queue
         SET status = ?1,
             error = ?2,
             last_attempt_at = ?4,
             next_retry_at = CASE WHEN ?1 = 'pending' THEN next_retry_at ELSE NULL END,
             lease_until = CASE WHEN ?1 = 'processing' THEN lease_until ELSE NULL END,
             claimed_by = CASE WHEN ?1 = 'processing' THEN claimed_by ELSE NULL END,
             expires_at = CASE WHEN ?1 = 'pending' THEN expires_at ELSE ?4 END
         WHERE id = ?3",
        params![status, error, id, now],
    )?;
    Ok(())
}

pub fn mark_handoff_consumed(conn: &Connection, id: i64) -> Result<()> {
    let now = format_utc_ts(Utc::now());
    conn.execute(
        "UPDATE collector_handoff_queue
         SET status = 'consumed',
             error = NULL,
             last_attempt_at = ?1,
             next_retry_at = NULL,
             lease_until = NULL,
             claimed_by = NULL,
             expires_at = ?1
         WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

pub fn mark_handoff_failed_with_backoff(
    conn: &Connection,
    id: i64,
    error: &str,
    max_attempts: i64,
    backoff_base_secs: u64,
) -> Result<HandoffFailureUpdate> {
    let max_attempts = max_attempts.max(1);
    let current_attempt = conn
        .query_row(
            "SELECT COALESCE(attempt_count, 0) FROM collector_handoff_queue WHERE id = ?1",
            [id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    let attempt_count = current_attempt.saturating_add(1);
    let terminal = attempt_count >= max_attempts;
    let now = Utc::now();
    let now_iso = format_utc_ts(now);
    let next_retry_at = if terminal {
        None
    } else {
        let base = backoff_base_secs.max(1);
        let shift = (attempt_count.saturating_sub(1) as u32).min(8);
        let delay = base.saturating_mul(1u64 << shift).min(7_200);
        Some(format_utc_ts(now + Duration::seconds(delay as i64)))
    };

    conn.execute(
        "UPDATE collector_handoff_queue
         SET status = 'failed',
             error = ?1,
             attempt_count = ?2,
             last_attempt_at = ?3,
             next_retry_at = ?4,
             lease_until = NULL,
             claimed_by = NULL,
             expires_at = ?3
         WHERE id = ?5",
        params![error, attempt_count, now_iso, next_retry_at.as_deref(), id],
    )?;

    Ok(HandoffFailureUpdate {
        attempt_count,
        max_attempts,
        next_retry_at,
        terminal,
    })
}

pub fn enqueue_handoff(conn: &Connection, payload: &HandoffPayload) -> Result<()> {
    let package_id = payload
        .payload
        .get("package_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let created_at = payload
        .payload
        .get("created_at")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    conn.execute(
        "INSERT INTO collector_handoff_queue (
            package_id, created_at, status, payload_json, payload_size, expires_at, error
         ) VALUES (?1, ?2, 'pending', ?3, ?4, NULL, NULL)",
        params![
            package_id,
            created_at,
            serde_json::to_string(&payload.payload)?,
            payload.size_bytes as i64
        ],
    )?;

    Ok(())
}
