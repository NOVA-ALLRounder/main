#![allow(dead_code)] // Allow unused library functions for future use
use rusqlite::{params, Connection, Result};

use crate::db_schema::ensure_column;
use crate::privacy::PrivacyGuard;
use crate::quality_scorer::QualityScore;
use crate::recommendation::AutomationProposal;
use lazy_static::lazy_static;
use std::sync::Mutex; // Added

#[path = "db/db_aux.rs"]
mod db_aux;
pub use db_aux::*;
#[path = "db/db_recommendation.rs"]
mod db_recommendation;
pub use db_recommendation::*;
#[path = "db/db_analytics.rs"]
mod db_analytics;
pub use db_analytics::*;
#[path = "db/db_routines.rs"]
mod db_routines;
pub use db_routines::*;
#[path = "db/db_exec.rs"]
mod db_exec;
pub use db_exec::*;

// Global DB connection (for MVP simplicity)
// In production, we should pass a connection pool or handle.
// But rusqlite Connection is not thread-safe, so we wrap in Mutex.
lazy_static! {
    static ref DB_CONN: Mutex<Option<Connection>> = Mutex::new(None);
}

/// Safe helper to acquire DB lock. Recovers from poisoned mutex.
pub(crate) fn get_db_lock() -> std::sync::MutexGuard<'static, Option<Connection>> {
    match DB_CONN.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("⚠️ DB Mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    }
}

