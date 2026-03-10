use rusqlite::{params, Result};

use crate::{schema::EventEnvelope, session::SessionRecord};

use super::{with_read_conn, with_write_conn_if_available};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub created_at: String,
}

fn map_event_envelope_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventEnvelope> {
    let payload_str: String = row.get(9)?;
    let privacy_str: String = row.get(10)?;
    let raw_str: String = row.get(15)?;

    let payload = serde_json::from_str(&payload_str).unwrap_or(serde_json::Value::Null);
    let privacy = serde_json::from_str(&privacy_str).ok();
    let raw = serde_json::from_str(&raw_str).ok();
    let res_type: String = row.get(7)?;
    let res_id: String = row.get(8)?;
    let resource = if res_type.is_empty() && res_id.is_empty() {
        None
    } else {
        Some(crate::schema::ResourceContext {
            resource_type: res_type,
            id: res_id,
        })
    };

    Ok(EventEnvelope {
        schema_version: row.get(0)?,
        event_id: row.get(1)?,
        ts: row.get(2)?,
        source: row.get(3)?,
        app: row.get(4)?,
        event_type: row.get(5)?,
        priority: row.get(6)?,
        resource,
        payload,
        privacy,
        pid: row.get(11)?,
        window_id: row.get(12)?,
        window_title: row.get(13).ok(),
        browser_url: row.get(14).ok(),
        raw,
    })
}

pub fn init_v2() -> Result<()> {
    with_write_conn_if_available(|conn| {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS events_v2 (
                schema_version TEXT,
                event_id TEXT PRIMARY KEY,
                ts TEXT NOT NULL,
                source TEXT NOT NULL,
                app TEXT NOT NULL,
                event_type TEXT NOT NULL,
                priority TEXT,
                resource_type TEXT,
                resource_id TEXT,
                payload_json TEXT,
                privacy_json TEXT,
                pid INTEGER,
                window_id TEXT,
                window_title TEXT,
                browser_url TEXT,
                raw_json TEXT
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_events_v2_ts ON events_v2(ts)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_events_v2_type ON events_v2(event_type)",
            [],
        )?;
        Ok(())
    })?;
    Ok(())
}

pub fn insert_event_v2(envelope: &EventEnvelope) -> Result<()> {
    with_write_conn_if_available(|conn| {
        let payload_json = serde_json::to_string(&envelope.payload).unwrap_or_default();
        let privacy_json = serde_json::to_string(&envelope.privacy).unwrap_or_default();
        let raw_json = serde_json::to_string(&envelope.raw).unwrap_or_default();

        let (res_type, res_id) = match &envelope.resource {
            Some(r) => (r.resource_type.clone(), r.id.clone()),
            None => ("".to_string(), "".to_string()),
        };

        let salt = std::env::var("PRIVACY_SALT").unwrap_or_else(|_| "default_salt".to_string());
        let guard = crate::privacy::PrivacyGuard::new(salt);

        let window_title = envelope
            .window_title
            .as_ref()
            .map(|t| guard.mask_sensitive_text(t));
        let browser_url = envelope
            .browser_url
            .as_ref()
            .map(|u| guard.mask_sensitive_text(u));

        conn.execute(
            "INSERT OR IGNORE INTO events_v2 (
                schema_version, event_id, ts, source, app, event_type, priority,
                resource_type, resource_id, payload_json, privacy_json, pid, window_id, window_title, browser_url, raw_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                envelope.schema_version,
                envelope.event_id,
                envelope.ts,
                envelope.source,
                envelope.app,
                envelope.event_type,
                envelope.priority,
                res_type,
                res_id,
                payload_json,
                privacy_json,
                envelope.pid,
                envelope.window_id,
                window_title,
                browser_url,
                raw_json
            ],
        )?;
        Ok(())
    })?;
    Ok(())
}

