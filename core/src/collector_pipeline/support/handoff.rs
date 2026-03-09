use super::super::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_handoff_payload(
    conn: &Connection,
    rules: &HandoffPrivacyRules,
    package_id: &str,
    created_at: &str,
    sessions_limit: usize,
    routines_limit: usize,
    resources_limit: usize,
    max_evidence: usize,
    redaction_scan_limit: usize,
) -> Result<Value> {
    let device_context = build_device_context(conn, rules)?;
    let last_event_ts = device_context
        .get("last_event_ts")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());

    let recent_sessions = build_recent_sessions(conn, sessions_limit, resources_limit)?;
    let routines = build_routine_candidates_section(conn, routines_limit, max_evidence)?;
    let signals = build_signals(conn, last_event_ts.as_deref())?;
    let privacy_state = build_privacy_state(conn, rules, redaction_scan_limit)?;

    Ok(json!({
        "package_id": package_id,
        "created_at": created_at,
        "version": "1.0",
        "device_context": device_context,
        "recent_sessions": recent_sessions,
        "routine_candidates": routines,
        "signals": signals,
        "privacy_state": privacy_state
    }))
}

pub(crate) fn build_device_context(
    conn: &Connection,
    rules: &HandoffPrivacyRules,
) -> Result<Value> {
    let row = conn
        .query_row(
            "SELECT ts, event_type, app, payload_json FROM events_v2 ORDER BY ts DESC LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;

    let Some((ts, event_type, app, payload_json)) = row else {
        return Ok(json!({
            "active_app": Value::Null,
            "active_window_hint": Value::Null,
            "last_event_ts": Value::Null
        }));
    };

    let payload: Value = serde_json::from_str(&payload_json).unwrap_or_else(|_| json!({}));
    let window_title = payload
        .get("window_title")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let hint = if window_title.is_empty() {
        Value::Null
    } else {
        Value::String(sanitize_hint(window_title, rules))
    };

    Ok(json!({
        "active_app": app,
        "active_window_hint": hint,
        "last_event_ts": ts,
        "last_event_type": event_type,
    }))
}

pub(crate) fn build_recent_sessions(
    conn: &Connection,
    limit: usize,
    max_resources: usize,
) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT session_id, start_ts, end_ts, duration_sec, summary_json
         FROM sessions_v2
         ORDER BY start_ts DESC
         LIMIT ?1",
    )?;

    let rows = stmt.query_map([limit as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<i64>>(3)?.unwrap_or(0),
            row.get::<_, Option<String>>(4)?
                .unwrap_or_else(|| "{}".to_string()),
        ))
    })?;

    let mut out = Vec::new();
    for row in rows {
        let (session_id, start_ts, end_ts, duration_sec, summary_json) = row?;
        let mut summary: Value = serde_json::from_str(&summary_json).unwrap_or_else(|_| json!({}));

        let resources = summary
            .get_mut("resources")
            .and_then(|v| v.as_array_mut())
            .map(|items| {
                if items.len() > max_resources {
                    items.truncate(max_resources);
                }
                items.clone()
            })
            .unwrap_or_default();

        let apps_timeline = summary
            .get("apps_timeline")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let key_events = summary
            .get("key_events")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let counts = summary.get("counts").cloned().unwrap_or_else(|| json!({}));

        out.push(json!({
            "session_id": session_id,
            "start_ts": start_ts,
            "end_ts": end_ts,
            "duration_sec": duration_sec,
            "apps_timeline": apps_timeline,
            "key_events": key_events,
            "resources": resources,
            "counts": counts
        }));
    }

    Ok(out)
}

