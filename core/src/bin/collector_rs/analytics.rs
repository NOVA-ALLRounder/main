use super::{support, AggregationConfig};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use rusqlite::{params, Connection};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Default)]
struct AppAggregate {
    event_count: u64,
    actions: HashMap<String, u64>,
}

pub(super) fn run_analytics_tick(db_path: &PathBuf, cfg: &AggregationConfig) -> anyhow::Result<()> {
    let conn = Connection::open(db_path)?;
    conn.busy_timeout(Duration::from_secs(5))?;

    ensure_analytics_tables(&conn)?;
    aggregate_last_five_minute_bucket(&conn, Utc::now())?;
    build_daily_summary_for_yesterday(&conn, Utc::now())?;
    cleanup_old_data(
        &conn,
        cfg.raw_retention_days,
        cfg.summary_retention_days,
        Utc::now(),
    )?;

    Ok(())
}

fn ensure_analytics_tables(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS minute_aggregates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            app TEXT NOT NULL,
            event_count INTEGER DEFAULT 0,
            actions_json TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(timestamp, app)
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS daily_summaries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT UNIQUE NOT NULL,
            total_events INTEGER DEFAULT 0,
            total_apps INTEGER DEFAULT 0,
            active_hours INTEGER DEFAULT 0,
            app_usage_json TEXT,
            top_actions_json TEXT,
            summary_text TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_ma_ts ON minute_aggregates(timestamp)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_ds_date ON daily_summaries(date)",
        [],
    )?;

    Ok(())
}

fn aggregate_last_five_minute_bucket(conn: &Connection, now: DateTime<Utc>) -> anyhow::Result<()> {
    let bucket_end = support::floor_to_five_minute_bucket(now);
    let bucket_start = bucket_end - ChronoDuration::minutes(5);

    let start_iso = support::format_iso_z(bucket_start);
    let end_iso = support::format_iso_z(bucket_end);

    let mut stmt = conn.prepare(
        "SELECT app, payload_json
         FROM events_v2
         WHERE datetime(ts) >= datetime(?1)
           AND datetime(ts) < datetime(?2)
         ORDER BY ts",
    )?;

    let rows = stmt.query_map(params![start_iso, end_iso], |row| {
        let app: String = row.get(0)?;
        let payload_json: String = row.get(1)?;
        Ok((app, payload_json))
    })?;

    let mut by_app: HashMap<String, AppAggregate> = HashMap::new();

    for row in rows {
        let (app, payload_json) = row?;
        let aggregate = by_app.entry(app).or_default();
        aggregate.event_count += 1;

        let payload: Value = serde_json::from_str(&payload_json).unwrap_or(Value::Null);
        if let Some(obj) = payload.as_object() {
            let control_type = obj
                .get("control_type")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let element_name = obj
                .get("element_name")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            if !control_type.is_empty() && !element_name.is_empty() {
                let action_key =
                    format!("{}:{}", control_type, support::truncate(element_name, 64));
                *aggregate.actions.entry(action_key).or_insert(0) += 1;
            }
        }
    }

    if by_app.is_empty() {
        return Ok(());
    }

    let bucket_label = bucket_start.format("%Y-%m-%d %H:%M").to_string();

    for (app, mut aggregate) in by_app {
        let mut top_actions: Vec<(String, u64)> = aggregate.actions.drain().collect();
        top_actions.sort_by(|a, b| b.1.cmp(&a.1));
        top_actions.truncate(5);
        let actions_json = serde_json::to_string(&top_actions)?;

        conn.execute(
            "INSERT OR REPLACE INTO minute_aggregates (timestamp, app, event_count, actions_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                bucket_label,
                app,
                aggregate.event_count as i64,
                actions_json
            ],
        )?;
    }

    Ok(())
}

fn build_daily_summary_for_yesterday(conn: &Connection, now: DateTime<Utc>) -> anyhow::Result<()> {
    let yesterday = (now - ChronoDuration::days(1))
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    build_daily_summary_for_date(conn, &yesterday)
}

