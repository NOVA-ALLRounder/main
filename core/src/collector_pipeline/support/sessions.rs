use super::super::*;

pub(crate) fn map_event_row(
    ts_raw: String,
    event_type: String,
    priority: String,
    app: String,
    resource_type: String,
    resource_id: String,
    payload_json: String,
) -> rusqlite::Result<SessionEventRow> {
    let ts = parse_iso_ts(&ts_raw).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid ts: {ts_raw}"),
            )),
        )
    })?;

    let payload: Value = serde_json::from_str(&payload_json).unwrap_or_else(|_| json!({}));

    Ok(SessionEventRow {
        ts,
        event_type,
        priority,
        app,
        resource_type,
        resource_id,
        payload,
    })
}

pub(crate) fn flush_session(
    current: &mut Vec<SessionEventRow>,
    sessions: &mut Vec<Vec<SessionEventRow>>,
) {
    if !current.is_empty() {
        sessions.push(std::mem::take(current));
    }
}

pub(crate) fn build_session_summary(events: &[SessionEventRow]) -> Value {
    let mut app_secs: HashMap<String, i64> = HashMap::new();
    let mut key_seen = HashSet::new();
    let mut key_events: Vec<String> = Vec::new();
    let mut resource_seen = HashSet::new();
    let mut resources: Vec<Value> = Vec::new();
    let mut p0 = 0i64;
    let mut p1 = 0i64;
    let mut p2 = 0i64;

    for event in events {
        if event.event_type.eq_ignore_ascii_case("os.app_focus_block") {
            let duration = event
                .payload
                .get("duration_sec")
                .and_then(value_to_i64)
                .unwrap_or(0);
            if duration > 0 {
                *app_secs.entry(event.app.clone()).or_insert(0) += duration;
            }
        }

        let event_type = event.event_type.to_lowercase();
        let priority = event.priority.to_uppercase();

        if priority == "P0" {
            p0 += 1;
        } else if priority == "P1" {
            p1 += 1;
        } else if priority == "P2" {
            p2 += 1;
        }

        let include_key = priority == "P0" || KEY_P1_TYPES.contains(&event_type.as_str());
        if include_key && !event_type.is_empty() && key_seen.insert(event_type.clone()) {
            key_events.push(event_type);
        }

        let key = format!("{}::{}", event.resource_type, event.resource_id);
        if resource_seen.insert(key) {
            resources.push(json!({
                "type": event.resource_type,
                "id": event.resource_id
            }));
            if resources.len() >= DEFAULT_MAX_RESOURCES {
                // Keep scanning for counts/key-events but no more resource append.
            }
        }
    }

    if resources.len() > DEFAULT_MAX_RESOURCES {
        resources.truncate(DEFAULT_MAX_RESOURCES);
    }

    let mut timeline: Vec<Value> = app_secs
        .into_iter()
        .map(|(app, sec)| json!({"app": app, "sec": sec}))
        .collect();
    timeline.sort_by(|a, b| {
        let a_sec = a.get("sec").and_then(|v| v.as_i64()).unwrap_or(0);
        let b_sec = b.get("sec").and_then(|v| v.as_i64()).unwrap_or(0);
        b_sec.cmp(&a_sec)
    });

    json!({
        "apps_timeline": timeline,
        "key_events": key_events,
        "resources": resources,
        "counts": {
            "total": events.len(),
            "p0": p0,
            "p1": p1,
            "p2": p2
        }
    })
}

pub(crate) fn map_routine_session_row(
    session_id: String,
    start_ts_raw: String,
    end_ts_raw: String,
    summary_json: String,
) -> rusqlite::Result<RoutineSession> {
    let start_ts = parse_iso_ts(&start_ts_raw).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid start ts: {start_ts_raw}"),
            )),
        )
    })?;
    let end_ts = parse_iso_ts(&end_ts_raw).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid end ts: {end_ts_raw}"),
            )),
        )
    })?;

    let summary: Value = serde_json::from_str(&summary_json).unwrap_or_else(|_| json!({}));
    let mut key_events = Vec::new();
    if let Some(list) = summary.get("key_events").and_then(|v| v.as_array()) {
        for item in list {
            if let Some(s) = item.as_str() {
                let lower = s.to_lowercase();
                if !lower.is_empty() {
                    key_events.push(lower);
                }
            }
        }
    }

    Ok(RoutineSession {
        session_id,
        start_ts,
        end_ts,
        key_events,
    })
}

pub(crate) fn unique_ngrams(events: &[String], n_min: usize, n_max: usize) -> HashSet<Vec<String>> {
    if n_min == 0 || n_max < n_min {
        return HashSet::new();
    }

    let limit = n_max.min(events.len());
    let mut out = HashSet::new();

    for n in n_min..=limit {
        for idx in 0..=(events.len() - n) {
            out.insert(events[idx..idx + n].to_vec());
        }
    }

    out
}

pub(crate) fn compute_confidence(
    support: i64,
    weekday_counts: &HashMap<u32, i64>,
    last_seen: DateTime<Utc>,
    now: DateTime<Utc>,
) -> f64 {
    let days_ago = (now - last_seen).num_days();
    let recency_bonus = if days_ago <= 1 {
        0.3
    } else if days_ago <= 7 {
        0.1
    } else {
        0.0
    };

    let periodicity_bonus = if weekday_counts.values().any(|count| *count >= 2) {
        0.1
    } else {
        0.0
    };

    support as f64 * (1.0 + recency_bonus) * (1.0 + periodicity_bonus)
}