pub fn init() -> anyhow::Result<()> {
    // [Paranoid Audit] Fix Connection Leak & Idempotency
    {
        let lock = get_db_lock();
        if lock.is_some() {
            // Already initialized, do nothing.
            return Ok(());
        }
    }

    // [Paranoid Audit] Path resolution with writable fallback
    let db_path = resolve_db_path()?;
    let conn = Connection::open(&db_path)?;
    println!("📦 Database initialized at: {:?}", db_path);

    // [Paranoid Audit] Set Busy Timeout to 5s to handle concurrency (Analyzer + API + Main)
    conn.busy_timeout(std::time::Duration::from_secs(5))?;

    // Legacy simple events table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            source TEXT NOT NULL,
            type TEXT NOT NULL,
            data TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS recommendations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at TEXT NOT NULL,
            status TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            trigger TEXT NOT NULL,
            actions TEXT NOT NULL,
            n8n_prompt TEXT NOT NULL,
            fingerprint TEXT NOT NULL UNIQUE,
            confidence REAL NOT NULL,
            workflow_id TEXT,
            workflow_json TEXT,
            approved_at TEXT,
            evidence TEXT NOT NULL DEFAULT '[]',
            last_error TEXT,
            pattern_id TEXT
        )",
        [],
    )?;

    // Create 'chat_history' table (New Memory System)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS chat_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL
        )",
        [],
    )?;

    // Create 'routines' table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS routines (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            cron_expression TEXT NOT NULL,
            prompt TEXT NOT NULL,
            enabled BOOLEAN NOT NULL DEFAULT 1,
            last_run TEXT,
            next_run TEXT,
            created_at TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS exec_approvals (
            id TEXT PRIMARY KEY,
            command TEXT NOT NULL,
            cwd TEXT,
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            status TEXT NOT NULL,
            decision TEXT,
            resolved_at TEXT,
            resolved_by TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS nl_approval_policies (
            policy_key TEXT PRIMARY KEY,
            decision TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS exec_allowlist (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            pattern TEXT NOT NULL,
            cwd TEXT,
            created_at TEXT NOT NULL,
            last_used_at TEXT,
            uses_count INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS exec_results (
            id TEXT PRIMARY KEY,
            command TEXT NOT NULL,
            cwd TEXT,
            status TEXT NOT NULL,
            output TEXT,
            error TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS learned_routines (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            steps_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS quality_scores (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at TEXT NOT NULL,
            overall REAL NOT NULL,
            breakdown TEXT NOT NULL,
            issues TEXT NOT NULL,
            strengths TEXT NOT NULL,
            recommendation TEXT NOT NULL,
            summary TEXT NOT NULL
        )",
        [],
    )?;

    // [Paranoid Audit] Performance Indexes - ADDED MISSING INDICES
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_chat_created ON chat_history(created_at)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_recs_created ON recommendations(created_at)",
        [],
    )?;
    // V2 Indices moved to init_v2 to ensure table exists

    conn.execute(
        "CREATE TABLE IF NOT EXISTS judgment_states (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            last_hash TEXT,
            consecutive_no_progress INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS release_baseline (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            created_at TEXT NOT NULL,
            baseline_json TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS verification_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at TEXT NOT NULL,
            kind TEXT NOT NULL,
            ok BOOLEAN NOT NULL,
            summary TEXT NOT NULL,
            details TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS routine_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            routine_id INTEGER NOT NULL,
            started_at TEXT NOT NULL,
            finished_at TEXT,
            status TEXT NOT NULL,
            error TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS nl_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at TEXT NOT NULL,
            intent TEXT NOT NULL,
            prompt TEXT NOT NULL,
            status TEXT NOT NULL,
            summary TEXT,
            details TEXT
        )",
        [],
    )?;

    // Store connection
    {
        let mut lock = get_db_lock();
        *lock = Some(conn);
    } // Lock is dropped here

    println!("📦 Database 'steer.db' initialized.");

    // Init V2 Schema
    {
        // Must release lock before calling init_v2 if it grabs lock?
        // Actually init_v2 grabs lock. But here we already dropped the lock scope in line 79.
    }
    if let Err(e) = init_v2() {
        eprintln!("Failed to init events_v2: {}", e);
    }
    if let Err(e) = init_sessions_table() {
        eprintln!("Failed to init sessions_v2: {}", e);
    }

    // Seed templates if needed (now safe to call)
    if let Err(e) = seed_advanced_examples() {
        eprintln!("Failed to seed templates: {}", e);
    }

    // [Migration] Ensure 'evidence' column exists
    if let Some(conn) = get_db_lock().as_mut() {
        ensure_column(
            conn,
            "recommendations",
            "evidence",
            "TEXT NOT NULL DEFAULT '[]'",
        );
        // [Migration] Phase 1 Context Enrichment
        ensure_column(conn, "events_v2", "window_title", "TEXT");
        ensure_column(conn, "events_v2", "browser_url", "TEXT");
        // [Migration] Phase 3 Final Polish
        ensure_column(conn, "recommendations", "pattern_id", "TEXT");
        ensure_column(conn, "recommendations", "last_error", "TEXT");
        ensure_column(conn, "exec_approvals", "decision", "TEXT");

        // 1-2. Routine Candidates Table
        if let Err(e) = conn.execute(
            "CREATE TABLE IF NOT EXISTS routine_candidates (
                candidate_id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                pattern_type TEXT NOT NULL,
                description TEXT,
                frequency INTEGER,
                score REAL,
                sample_events TEXT
            )",
            [],
        ) {
            eprintln!("Failed to create routine_candidates: {}", e);
        }
    }

    Ok(())
}

fn resolve_db_path() -> anyhow::Result<std::path::PathBuf> {
    if let Ok(raw) = std::env::var("STEER_DB_PATH") {
        let path = std::path::PathBuf::from(raw);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        return Ok(path);
    }

    let primary = crate::paths::app_dir().join("steer.db");
    if let Some(parent) = primary.parent() {
        if std::fs::create_dir_all(parent).is_ok() {
            return Ok(primary);
        }
    }

    let fallback = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".steer")
        .join("steer.db");
    if let Some(parent) = fallback.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(fallback)
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Recommendation {
    pub id: i64,
    pub status: String,
    pub title: String,
    pub summary: String,
    pub trigger: String,
    pub actions: Vec<String>,
    pub n8n_prompt: String,
    pub confidence: f64,
    pub workflow_id: Option<String>,
    pub workflow_json: Option<String>,
    pub evidence: Vec<String>,
    pub pattern_id: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RecommendationMetrics {
    pub total: i64,
    pub approved: i64,
    pub rejected: i64,
    pub failed: i64,
    pub pending: i64,
    pub later: i64,
    pub last_created_at: Option<String>,
}

pub fn insert_recommendation(proposal: &AutomationProposal) -> Result<bool> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        let actions_json =
            serde_json::to_string(&proposal.actions).unwrap_or_else(|_| "[]".to_string());
        let fingerprint = proposal.fingerprint();

        let rows = conn.execute(
            "INSERT OR IGNORE INTO recommendations (
                created_at, status, title, summary, trigger, actions, n8n_prompt, fingerprint, confidence, workflow_json, evidence, pattern_id, last_error
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                created_at,
                "pending",
                &proposal.title,
                &proposal.summary,
                &proposal.trigger,
                actions_json,
                &proposal.n8n_prompt,
                fingerprint,
                proposal.confidence,
                None::<String>, // No pre-filled JSON for auto-generated ones
                serde_json::to_string(&proposal.evidence).unwrap_or_else(|_| "[]".to_string()),
                proposal.pattern_id,
                None::<String>,
            ],
        )?;
        return Ok(rows > 0);
    }
    Ok(false)
}

