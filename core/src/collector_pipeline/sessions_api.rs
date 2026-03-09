use super::*;

pub fn fetch_events(
    conn: &Connection,
    start_ts: Option<&str>,
    end_ts: Option<&str>,
) -> Result<Vec<SessionEventRow>> {
    let mut sql = String::from(
        "SELECT ts, event_type, priority, app, resource_type, resource_id, payload_json FROM events_v2",
    );
    let mut clauses: Vec<&str> = Vec::new();
    let mut params_vec: Vec<String> = Vec::new();

    if let Some(start) = start_ts {
        clauses.push("ts >= ?");
        params_vec.push(start.to_string());
    }
    if let Some(end) = end_ts {
        clauses.push("ts <= ?");
        params_vec.push(end.to_string());
    }

    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }
    sql.push_str(" ORDER BY ts ASC");

    let mut stmt = conn.prepare(&sql)?;

    let rows = if params_vec.is_empty() {
        stmt.query_map([], |row| {
            map_event_row(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            )
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    } else if params_vec.len() == 1 {
        stmt.query_map([params_vec[0].as_str()], |row| {
            map_event_row(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            )
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        stmt.query_map(
            params![params_vec[0].as_str(), params_vec[1].as_str()],
            |row| {
                map_event_row(
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                )
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };

    Ok(rows)
}

pub fn sessionize_events(
    events: &[SessionEventRow],
    gap_seconds: i64,
) -> Vec<Vec<SessionEventRow>> {
    let mut sessions: Vec<Vec<SessionEventRow>> = Vec::new();
    let mut current: Vec<SessionEventRow> = Vec::new();
    let mut last_ts: Option<DateTime<Utc>> = None;

    for event in events {
        if let Some(prev) = last_ts {
            if gap_seconds > 0 {
                let gap = (event.ts - prev).num_seconds();
                if gap >= gap_seconds {
                    flush_session(&mut current, &mut sessions);
                }
            }
        }

        if event.event_type.eq_ignore_ascii_case("os.idle_start") {
            flush_session(&mut current, &mut sessions);
            last_ts = None;
            continue;
        }

        current.push(event.clone());

        if event.priority.eq_ignore_ascii_case("P0") {
            flush_session(&mut current, &mut sessions);
            last_ts = None;
            continue;
        }

        last_ts = Some(event.ts);
    }

    flush_session(&mut current, &mut sessions);
    sessions
}

pub fn build_session_records(sessions: &[Vec<SessionEventRow>]) -> Vec<SessionRecord> {
    let mut records = Vec::new();

    for session in sessions {
        if session.is_empty() {
            continue;
        }
        let start = session.first().expect("non-empty session").ts;
        let end = session.last().expect("non-empty session").ts;
        let duration_sec = (end - start).num_seconds().max(0);

        let summary = build_session_summary(session);
        records.push(SessionRecord {
            session_id: uuid::Uuid::new_v4().to_string(),
            start_ts: format_utc_ts(start),
            end_ts: format_utc_ts(end),
            duration_sec,
            summary,
        });
    }

    records
}

pub fn insert_session_record(conn: &Connection, record: &SessionRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO sessions_v2 (session_id, start_ts, end_ts, duration_sec, summary_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            record.session_id,
            record.start_ts,
            record.end_ts,
            record.duration_sec,
            serde_json::to_string(&record.summary)?
        ],
    )?;
    Ok(())
}

pub fn fetch_sessions(
    conn: &Connection,
    start_ts: Option<&str>,
    end_ts: Option<&str>,
) -> Result<Vec<RoutineSession>> {
    let mut sql =
        String::from("SELECT session_id, start_ts, end_ts, summary_json FROM sessions_v2");
    let mut clauses: Vec<&str> = Vec::new();
    let mut params_vec: Vec<String> = Vec::new();

    if let Some(start) = start_ts {
        clauses.push("start_ts >= ?");
        params_vec.push(start.to_string());
    }
    if let Some(end) = end_ts {
        clauses.push("end_ts <= ?");
        params_vec.push(end.to_string());
    }

    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }
    sql.push_str(" ORDER BY start_ts ASC");

    let mut stmt = conn.prepare(&sql)?;

    let rows = if params_vec.is_empty() {
        stmt.query_map([], |row| {
            map_routine_session_row(row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    } else if params_vec.len() == 1 {
        stmt.query_map([params_vec[0].as_str()], |row| {
            map_routine_session_row(row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        stmt.query_map(
            params![params_vec[0].as_str(), params_vec[1].as_str()],
            |row| map_routine_session_row(row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };

    Ok(rows)
}