pub(crate) fn build_routine_candidates_section(
    conn: &Connection,
    limit: usize,
    max_evidence: usize,
) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT pattern_id, pattern_json, support, confidence, last_seen_ts, evidence_session_ids
         FROM collector_routine_candidates
         ORDER BY support DESC, confidence DESC
         LIMIT ?1",
    )?;

    let rows = stmt.query_map([limit as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, f64>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;

    let mut out = Vec::new();
    for row in rows {
        let (pattern_id, pattern_json, support, confidence, last_seen_ts, evidence_json) = row?;
        let pattern: Value = serde_json::from_str(&pattern_json).unwrap_or_else(|_| json!({}));
        let mut evidence: Vec<Value> = serde_json::from_str(&evidence_json).unwrap_or_default();
        if evidence.len() > max_evidence {
            evidence.truncate(max_evidence);
        }

        out.push(json!({
            "pattern_id": pattern_id,
            "pattern": pattern,
            "support": support,
            "confidence": confidence,
            "last_seen_ts": last_seen_ts,
            "evidence_session_ids": evidence,
        }));
    }

    Ok(out)
}

pub(crate) fn build_signals(conn: &Connection, _last_event_ts: Option<&str>) -> Result<Value> {
    let since = format_utc_ts(Utc::now() - Duration::minutes(5));
    let p0_recent = conn
        .query_row(
            "SELECT 1 FROM events_v2 WHERE priority = 'P0' AND ts >= ?1 LIMIT 1",
            [since],
            |_row| Ok(true),
        )
        .optional()?
        .unwrap_or(false);

    let latest_type = conn
        .query_row(
            "SELECT event_type FROM events_v2 ORDER BY ts DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    let idle_state = match latest_type.unwrap_or_default().to_lowercase().as_str() {
        "os.idle_start" => Value::Bool(true),
        "os.idle_end" => Value::Bool(false),
        _ => Value::Null,
    };

    Ok(json!({
        "p0_recent": p0_recent,
        "idle_state": idle_state,
    }))
}

pub(crate) fn build_privacy_state(
    conn: &Connection,
    rules: &HandoffPrivacyRules,
    scan_limit: usize,
) -> Result<Value> {
    let mut stmt = conn.prepare("SELECT privacy_json FROM events_v2 ORDER BY ts DESC LIMIT ?1")?;
    let rows = stmt.query_map([scan_limit as i64], |row| row.get::<_, String>(0))?;

    let mut total = 0i64;
    let mut items: HashMap<String, i64> = HashMap::new();

    for row in rows {
        let privacy_json = row?;
        let parsed: Value = serde_json::from_str(&privacy_json).unwrap_or_else(|_| json!({}));
        if let Some(list) = parsed.get("redaction").and_then(|v| v.as_array()) {
            for item in list {
                if let Some(s) = item.as_str() {
                    total += 1;
                    *items.entry(s.to_string()).or_insert(0) += 1;
                }
            }
        }
    }

    let mut ranked: Vec<(String, i64)> = items.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1));
    ranked.truncate(10);

    let top_map: serde_json::Map<String, Value> = ranked
        .into_iter()
        .map(|(k, v)| (k, Value::Number(v.into())))
        .collect();

    Ok(json!({
        "content_collection": false,
        "denylist_active": !rules.denylist_apps.is_empty(),
        "redaction_summary": {
            "total": total,
            "items": Value::Object(top_map)
        }
    }))
}

pub(crate) fn sanitize_hint(input: &str, rules: &HandoffPrivacyRules) -> String {
    let mut value = input.to_string();

    for re in &rules.redaction_patterns {
        value = re.replace_all(&value, "[REDACTED]").to_string();
    }

    if value.chars().count() > rules.window_title_limit {
        value = value.chars().take(rules.window_title_limit).collect();
    }

    scrub_string(&value)
}

pub(crate) fn scrub_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mapped = map
                .into_iter()
                .map(|(k, v)| (k, scrub_value(v)))
                .collect::<serde_json::Map<String, Value>>();
            Value::Object(mapped)
        }
        Value::Array(list) => Value::Array(list.into_iter().map(scrub_value).collect()),
        Value::String(s) => Value::String(scrub_string(&s)),
        other => other,
    }
}

pub(crate) fn scrub_string(value: &str) -> String {
    static EMAIL_RE_STR: &str = r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}";
    static PATH_RE_STR: &str = r"([A-Za-z]:\\\\|/Users/|/home/|\\.xlsx|\\.docx|\\.pptx)";
    static LONG_DIGITS_RE_STR: &str = r"\b\d{12,}\b";
    static HEX64_RE_STR: &str = r"^[a-f0-9]{64}$";

    let email_re = Regex::new(EMAIL_RE_STR).expect("valid email regex");
    let path_re = Regex::new(PATH_RE_STR).expect("valid path regex");
    let long_digits_re = Regex::new(LONG_DIGITS_RE_STR).expect("valid long digits regex");
    let hex64_re = Regex::new(HEX64_RE_STR).expect("valid hex64 regex");

    if hex64_re.is_match(value) {
        return value.to_string();
    }

    if email_re.is_match(value) || path_re.is_match(value) || long_digits_re.is_match(value) {
        return "[REDACTED]".to_string();
    }

    value.to_string()
}

pub(crate) fn value_to_i64(value: &Value) -> Option<i64> {
    if let Some(v) = value.as_i64() {
        return Some(v);
    }
    if let Some(v) = value.as_u64() {
        return i64::try_from(v).ok();
    }
    if let Some(s) = value.as_str() {
        return s.parse::<i64>().ok();
    }
    None
}