pub fn count_recent_recommendations(hours: i64) -> Result<i64> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let cutoff = (chrono::Utc::now() - chrono::Duration::hours(hours)).to_rfc3339();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM recommendations WHERE created_at >= ?1",
            params![cutoff],
            |row| row.get(0),
        )?;
        return Ok(count);
    }
    Ok(0)
}

pub fn get_recommendation_metrics() -> Result<RecommendationMetrics> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT
                COUNT(*) as total,
                COALESCE(SUM(CASE WHEN status = 'approved' THEN 1 ELSE 0 END), 0) as approved,
                COALESCE(SUM(CASE WHEN status = 'rejected' THEN 1 ELSE 0 END), 0) as rejected,
                COALESCE(SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END), 0) as failed,
                COALESCE(SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END), 0) as pending,
                COALESCE(SUM(CASE WHEN status = 'later' THEN 1 ELSE 0 END), 0) as later,
                MAX(created_at) as last_created_at
             FROM recommendations",
        )?;

        let metrics = stmt.query_row([], |row| {
            Ok(RecommendationMetrics {
                total: row.get(0)?,
                approved: row.get(1)?,
                rejected: row.get(2)?,
                failed: row.get(3)?,
                pending: row.get(4)?,
                later: row.get(5)?,
                last_created_at: row.get(6).ok(),
            })
        })?;

        return Ok(metrics);
    }
    Ok(RecommendationMetrics {
        total: 0,
        approved: 0,
        rejected: 0,
        failed: 0,
        pending: 0,
        later: 0,
        last_created_at: None,
    })
}

// Function to seed advanced examples if DB is empty
pub fn seed_advanced_examples() -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        // Check if any recommendations exist
        let count: i64 =
            conn.query_row("SELECT count(*) FROM recommendations", [], |row| row.get(0))?;

        if count > 0 {
            return Ok(());
        }

        println!("🌱 Seeding advanced workflow templates...");

        let created_at = chrono::Utc::now().to_rfc3339();

        // Example 1: Morning Briefing
        let briefing_json = r#"{
            "name": "Daily Morning Briefing",
            "nodes": [
                { "type": "n8n-nodes-base.cron", "typeVersion": 1, "position": [100, 300], "parameters": { "triggerTimes": { "item": [{ "mode": "everyDay", "hour": 9 }] } }, "name": "Schedule (9 AM)" },
                { "type": "n8n-nodes-base.googleCalendar", "typeVersion": 1, "position": [300, 300], "parameters": { "operation": "getAll", "calendar": { "__rl": true, "mode": "list", "value": "primary" }, "options": { "timeMin": "={{ $today }}", "timeMax": "={{ $today.end }}" } }, "name": "Get Appointments" },
                { "type": "n8n-nodes-base.openAi", "typeVersion": 1, "position": [500, 300], "parameters": { "resource": "chat", "prompt": { "messages": [{ "role": "user", "content": "Summarize my day based on these events: {{ JSON.stringify($json) }}" }] } }, "name": "AI Summary" },
                { "type": "n8n-nodes-base.telegram", "typeVersion": 1, "position": [700, 300], "parameters": { "chatId": "YOUR_CHAT_ID", "text": "🌞 *Morning Briefing*\n\n{{ $json.message.content }}", "additionalFields": { "parseMode": "Markdown" } }, "name": "Send to Telegram" }
            ],
            "connections": {
                "Schedule (9 AM)": { "main": [[{ "node": "Get Appointments", "type": "main", "index": 0 }]] },
                "Get Appointments": { "main": [[{ "node": "AI Summary", "type": "main", "index": 0 }]] },
                "AI Summary": { "main": [[{ "node": "Send to Telegram", "type": "main", "index": 0 }]] }
            }
        }"#;

        conn.execute(
            "INSERT INTO recommendations (
                created_at, status, title, summary, trigger, actions, n8n_prompt, fingerprint, confidence, workflow_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                created_at,
                "pending",
                "🌞 Daily Morning Briefing",
                "매일 아침 9시에 일정과 날씨를 요약해서 텔레그램으로 보냅니다.",
                "Every Day at 9:00 AM",
                "[\"Calendar Check\", \"AI Summary\", \"Telegram Notify\"]",
                "Create a workflow that runs every day at 9am, fetches Google Calendar events, summarizes them using OpenAI, and sends it to Telegram.",
                "seed_briefing_001",
                1.0,
                briefing_json
            ],
        )?;

        // Example 2: Urgent Email Alert
        let urgent_mail_json = r#"{
            "name": "Urgent Email Alert",
            "nodes": [
                { "type": "n8n-nodes-base.gmail", "typeVersion": 2, "position": [100, 300], "parameters": { "pollTimes": { "item": [{ "mode": "everyMinute" }] }, "filters": { "labelIds": ["INBOX"], "readStatus": "unread" } }, "name": "Check Inbox" },
                { "type": "n8n-nodes-base.if", "typeVersion": 1, "position": [300, 300], "parameters": { "conditions": { "string": [{ "value1": "={{ $json.snippet }}", "operation": "contains", "value2": "urgent" }, { "value1": "={{ $json.subject }}", "operation": "contains", "value2": "긴급" }] }, "combineOperation": "any" }, "name": "Is Urgent?" },
                { "type": "n8n-nodes-base.telegram", "typeVersion": 1, "position": [500, 200], "parameters": { "chatId": "YOUR_CHAT_ID", "text": "🚨 *Urgent Email*\n\nFrom: {{ $json.from }}\nSubject: {{ $json.subject }}\nSnippet: {{ $json.snippet }}" }, "name": "Notify Telegram" }
            ],
            "connections": {
                "Check Inbox": { "main": [[{ "node": "Is Urgent?", "type": "main", "index": 0 }]] },
                "Is Urgent?": { "main": [[{ "node": "Notify Telegram", "type": "main", "index": 0 }]] }
            }
        }"#;

        conn.execute(
            "INSERT INTO recommendations (
                created_at, status, title, summary, trigger, actions, n8n_prompt, fingerprint, confidence, workflow_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                created_at,
                "pending",
                "🚨 긴급 메일 알림",
                "제목이나 내용에 '긴급'이 포함된 메일이 오면 즉시 알림을 보냅니다.",
                "New Email in Inbox",
                "[\"Check Keywords\", \"Telegram Alert\"]",
                "Watch Gmail for new emails. If subject contains 'urgent' or '긴급', send a Telegram notification.",
                "seed_urgent_001",
                0.95,
                urgent_mail_json
            ],
        )?;
    }
    Ok(())
}