fn build_daily_summary_for_date(conn: &Connection, date: &str) -> anyhow::Result<()> {
    let mut stmt = conn.prepare(
        "SELECT app, SUM(event_count) as total
         FROM minute_aggregates
         WHERE date(timestamp) = ?1
         GROUP BY app
         ORDER BY total DESC",
    )?;

    let rows = stmt.query_map(params![date], |row| {
        let app: String = row.get(0)?;
        let total: i64 = row.get(1)?;
        Ok((app, total.max(0) as u64))
    })?;

    let mut app_usage: Vec<(String, u64)> = Vec::new();
    let mut total_events = 0u64;

    for row in rows {
        let (app, total) = row?;
        total_events += total;
        app_usage.push((app, total));
    }

    if app_usage.is_empty() {
        return Ok(());
    }

    let mut actions_stmt = conn.prepare(
        "SELECT actions_json
         FROM minute_aggregates
         WHERE date(timestamp) = ?1",
    )?;

    let action_rows = actions_stmt.query_map(params![date], |row| row.get::<_, String>(0))?;

    let mut all_actions: HashMap<String, u64> = HashMap::new();
    for row in action_rows {
        let actions_json = row?;
        let parsed: Vec<(String, u64)> = serde_json::from_str(&actions_json).unwrap_or_default();
        for (action, count) in parsed {
            *all_actions.entry(action).or_insert(0) += count;
        }
    }

    let active_hours: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT substr(timestamp, 12, 2))
         FROM minute_aggregates
         WHERE date(timestamp) = ?1",
        params![date],
        |row| row.get(0),
    )?;

    app_usage.sort_by(|a, b| b.1.cmp(&a.1));
    let mut top_actions: Vec<(String, u64)> = all_actions.into_iter().collect();
    top_actions.sort_by(|a, b| b.1.cmp(&a.1));
    top_actions.truncate(20);

    let top_apps_display: Vec<(String, u64)> = app_usage.iter().take(5).cloned().collect();

    let summary_text = {
        let mut parts = vec![
            format!("date: {date}"),
            format!("total events: {total_events}"),
            format!("apps used: {}", app_usage.len()),
            format!("active hours: {}", active_hours.max(0)),
        ];
        if !top_apps_display.is_empty() {
            parts.push("top apps:".to_string());
            for (app, count) in &top_apps_display {
                parts.push(format!(
                    "- {}: {}",
                    support::normalize_app_for_display(app),
                    count
                ));
            }
        }
        parts.join("\n")
    };

    let app_usage_json = serde_json::to_string(&app_usage)?;
    let top_actions_json = serde_json::to_string(&top_actions)?;

    conn.execute(
        "INSERT INTO daily_summaries
            (date, total_events, total_apps, active_hours, app_usage_json, top_actions_json, summary_text)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(date) DO UPDATE SET
            total_events = excluded.total_events,
            total_apps = excluded.total_apps,
            active_hours = excluded.active_hours,
            app_usage_json = excluded.app_usage_json,
            top_actions_json = excluded.top_actions_json,
            summary_text = excluded.summary_text",
        params![
            date,
            total_events as i64,
            app_usage.len() as i64,
            active_hours,
            app_usage_json,
            top_actions_json,
            summary_text
        ],
    )?;

    Ok(())
}

fn cleanup_old_data(
    conn: &Connection,
    raw_retention_days: i64,
    summary_retention_days: i64,
    now: DateTime<Utc>,
) -> anyhow::Result<()> {
    let raw_cutoff = (now - ChronoDuration::days(raw_retention_days.max(1)))
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let summary_cutoff = (now - ChronoDuration::days(summary_retention_days.max(1)))
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();

    conn.execute(
        "DELETE FROM events_v2 WHERE date(ts) < ?1",
        params![raw_cutoff],
    )?;
    conn.execute(
        "DELETE FROM minute_aggregates WHERE date(timestamp) < ?1",
        params![summary_cutoff],
    )?;

    Ok(())
}