pub fn init_sessions_table() -> Result<()> {
    with_write_conn_if_available(|conn| {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions_v2 (
                session_id TEXT PRIMARY KEY,
                start_ts TEXT NOT NULL,
                end_ts TEXT NOT NULL,
                duration_sec INTEGER,
                summary_json TEXT
            )",
            [],
        )?;
        Ok(())
    })?;
    Ok(())
}

pub fn fetch_all_events_v2(limit: i64) -> Result<Vec<EventEnvelope>> {
    match with_read_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT schema_version, event_id, ts, source, app, event_type, priority,
             resource_type, resource_id, payload_json, privacy_json, pid, window_id, window_title, browser_url, raw_json
             FROM events_v2 ORDER BY ts ASC LIMIT ?1",
        )?;

        let rows = stmt.query_map([limit], map_event_envelope_row)?;

        let mut events = Vec::new();
        for r in rows {
            events.push(r?);
        }
        Ok(events)
    }) {
        Ok(events) => Ok(events),
        Err(rusqlite::Error::InvalidQuery) => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

pub fn insert_session(session: &SessionRecord) -> Result<()> {
    with_write_conn_if_available(|conn| {
        let summary_json = serde_json::to_string(&session.summary).unwrap_or_default();
        conn.execute(
            "INSERT INTO sessions_v2 (session_id, start_ts, end_ts, duration_sec, summary_json)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session.session_id,
                session.start_ts,
                session.end_ts,
                session.duration_sec,
                summary_json
            ],
        )?;
        Ok(())
    })?;
    Ok(())
}

pub fn insert_event(event_json: &str) -> Result<()> {
    with_write_conn_if_available(|conn| {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(event_json) {
            let timestamp_str = value["timestamp"].as_str().unwrap_or("");
            let timestamp = if timestamp_str.is_empty() {
                chrono::Utc::now().to_rfc3339()
            } else {
                timestamp_str.to_string()
            };

            let source = value["source"].as_str().unwrap_or("unknown");
            let type_ = value["type"].as_str().unwrap_or("unknown");
            conn.execute(
                "INSERT INTO events (timestamp, source, type, data) VALUES (?1, ?2, ?3, ?4)",
                params![timestamp, source, type_, event_json],
            )?;
        }
        Ok(())
    })?;
    Ok(())
}

pub fn get_recent_events(cutoff_hours: i64) -> anyhow::Result<Vec<String>> {
    with_read_conn(|conn| {
        let cutoff = (chrono::Utc::now() - chrono::Duration::hours(cutoff_hours)).to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT schema_version, event_id, ts, source, app, event_type, priority,
             resource_type, resource_id, payload_json, privacy_json, pid, window_id, window_title, browser_url, raw_json
             FROM events_v2 WHERE ts >= ?1 ORDER BY ts ASC",
        )?;

        let rows = stmt.query_map([cutoff.clone()], |row| {
            let envelope = map_event_envelope_row(row)?;
            Ok(serde_json::to_string(&envelope).unwrap_or_default())
        })?;

        let mut events = Vec::new();
        for event in rows {
            events.push(event?);
        }

        let mut stmt = conn.prepare(
            "SELECT data FROM events
             WHERE timestamp >= ?1
             ORDER BY timestamp ASC",
        )?;

        let rows = stmt.query_map(params![&cutoff], |row| row.get::<_, String>(0))?;
        for json in rows.flatten() {
            events.push(json);
        }

        Ok(events)
    })
    .map_err(anyhow::Error::from)
}

pub fn insert_chat_message(role: &str, content: &str) -> Result<()> {
    with_write_conn_if_available(|conn| {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO chat_history (role, content, created_at) VALUES (?1, ?2, ?3)",
            params![role, content, created_at],
        )?;
        Ok(())
    })?;
    Ok(())
}

pub fn get_recent_chat_history(limit: i64) -> Result<Vec<ChatMessage>> {
    match with_read_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT role, content, created_at FROM chat_history ORDER BY created_at DESC LIMIT ?1",
        )?;

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
    }) {
        Ok(history) => Ok(history),
        Err(rusqlite::Error::InvalidQuery) => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}