// --- V2 Event Ingestion (Matches Python Schema) ---

pub fn init_v2() -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
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

        // [Paranoid Audit] Add Index for V2 Events
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_events_v2_ts ON events_v2(ts)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_events_v2_type ON events_v2(event_type)",
            [],
        )?;
    }
    Ok(())
}

pub fn insert_event_v2(envelope: &crate::schema::EventEnvelope) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let payload_json = serde_json::to_string(&envelope.payload).unwrap_or_default();
        let privacy_json = serde_json::to_string(&envelope.privacy).unwrap_or_default();
        let raw_json = serde_json::to_string(&envelope.raw).unwrap_or_default();

        let (res_type, res_id) = match &envelope.resource {
            Some(r) => (r.resource_type.clone(), r.id.clone()),
            None => ("".to_string(), "".to_string()),
        };

        // Initialize PrivacyGuard (Local scope to safeguard data)
        let salt = std::env::var("PRIVACY_SALT").unwrap_or_else(|_| "default_salt".to_string());
        let guard = PrivacyGuard::new(salt);

        // Mask specific fields
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
                window_title, // Masked
                browser_url,  // Masked
                raw_json
            ],
        )?;
    }
    Ok(())
}

pub fn insert_events_v2_batch(events: &[crate::schema::EventEnvelope]) -> Result<usize> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        if events.is_empty() {
            return Ok(0);
        }

        // Reuse one guard and one prepared statement for the whole batch.
        let salt = std::env::var("PRIVACY_SALT").unwrap_or_else(|_| "default_salt".to_string());
        let guard = PrivacyGuard::new(salt);
        let tx = conn.transaction()?;
        let mut inserted = 0usize;

        {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO events_v2 (
                    schema_version, event_id, ts, source, app, event_type, priority,
                    resource_type, resource_id, payload_json, privacy_json, pid, window_id, window_title, browser_url, raw_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            )?;

            for envelope in events {
                let payload_json = serde_json::to_string(&envelope.payload).unwrap_or_default();
                let privacy_json = serde_json::to_string(&envelope.privacy).unwrap_or_default();
                let raw_json = serde_json::to_string(&envelope.raw).unwrap_or_default();

                let (res_type, res_id) = match &envelope.resource {
                    Some(r) => (r.resource_type.clone(), r.id.clone()),
                    None => ("".to_string(), "".to_string()),
                };

                let window_title = envelope
                    .window_title
                    .as_ref()
                    .map(|t| guard.mask_sensitive_text(t));
                let browser_url = envelope
                    .browser_url
                    .as_ref()
                    .map(|u| guard.mask_sensitive_text(u));

                let changed = stmt.execute(params![
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
                ])?;
                inserted += changed as usize;
            }
        }

        tx.commit()?;
        Ok(inserted)
    } else {
        Ok(0)
    }
}

// Add Sessions Table
pub fn init_sessions_table() -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
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
    }
    Ok(())
}

pub fn fetch_all_events_v2(limit: i64) -> Result<Vec<crate::schema::EventEnvelope>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT schema_version, event_id, ts, source, app, event_type, priority,
             resource_type, resource_id, payload_json, privacy_json, pid, window_id, window_title, browser_url, raw_json
             FROM events_v2 ORDER BY ts ASC LIMIT ?1"
        )?;

        let rows = stmt.query_map([limit], |row| {
            let payload_str: String = row.get(9)?;
            let privacy_str: String = row.get(10)?;
            let raw_str: String = row.get(15)?;
            let res_type: String = row.get(7)?;
            let res_id: String = row.get(8)?;

            let payload = serde_json::from_str(&payload_str).unwrap_or(serde_json::Value::Null);
            let privacy = serde_json::from_str(&privacy_str).ok();
            let raw = serde_json::from_str(&raw_str).ok();
            let resource = if res_type.is_empty() && res_id.is_empty() {
                None
            } else {
                Some(crate::schema::ResourceContext {
                    resource_type: res_type,
                    id: res_id,
                })
            };

            Ok(crate::schema::EventEnvelope {
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
        })?;

        let mut events = Vec::new();
        for r in rows {
            events.push(r?);
        }
        Ok(events)
    } else {
        Ok(Vec::new())
    }
}

pub fn insert_session(session: &crate::session::SessionRecord) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
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
    }
    Ok(())
}

// Legacy simple insert (kept for backward compat during migration)
pub fn insert_event(event_json: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        // Parse basic fields
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(event_json) {
            let timestamp_str = value["timestamp"].as_str().unwrap_or("");
            let timestamp = if timestamp_str.is_empty() {
                chrono::Utc::now().to_rfc3339()
            } else {
                timestamp_str.to_string()
            };

            let source = value["source"].as_str().unwrap_or("unknown");
            let type_ = value["type"].as_str().unwrap_or("unknown");
            // Store full JSON in data
            let data = event_json;

            conn.execute(
                "INSERT INTO events (timestamp, source, type, data) VALUES (?1, ?2, ?3, ?4)",
                params![timestamp, source, type_, data],
            )?;
        }
    }
    Ok(())
}

pub fn get_recent_events(cutoff_hours: i64) -> anyhow::Result<Vec<String>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let cutoff = (chrono::Utc::now() - chrono::Duration::hours(cutoff_hours)).to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT schema_version, event_id, ts, source, app, event_type, priority,
             resource_type, resource_id, payload_json, privacy_json, pid, window_id, window_title, browser_url, raw_json
             FROM events_v2 WHERE ts >= ?1 ORDER BY ts ASC"
        )?;

        let rows = stmt.query_map([cutoff.clone()], |row| {
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

            let envelope = crate::schema::EventEnvelope {
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
            };

            Ok(serde_json::to_string(&envelope).unwrap_or_default())
        })?;

        let mut events = Vec::new();
        for event in rows {
            events.push(event?);
        }

        // [Paranoid Audit] Merge Legacy Events correctly
        let mut stmt = conn.prepare(
            "SELECT data FROM events
             WHERE timestamp >= ?1
             ORDER BY timestamp ASC",
        )?;

        let rows = stmt.query_map(params![&cutoff], |row| row.get::<_, String>(0))?;
        for event in rows {
            if let Ok(json) = event {
                events.push(json);
            }
        }

        // Sort merged events by timestamp to be safe (though usually appended logic works if legacy is old)
        // But here we rely on the fact that legacy is old.
        // If we want true sort, we need to parse JSON.
        // For MVP harding, we assume legacy is strictly older or concurrent?
        // Actually, let's just append. Data from V2 is recent.
        // Wait, if V2 is empty, we get legacy. If V2 has data, we ALSO get legacy?
        // Yes, we want full history for the window.

        return Ok(events);
    }
    Err(anyhow::anyhow!("DB not initialized").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_creates_table() {
        let result = init();
        assert!(result.is_ok());
    }

    #[test]
    fn test_insert_event() {
        init().ok(); // Might error if already init

        let test_event = r#"{"type":"test","source":"unit_test"}"#;
        let insert_result = insert_event(test_event);
        assert!(insert_result.is_ok());
    }
}
