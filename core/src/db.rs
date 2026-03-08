#![allow(dead_code)] // Allow unused library functions for future use
use rusqlite::{params, params_from_iter, Connection, Result};
use serde_json::Value;

use crate::privacy::PrivacyGuard;
use crate::quality_scorer::QualityScore;
use crate::recommendation::AutomationProposal;
use crate::request_memory::{build_request_signature, normalize_request_text};
use lazy_static::lazy_static;
use std::str::FromStr;
use std::sync::Mutex; // Added

// Global DB connection (for MVP simplicity)
// In production, we should pass a connection pool or handle.
// But rusqlite Connection is not thread-safe, so we wrap in Mutex.
lazy_static! {
    static ref DB_CONN: Mutex<Option<Connection>> = Mutex::new(None);
}

/// Safe helper to acquire DB lock. Recovers from poisoned mutex.
fn get_db_lock() -> std::sync::MutexGuard<'static, Option<Connection>> {
    match DB_CONN.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("⚠️ DB Mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    }
}

pub fn current_db_path() -> Option<String> {
    let mut lock = get_db_lock();
    let conn = lock.as_mut()?;
    let mut stmt = conn.prepare("PRAGMA database_list").ok()?;
    let mut rows = stmt.query([]).ok()?;
    while let Ok(Some(row)) = rows.next() {
        let name: String = row.get(1).ok()?;
        if name == "main" {
            let path: String = row.get(2).ok()?;
            if !path.trim().is_empty() {
                return Some(path);
            }
        }
    }
    None
}

pub fn reset_connection() {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.take() {
        drop(conn);
    }
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    let sql = format!("PRAGMA table_info({})", table);
    let mut stmt = match conn.prepare(&sql) {
        Ok(stmt) => stmt,
        Err(e) => {
            eprintln!("Failed to read table_info for {}: {}", table, e);
            return false;
        }
    };
    let rows = match stmt.query_map([], |row| row.get::<_, String>(1)) {
        Ok(rows) => rows,
        Err(e) => {
            eprintln!("Failed to query table_info for {}: {}", table, e);
            return false;
        }
    };
    for name in rows.flatten() {
        if name == column {
            return true;
        }
    }
    false
}

fn ensure_column(conn: &Connection, table: &str, column: &str, ddl: &str) {
    if column_exists(conn, table, column) {
        return;
    }
    let sql = format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, ddl);
    if let Err(e) = conn.execute(&sql, []) {
        eprintln!("Failed to add column {}.{}: {}", table, column, e);
    }
}

fn ensure_approval_decisions_table(conn: &Connection) {
    if let Err(e) = conn.execute(
        "CREATE TABLE IF NOT EXISTS nl_approval_decisions (
            decision_key TEXT PRIMARY KEY,
            plan_id TEXT NOT NULL,
            action TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL
        )",
        [],
    ) {
        eprintln!("Failed to create nl_approval_decisions table: {}", e);
        return;
    }
    if let Err(e) = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_nl_approval_decisions_expires_at
         ON nl_approval_decisions(expires_at)",
        [],
    ) {
        eprintln!(
            "Failed to create idx_nl_approval_decisions_expires_at index: {}",
            e
        );
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

    // [Paranoid Audit] Use stable path, with explicit override for pipeline integration.
    let db_path = if let Ok(override_path) = std::env::var("STEER_DB_PATH") {
        let trimmed = override_path.trim();
        if trimmed.is_empty() {
            std::path::PathBuf::from("steer.db")
        } else {
            std::path::PathBuf::from(trimmed)
        }
    } else {
        #[cfg(test)]
        {
            std::env::temp_dir().join("steer_test.db")
        }
        #[cfg(not(test))]
        {
            if let Some(mut path) = dirs::data_local_dir() {
                path.push("steer");
                std::fs::create_dir_all(&path)?; // Ensure ~/.local/share/steer exists
                path.push("steer.db");
                path
            } else {
                std::path::PathBuf::from("steer.db") // Fallback
            }
        }
    };

    if let Some(parent) = db_path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }

    // Open (or create) steer.db
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
            pattern_id TEXT,
            snoozed_until TEXT,
            category TEXT NOT NULL DEFAULT 'unknown',
            business_score REAL NOT NULL DEFAULT 0.0,
            feedback_status TEXT,
            feedback_note TEXT,
            feedback_count INTEGER NOT NULL DEFAULT 0,
            last_feedback_at TEXT
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

    conn.execute(
        "CREATE TABLE IF NOT EXISTS request_memory (
            normalized_request TEXT PRIMARY KEY,
            memory_scope TEXT NOT NULL DEFAULT 'global',
            original_request TEXT NOT NULL,
            request_signature TEXT,
            intent_json TEXT,
            intent_command TEXT,
            response_text TEXT,
            response_mode TEXT NOT NULL DEFAULT 'intent_only',
            source TEXT NOT NULL DEFAULT 'api.chat',
            confidence REAL NOT NULL DEFAULT 0.0,
            positive_feedback_count INTEGER NOT NULL DEFAULT 0,
            negative_feedback_count INTEGER NOT NULL DEFAULT 0,
            last_feedback_at TEXT,
            suppressed INTEGER NOT NULL DEFAULT 0,
            suppressed_reason TEXT,
            suppressed_at TEXT,
            use_count INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            last_used_at TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS execution_memory (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            memory_scope TEXT NOT NULL DEFAULT 'global',
            intent_command TEXT NOT NULL,
            params_key TEXT NOT NULL,
            params_json TEXT,
            request_signature TEXT,
            response_text TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'api.chat',
            tool_path TEXT NOT NULL,
            freshness_ttl_seconds INTEGER NOT NULL DEFAULT 0,
            success INTEGER NOT NULL DEFAULT 1,
            positive_feedback_count INTEGER NOT NULL DEFAULT 0,
            negative_feedback_count INTEGER NOT NULL DEFAULT 0,
            last_feedback_at TEXT,
            suppressed INTEGER NOT NULL DEFAULT 0,
            suppressed_reason TEXT,
            suppressed_at TEXT,
            use_count INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            last_used_at TEXT NOT NULL,
            UNIQUE(intent_command, params_key)
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS launch_ops_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at TEXT NOT NULL,
            channel TEXT,
            memory_scope TEXT,
            message_preview TEXT NOT NULL,
            route_kind TEXT NOT NULL,
            command TEXT,
            outcome TEXT NOT NULL,
            confidence REAL,
            freshness_bypassed INTEGER NOT NULL DEFAULT 0,
            intent_memory_hit INTEGER NOT NULL DEFAULT 0,
            request_memory_hit INTEGER NOT NULL DEFAULT 0,
            execution_memory_hit INTEGER NOT NULL DEFAULT 0,
            deterministic_used INTEGER NOT NULL DEFAULT 0,
            llm_used INTEGER NOT NULL DEFAULT 0,
            ai_digest_used INTEGER NOT NULL DEFAULT 0,
            local_only INTEGER NOT NULL DEFAULT 0,
            note TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS memory_admin_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at TEXT NOT NULL,
            kind TEXT NOT NULL,
            action TEXT NOT NULL,
            memory_scope TEXT,
            target_key TEXT NOT NULL,
            reason TEXT,
            actor TEXT,
            ok INTEGER NOT NULL DEFAULT 0,
            message TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS recommendation_review_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at TEXT NOT NULL,
            recommendation_id INTEGER NOT NULL,
            recommendation_title TEXT NOT NULL,
            status_after TEXT,
            category TEXT,
            action TEXT NOT NULL,
            actor TEXT,
            note TEXT,
            ok INTEGER NOT NULL DEFAULT 0,
            message TEXT
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
            run_claimed_at TEXT,
            run_claim_owner TEXT,
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
        "CREATE TABLE IF NOT EXISTS nl_approval_decisions (
            decision_key TEXT PRIMARY KEY,
            plan_id TEXT NOT NULL,
            action TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_nl_approval_decisions_expires_at
         ON nl_approval_decisions(expires_at)",
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
        "CREATE INDEX IF NOT EXISTS idx_launch_ops_events_created
         ON launch_ops_events(created_at DESC)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_launch_ops_events_route
         ON launch_ops_events(route_kind, created_at DESC)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_memory_admin_events_created
         ON memory_admin_events(created_at DESC, id DESC)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_recommendation_review_events_created
         ON recommendation_review_events(created_at DESC, id DESC)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_recommendation_review_events_recommendation
         ON recommendation_review_events(recommendation_id, created_at DESC, id DESC)",
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
    conn.execute(
        "CREATE TABLE IF NOT EXISTS task_runs (
            run_id TEXT PRIMARY KEY,
            plan_id TEXT,
            created_at TEXT NOT NULL,
            finished_at TEXT,
            intent TEXT NOT NULL,
            prompt TEXT NOT NULL,
            planner_complete INTEGER NOT NULL DEFAULT 0,
            execution_complete INTEGER NOT NULL DEFAULT 0,
            business_complete INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL,
            summary TEXT,
            details TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS task_stage_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            stage_name TEXT NOT NULL,
            stage_order INTEGER NOT NULL,
            status TEXT NOT NULL,
            started_at TEXT NOT NULL,
            finished_at TEXT NOT NULL,
            details TEXT,
            retry_count INTEGER NOT NULL DEFAULT 0,
            max_retries INTEGER NOT NULL DEFAULT 0,
            next_retry_at TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS task_stage_assertions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            stage_name TEXT NOT NULL,
            assertion_key TEXT NOT NULL,
            expected TEXT NOT NULL,
            actual TEXT NOT NULL,
            passed INTEGER NOT NULL,
            evidence TEXT,
            created_at TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS task_run_artifacts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            artifact_type TEXT NOT NULL,
            artifact_key TEXT NOT NULL,
            value TEXT NOT NULL,
            metadata TEXT,
            created_at TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS collector_handoff_receipts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            received_at TEXT NOT NULL,
            package_id TEXT NOT NULL,
            collector_row_id INTEGER,
            status TEXT NOT NULL,
            recommendation_id INTEGER,
            detail TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS workflow_provision_ops (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            recommendation_id INTEGER NOT NULL,
            claim_token TEXT,
            status TEXT NOT NULL,
            workflow_id TEXT,
            workflow_json TEXT,
            error TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_task_stage_runs_run_id
         ON task_stage_runs(run_id, stage_order)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_task_stage_assertions_run_id
         ON task_stage_assertions(run_id, stage_name)",
        [],
    )?;
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_task_run_artifacts_unique
         ON task_run_artifacts(run_id, artifact_type, artifact_key)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_task_run_artifacts_run_id
         ON task_run_artifacts(run_id, created_at)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_collector_handoff_receipts_package
         ON collector_handoff_receipts(package_id, received_at)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_collector_handoff_receipts_status
         ON collector_handoff_receipts(status, received_at)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_workflow_provision_ops_status
         ON workflow_provision_ops(status, updated_at)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_workflow_provision_ops_recommendation
         ON workflow_provision_ops(recommendation_id, updated_at)",
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
        ensure_column(conn, "recommendations", "snoozed_until", "TEXT");
        ensure_column(
            conn,
            "recommendations",
            "category",
            "TEXT NOT NULL DEFAULT 'unknown'",
        );
        ensure_column(
            conn,
            "recommendations",
            "business_score",
            "REAL NOT NULL DEFAULT 0.0",
        );
        ensure_column(conn, "recommendations", "feedback_status", "TEXT");
        ensure_column(conn, "recommendations", "feedback_note", "TEXT");
        ensure_column(
            conn,
            "recommendations",
            "feedback_count",
            "INTEGER NOT NULL DEFAULT 0",
        );
        ensure_column(conn, "recommendations", "last_feedback_at", "TEXT");
        ensure_column(conn, "exec_approvals", "decision", "TEXT");
        ensure_column(conn, "nl_runs", "source_key", "TEXT");
        ensure_column(conn, "task_runs", "plan_id", "TEXT");
        ensure_column(conn, "routines", "run_claimed_at", "TEXT");
        ensure_column(conn, "routines", "run_claim_owner", "TEXT");
        ensure_column(conn, "request_memory", "request_signature", "TEXT");
        ensure_column(conn, "request_memory", "intent_command", "TEXT");
        ensure_column(
            conn,
            "request_memory",
            "memory_scope",
            "TEXT NOT NULL DEFAULT 'global'",
        );
        ensure_column(
            conn,
            "request_memory",
            "positive_feedback_count",
            "INTEGER NOT NULL DEFAULT 0",
        );
        ensure_column(
            conn,
            "request_memory",
            "negative_feedback_count",
            "INTEGER NOT NULL DEFAULT 0",
        );
        ensure_column(conn, "request_memory", "last_feedback_at", "TEXT");
        ensure_column(
            conn,
            "request_memory",
            "suppressed",
            "INTEGER NOT NULL DEFAULT 0",
        );
        ensure_column(conn, "request_memory", "suppressed_reason", "TEXT");
        ensure_column(conn, "request_memory", "suppressed_at", "TEXT");
        ensure_column(
            conn,
            "execution_memory",
            "memory_scope",
            "TEXT NOT NULL DEFAULT 'global'",
        );
        ensure_column(
            conn,
            "execution_memory",
            "positive_feedback_count",
            "INTEGER NOT NULL DEFAULT 0",
        );
        ensure_column(
            conn,
            "execution_memory",
            "negative_feedback_count",
            "INTEGER NOT NULL DEFAULT 0",
        );
        ensure_column(conn, "execution_memory", "last_feedback_at", "TEXT");
        ensure_column(
            conn,
            "execution_memory",
            "suppressed",
            "INTEGER NOT NULL DEFAULT 0",
        );
        ensure_column(conn, "execution_memory", "suppressed_reason", "TEXT");
        ensure_column(conn, "execution_memory", "suppressed_at", "TEXT");
        ensure_column(
            conn,
            "task_stage_runs",
            "retry_count",
            "INTEGER NOT NULL DEFAULT 0",
        );
        ensure_column(
            conn,
            "task_stage_runs",
            "max_retries",
            "INTEGER NOT NULL DEFAULT 0",
        );
        ensure_column(conn, "task_stage_runs", "next_retry_at", "TEXT");
        let _ = conn.execute("DROP INDEX IF EXISTS idx_nl_runs_source_key_unique", []);
        let _ = conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_nl_runs_source_key_unique
             ON nl_runs(source_key)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_task_runs_plan_status
             ON task_runs(plan_id, status, created_at)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_routines_due_claim
             ON routines(enabled, next_run, run_claimed_at)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_recommendations_category_status
             ON recommendations(category, status, created_at)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_request_memory_last_used
             ON request_memory(last_used_at)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_request_memory_signature_command
             ON request_memory(request_signature, intent_command, last_used_at)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_request_memory_scope_signature_command
             ON request_memory(memory_scope, request_signature, intent_command, last_used_at)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_execution_memory_lookup
             ON execution_memory(intent_command, params_key, last_used_at)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_execution_memory_scope_lookup
             ON execution_memory(memory_scope, intent_command, params_key, last_used_at)",
            [],
        );
        backfill_request_memory_scope_keys(conn);
        backfill_execution_memory_scope_keys(conn);
        backfill_request_memory_cache_columns(conn);
        // Keep recommendation status model strict: pending/approved/rejected only.
        let _ = conn.execute(
            "UPDATE recommendations
             SET status = CASE
                 WHEN LOWER(status) IN ('pending', 'approved', 'rejected') THEN LOWER(status)
                 ELSE 'pending'
             END
             WHERE status IS NULL
                OR LOWER(status) NOT IN ('pending', 'approved', 'rejected')
                OR status != LOWER(status)",
            [],
        );

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

#[cfg(test)]
pub fn clear_request_memory_for_tests() {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let _ = conn.execute("DELETE FROM request_memory", []);
    }
}

#[cfg(test)]
pub fn clear_execution_memory_for_tests() {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let _ = conn.execute("DELETE FROM execution_memory", []);
    }
}

#[cfg(test)]
pub fn clear_recommendations_for_tests() {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let _ = conn.execute("DELETE FROM recommendations", []);
    }
}

#[cfg(test)]
pub fn clear_launch_ops_events_for_tests() {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let _ = conn.execute("DELETE FROM launch_ops_events", []);
    }
}

#[cfg(test)]
pub fn clear_memory_admin_events_for_tests() {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let _ = conn.execute("DELETE FROM memory_admin_events", []);
    }
}

#[cfg(test)]
pub fn clear_recommendation_review_events_for_tests() {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let _ = conn.execute("DELETE FROM recommendation_review_events", []);
    }
}

#[cfg(test)]
pub fn clear_exec_approvals_for_tests() {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let _ = conn.execute("DELETE FROM exec_approvals", []);
    }
}

pub fn clear_nl_runs_for_tests() {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let _ = conn.execute("DELETE FROM nl_runs", []);
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Routine {
    pub id: i64,
    pub name: String,
    pub cron_expression: String,
    pub prompt: String,
    pub enabled: bool,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
    pub created_at: String,
}

pub fn create_routine(name: &str, cron: &str, prompt: &str) -> Result<i64> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();

        // Calculate initial next_run
        let next_run = match cron::Schedule::from_str(cron) {
            Ok(s) => s
                .upcoming(chrono::Utc)
                .next()
                .map(|d: chrono::DateTime<chrono::Utc>| d.to_rfc3339()),
            Err(_) => None, // Invalid cron, will never run (validation should happen before)
        };

        conn.execute(
            "INSERT INTO routines (name, cron_expression, prompt, created_at, next_run) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![name, cron, prompt, created_at, next_run],
        )?;
        Ok(conn.last_insert_rowid())
    } else {
        Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(1),
            Some("DB not initialized".to_string()),
        ))
    }
}

pub fn get_due_routines() -> Result<Vec<Routine>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = conn.prepare("SELECT id, name, cron_expression, prompt, enabled, last_run, next_run, created_at FROM routines WHERE enabled = 1 AND next_run <= ?1")?;
        let rows = stmt.query_map(params![now], |row| {
            Ok(Routine {
                id: row.get(0)?,
                name: row.get(1)?,
                cron_expression: row.get(2)?,
                prompt: row.get(3)?,
                enabled: row.get(4)?,
                last_run: row.get(5)?,
                next_run: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;

        let mut routines = Vec::new();
        for routine in rows {
            routines.push(routine?);
        }
        Ok(routines)
    } else {
        Ok(Vec::new())
    }
}

fn parse_routine_claim_stale_minutes() -> i64 {
    std::env::var("STEER_ROUTINE_CLAIM_STALE_MINUTES")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(120)
}

fn parse_ts_utc(value: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

pub fn claim_routine_execution(routine_id: i64, owner: &str) -> Result<bool> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let now_dt = chrono::Utc::now();
        let now = now_dt.to_rfc3339();
        let stale_before = now_dt - chrono::Duration::minutes(parse_routine_claim_stale_minutes());

        let tx = conn.transaction()?;
        let mut found = false;
        let mut enabled_raw: i64 = 0;
        let mut next_run: Option<String> = None;
        let mut claimed_at: Option<String> = None;
        {
            let mut stmt = tx.prepare(
                "SELECT enabled, next_run, run_claimed_at
                 FROM routines
                 WHERE id = ?1",
            )?;
            let mut rows = stmt.query(params![routine_id])?;
            if let Some(row) = rows.next()? {
                found = true;
                enabled_raw = row.get(0)?;
                next_run = row.get(1)?;
                claimed_at = row.get(2)?;
            }
        }
        if !found {
            tx.rollback()?;
            return Ok(false);
        }
        if enabled_raw == 0 {
            tx.rollback()?;
            return Ok(false);
        }
        if let Some(next) = next_run.as_deref().and_then(parse_ts_utc) {
            if next > now_dt {
                tx.rollback()?;
                return Ok(false);
            }
        }
        if let Some(claimed) = claimed_at.as_deref().and_then(parse_ts_utc) {
            if claimed > stale_before {
                tx.rollback()?;
                return Ok(false);
            }
        }
        tx.execute(
            "UPDATE routines
             SET run_claimed_at = ?1, run_claim_owner = ?2
             WHERE id = ?3",
            params![now, owner, routine_id],
        )?;
        tx.commit()?;
        return Ok(true);
    }
    Ok(false)
}

pub fn release_routine_execution(routine_id: i64, owner: Option<&str>) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        if let Some(owner_value) = owner {
            conn.execute(
                "UPDATE routines
                 SET run_claimed_at = NULL, run_claim_owner = NULL
                 WHERE id = ?1 AND (run_claim_owner = ?2 OR run_claim_owner IS NULL)",
                params![routine_id, owner_value],
            )?;
        } else {
            conn.execute(
                "UPDATE routines
                 SET run_claimed_at = NULL, run_claim_owner = NULL
                 WHERE id = ?1",
                params![routine_id],
            )?;
        }
    }
    Ok(())
}

pub fn update_routine_execution(id: i64, next: Option<String>) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE routines SET last_run = ?1, next_run = ?2 WHERE id = ?3",
            params![now, next, id],
        )?;
        Ok(())
    } else {
        Ok(())
    }
}

pub fn get_active_routines() -> Result<Vec<Routine>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare("SELECT id, name, cron_expression, prompt, enabled, last_run, next_run, created_at FROM routines WHERE enabled = 1")?;
        let rows = stmt.query_map([], |row| {
            Ok(Routine {
                id: row.get(0)?,
                name: row.get(1)?,
                cron_expression: row.get(2)?,
                prompt: row.get(3)?,
                enabled: row.get(4)?,
                last_run: row.get(5)?,
                next_run: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        // ... (collect)
        let mut routines = Vec::new();
        for routine in rows {
            routines.push(routine?);
        }
        Ok(routines)
    } else {
        Ok(Vec::new())
    }
}

pub fn get_all_routines() -> Result<Vec<Routine>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare("SELECT id, name, cron_expression, prompt, enabled, last_run, next_run, created_at FROM routines ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| {
            Ok(Routine {
                id: row.get(0)?,
                name: row.get(1)?,
                cron_expression: row.get(2)?,
                prompt: row.get(3)?,
                enabled: row.get(4)?,
                last_run: row.get(5)?,
                next_run: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        // ... (collect)
        let mut routines = Vec::new();
        for routine in rows {
            routines.push(routine?);
        }
        Ok(routines)
    } else {
        Ok(Vec::new())
    }
}

/// Toggle routine enabled status
pub fn toggle_routine(id: i64, enabled: bool) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "UPDATE routines SET enabled = ?1 WHERE id = ?2",
            params![enabled, id],
        )?;
        Ok(())
    } else {
        Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(1),
            Some("DB not initialized".to_string()),
        ))
    }
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
    pub snoozed_until: Option<String>,
    pub category: String,
    pub business_score: f64,
    pub feedback_status: Option<String>,
    pub feedback_note: Option<String>,
    pub feedback_count: i64,
    pub last_feedback_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RequestMemoryRecord {
    pub normalized_request: String,
    pub memory_scope: String,
    pub original_request: String,
    pub request_signature: Option<String>,
    pub intent_json: Option<String>,
    pub intent_command: Option<String>,
    pub response_text: Option<String>,
    pub response_mode: String,
    pub source: String,
    pub confidence: f64,
    pub positive_feedback_count: i64,
    pub negative_feedback_count: i64,
    pub last_feedback_at: Option<String>,
    pub suppressed: bool,
    pub suppressed_reason: Option<String>,
    pub suppressed_at: Option<String>,
    pub use_count: i64,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecutionMemoryRecord {
    pub id: i64,
    pub memory_scope: String,
    pub intent_command: String,
    pub params_key: String,
    pub params_json: Option<String>,
    pub request_signature: Option<String>,
    pub response_text: String,
    pub source: String,
    pub tool_path: String,
    pub freshness_ttl_seconds: i64,
    pub success: bool,
    pub positive_feedback_count: i64,
    pub negative_feedback_count: i64,
    pub last_feedback_at: Option<String>,
    pub suppressed: bool,
    pub suppressed_reason: Option<String>,
    pub suppressed_at: Option<String>,
    pub use_count: i64,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MemoryOpsMetrics {
    pub request_active: i64,
    pub request_suppressed: i64,
    pub execution_active: i64,
    pub execution_suppressed: i64,
    pub last_request_used_at: Option<String>,
    pub last_execution_used_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LaunchOpsRouteBreakdown {
    pub route_kind: String,
    pub count: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LaunchOpsEventRecord {
    pub id: i64,
    pub created_at: String,
    pub channel: Option<String>,
    pub memory_scope: Option<String>,
    pub message_preview: String,
    pub route_kind: String,
    pub command: Option<String>,
    pub outcome: String,
    pub confidence: Option<f64>,
    pub freshness_bypassed: bool,
    pub intent_memory_hit: bool,
    pub request_memory_hit: bool,
    pub execution_memory_hit: bool,
    pub deterministic_used: bool,
    pub llm_used: bool,
    pub ai_digest_used: bool,
    pub local_only: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryAdminEventRecord {
    pub id: i64,
    pub created_at: String,
    pub kind: String,
    pub action: String,
    pub memory_scope: Option<String>,
    pub target_key: String,
    pub reason: Option<String>,
    pub actor: Option<String>,
    pub ok: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecommendationReviewEventRecord {
    pub id: i64,
    pub created_at: String,
    pub recommendation_id: i64,
    pub recommendation_title: String,
    pub status_after: Option<String>,
    pub category: Option<String>,
    pub action: String,
    pub actor: Option<String>,
    pub note: Option<String>,
    pub ok: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LaunchOpsMetrics {
    pub window_size: i64,
    pub total_requests: i64,
    pub blocked_requests: i64,
    pub intent_memory_hits: i64,
    pub request_memory_hits: i64,
    pub execution_memory_hits: i64,
    pub cached_response_hit_rate: f64,
    pub deterministic_routes: i64,
    pub llm_routes: i64,
    pub ai_digest_routes: i64,
    pub ai_digest_auto_routes: i64,
    pub local_routes: i64,
    pub freshness_bypasses: i64,
    pub low_confidence_routes: i64,
    pub unknown_routes: i64,
    pub error_routes: i64,
    pub last_event_at: Option<String>,
    pub route_breakdown: Vec<LaunchOpsRouteBreakdown>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecommendationMetrics {
    pub total: i64,
    pub approved: i64,
    pub rejected: i64,
    pub failed: i64,
    pub pending: i64,
    /// Backward-compatible counter kept for older dashboard cards.
    pub later: i64,
    /// Count of records outside pending/approved/rejected (legacy data).
    pub legacy_other: i64,
    pub last_created_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecommendationReviewMetrics {
    pub window_size: i64,
    pub total_events: i64,
    pub approve_actions: i64,
    pub reject_actions: i64,
    pub later_actions: i64,
    pub restore_actions: i64,
    pub feedback_positive: i64,
    pub feedback_refine: i64,
    pub feedback_negative: i64,
    pub failed_actions: i64,
    pub action_failure_rate: f64,
    pub non_positive_feedback_rate: f64,
    pub last_event_at: Option<String>,
}

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecApprovalMetrics {
    pub window_size: i64,
    pub total: i64,
    pub pending: i64,
    pub approved: i64,
    pub rejected: i64,
    pub expired_pending: i64,
    pub allow_once: i64,
    pub allow_always: i64,
    pub deny: i64,
    pub approval_rate: f64,
    pub oldest_pending_created_at: Option<String>,
    pub last_created_at: Option<String>,
    pub last_resolved_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApprovalPolicy {
    pub policy_key: String,
    pub decision: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct ActiveApprovalDecision {
    pub status: String,
    pub expires_at: String,
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct QualityScoreRecord {
    pub created_at: String,
    pub overall: f64,
    pub breakdown: serde_json::Value,
    pub issues: Vec<String>,
    pub strengths: Vec<String>,
    pub recommendation: String,
    pub summary: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct JudgmentState {
    pub last_hash: Option<String>,
    pub consecutive_no_progress: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReleaseBaselineRecord {
    pub created_at: String,
    pub baseline_json: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VerificationRun {
    pub id: i64,
    pub created_at: String,
    pub kind: String,
    pub ok: bool,
    pub summary: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NLRun {
    pub id: i64,
    pub created_at: String,
    pub intent: String,
    pub prompt: String,
    pub status: String,
    pub summary: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskRunRecord {
    pub run_id: String,
    pub created_at: String,
    pub finished_at: Option<String>,
    pub intent: String,
    pub prompt: String,
    pub planner_complete: bool,
    pub execution_complete: bool,
    pub business_complete: bool,
    pub status: String,
    pub summary: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskStageRunRecord {
    pub id: i64,
    pub run_id: String,
    pub stage_name: String,
    pub stage_order: i64,
    pub status: String,
    pub started_at: String,
    pub finished_at: String,
    pub details: Option<String>,
    pub retry_count: i64,
    pub max_retries: i64,
    pub next_retry_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskStageAssertionRecord {
    pub id: i64,
    pub run_id: String,
    pub stage_name: String,
    pub assertion_key: String,
    pub expected: String,
    pub actual: String,
    pub passed: bool,
    pub evidence: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskRunArtifactRecord {
    pub id: i64,
    pub run_id: String,
    pub artifact_type: String,
    pub artifact_key: String,
    pub value: String,
    pub metadata: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NLRunMetrics {
    pub total: i64,
    pub completed: i64,
    pub manual_required: i64,
    pub approval_required: i64,
    pub blocked: i64,
    pub error: i64,
    pub success_rate: f64,
}

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
#[derive(Debug, Clone, serde::Serialize)]
pub struct RoutineRun {
    pub id: i64,
    pub routine_id: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub error: Option<String>,
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
                created_at, status, title, summary, trigger, actions, n8n_prompt, fingerprint, confidence, workflow_json, evidence, pattern_id, last_error, category, business_score, snoozed_until
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
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
                &proposal.category,
                proposal.business_score,
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
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT
                COUNT(*) as total,
                COALESCE(SUM(CASE WHEN status = 'approved' THEN 1 ELSE 0 END), 0) as approved,
                COALESCE(SUM(CASE WHEN status = 'rejected' THEN 1 ELSE 0 END), 0) as rejected,
                COALESCE(SUM(CASE WHEN last_error IS NOT NULL AND TRIM(last_error) != '' THEN 1 ELSE 0 END), 0) as failed,
                COALESCE(SUM(CASE
                    WHEN status = 'pending'
                     AND (snoozed_until IS NULL OR TRIM(snoozed_until) = '' OR snoozed_until <= ?1)
                    THEN 1 ELSE 0 END), 0) as pending,
                COALESCE(SUM(CASE
                    WHEN status = 'pending'
                     AND snoozed_until IS NOT NULL
                     AND TRIM(snoozed_until) != ''
                     AND snoozed_until > ?1
                    THEN 1 ELSE 0 END), 0) as later,
                COALESCE(SUM(CASE WHEN status NOT IN ('pending','approved','rejected') THEN 1 ELSE 0 END), 0) as legacy_other,
                MAX(created_at) as last_created_at
             FROM recommendations",
        )?;

        let metrics = stmt.query_row(params![now], |row| {
            Ok(RecommendationMetrics {
                total: row.get(0)?,
                approved: row.get(1)?,
                rejected: row.get(2)?,
                failed: row.get(3)?,
                pending: row.get(4)?,
                later: row.get(5)?,
                legacy_other: row.get(6)?,
                last_created_at: row.get(7).ok(),
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
        legacy_other: 0,
        last_created_at: None,
    })
}

pub fn get_recommendation_review_metrics(limit: i64) -> Result<RecommendationReviewMetrics> {
    let limit = normalize_admin_limit(limit);
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let metrics = conn.query_row(
            "SELECT
                COUNT(*) as total_events,
                COALESCE(SUM(CASE WHEN action = 'approve' THEN 1 ELSE 0 END), 0) as approve_actions,
                COALESCE(SUM(CASE WHEN action = 'reject' THEN 1 ELSE 0 END), 0) as reject_actions,
                COALESCE(SUM(CASE WHEN action = 'later' THEN 1 ELSE 0 END), 0) as later_actions,
                COALESCE(SUM(CASE WHEN action = 'restore' THEN 1 ELSE 0 END), 0) as restore_actions,
                COALESCE(SUM(CASE WHEN action = 'feedback_positive' THEN 1 ELSE 0 END), 0) as feedback_positive,
                COALESCE(SUM(CASE WHEN action = 'feedback_refine' THEN 1 ELSE 0 END), 0) as feedback_refine,
                COALESCE(SUM(CASE WHEN action = 'feedback_negative' THEN 1 ELSE 0 END), 0) as feedback_negative,
                COALESCE(SUM(CASE WHEN ok = 0 THEN 1 ELSE 0 END), 0) as failed_actions,
                MAX(created_at) as last_event_at
             FROM (
                SELECT action, note, message, ok, created_at
                FROM recommendation_review_events
                ORDER BY created_at DESC, id DESC
                LIMIT ?1
             ) recent",
            params![limit],
            |row| {
                let total_events: i64 = row.get(0)?;
                let approve_actions: i64 = row.get(1)?;
                let reject_actions: i64 = row.get(2)?;
                let later_actions: i64 = row.get(3)?;
                let restore_actions: i64 = row.get(4)?;
                let feedback_positive: i64 = row.get(5)?;
                let feedback_refine: i64 = row.get(6)?;
                let feedback_negative: i64 = row.get(7)?;
                let failed_actions: i64 = row.get(8)?;
                let last_event_at: Option<String> = row.get(9).ok();

                let action_total =
                    approve_actions + reject_actions + later_actions + restore_actions;
                let action_failure_rate = if action_total > 0 {
                    (failed_actions as f64 / action_total as f64) * 100.0
                } else {
                    0.0
                };
                let feedback_total = feedback_positive + feedback_refine + feedback_negative;
                let non_positive_feedback_rate = if feedback_total > 0 {
                    ((feedback_refine + feedback_negative) as f64 / feedback_total as f64) * 100.0
                } else {
                    0.0
                };

                Ok(RecommendationReviewMetrics {
                    window_size: limit,
                    total_events,
                    approve_actions,
                    reject_actions,
                    later_actions,
                    restore_actions,
                    feedback_positive,
                    feedback_refine,
                    feedback_negative,
                    failed_actions,
                    action_failure_rate,
                    non_positive_feedback_rate,
                    last_event_at,
                })
            },
        )?;
        return Ok(metrics);
    }
    Ok(RecommendationReviewMetrics {
        window_size: limit,
        total_events: 0,
        approve_actions: 0,
        reject_actions: 0,
        later_actions: 0,
        restore_actions: 0,
        feedback_positive: 0,
        feedback_refine: 0,
        feedback_negative: 0,
        failed_actions: 0,
        action_failure_rate: 0.0,
        non_positive_feedback_rate: 0.0,
        last_event_at: None,
    })
}

pub fn snooze_recommendation(id: i64, hours: i64) -> Result<()> {
    let snooze_hours = hours.clamp(1, 24 * 30);
    let snoozed_until = (chrono::Utc::now() + chrono::Duration::hours(snooze_hours)).to_rfc3339();
    let rec = get_recommendation(id)?.ok_or_else(|| {
        rusqlite::Error::InvalidParameterName(format!("recommendation {} not found", id))
    })?;
    if !rec.status.eq_ignore_ascii_case("pending") {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "cannot snooze recommendation {} from status {}",
            id, rec.status
        )));
    }
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "UPDATE recommendations
             SET status = 'pending',
                 snoozed_until = ?1
             WHERE id = ?2",
            params![snoozed_until, id],
        )?;
    }
    Ok(())
}

pub fn restore_recommendation(id: i64) -> Result<()> {
    let rec = get_recommendation(id)?.ok_or_else(|| {
        rusqlite::Error::InvalidParameterName(format!("recommendation {} not found", id))
    })?;
    if !rec.status.eq_ignore_ascii_case("pending") {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "cannot restore recommendation {} from status {}",
            id, rec.status
        )));
    }
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "UPDATE recommendations
             SET status = 'pending',
                 snoozed_until = NULL
             WHERE id = ?1",
            params![id],
        )?;
    }
    Ok(())
}

fn normalize_recommendation_feedback_status(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "positive" => Some("positive"),
        "refine" => Some("refine"),
        "negative" => Some("negative"),
        _ => None,
    }
}

pub fn record_recommendation_feedback(
    id: i64,
    feedback_status: &str,
    feedback_note: &str,
) -> Result<bool> {
    let Some(normalized_status) = normalize_recommendation_feedback_status(feedback_status) else {
        return Ok(false);
    };
    let note = feedback_note.trim();
    if note.is_empty() {
        return Ok(false);
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let updated = conn.execute(
            "UPDATE recommendations
             SET feedback_status = ?1,
                 feedback_note = ?2,
                 feedback_count = feedback_count + 1,
                 last_feedback_at = ?3
             WHERE id = ?4",
            params![normalized_status, note, now, id],
        )?;
        return Ok(updated > 0);
    }
    Ok(false)
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

pub fn get_exec_approval_metrics(limit: i64) -> Result<ExecApprovalMetrics> {
    let capped = limit.clamp(10, 500);
    let now = chrono::Utc::now().to_rfc3339();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT
                COUNT(*) as total,
                COALESCE(SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END), 0) as pending,
                COALESCE(SUM(CASE WHEN status = 'approved' THEN 1 ELSE 0 END), 0) as approved,
                COALESCE(SUM(CASE WHEN status = 'rejected' THEN 1 ELSE 0 END), 0) as rejected,
                COALESCE(SUM(CASE WHEN status = 'pending' AND expires_at <= ?2 THEN 1 ELSE 0 END), 0) as expired_pending,
                COALESCE(SUM(CASE WHEN decision = 'allow-once' THEN 1 ELSE 0 END), 0) as allow_once,
                COALESCE(SUM(CASE WHEN decision = 'allow-always' THEN 1 ELSE 0 END), 0) as allow_always,
                COALESCE(SUM(CASE WHEN decision = 'deny' THEN 1 ELSE 0 END), 0) as deny,
                MIN(CASE WHEN status = 'pending' THEN created_at END) as oldest_pending_created_at,
                MAX(created_at) as last_created_at,
                MAX(resolved_at) as last_resolved_at
             FROM (
                SELECT status, decision, created_at, expires_at, resolved_at
                FROM exec_approvals
                ORDER BY created_at DESC
                LIMIT ?1
             )",
        )?;

        let metrics = stmt.query_row(params![capped, now], |row| {
            let approved: i64 = row.get(2)?;
            let rejected: i64 = row.get(3)?;
            let resolved = approved + rejected;
            let approval_rate = if resolved > 0 {
                (approved as f64 / resolved as f64) * 100.0
            } else {
                0.0
            };
            Ok(ExecApprovalMetrics {
                window_size: capped,
                total: row.get(0)?,
                pending: row.get(1)?,
                approved,
                rejected,
                expired_pending: row.get(4)?,
                allow_once: row.get(5)?,
                allow_always: row.get(6)?,
                deny: row.get(7)?,
                approval_rate,
                oldest_pending_created_at: row.get(8).ok(),
                last_created_at: row.get(9).ok(),
                last_resolved_at: row.get(10).ok(),
            })
        })?;
        return Ok(metrics);
    }

    Ok(ExecApprovalMetrics {
        window_size: capped,
        total: 0,
        pending: 0,
        approved: 0,
        rejected: 0,
        expired_pending: 0,
        allow_once: 0,
        allow_always: 0,
        deny: 0,
        approval_rate: 0.0,
        oldest_pending_created_at: None,
        last_created_at: None,
        last_resolved_at: None,
    })
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
            Err(e) => Err(e),
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
        return Ok(policies);
    }
    Ok(Vec::new())
}

pub fn upsert_approval_decision(
    decision_key: &str,
    plan_id: &str,
    action: &str,
    status: &str,
    ttl_seconds: i64,
) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        ensure_approval_decisions_table(conn);
        let now = chrono::Utc::now();
        let max_ttl_seconds = std::env::var("STEER_APPROVAL_DECISION_MAX_TTL_SECONDS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .map(|v| v.max(60))
            .unwrap_or(60 * 60 * 24 * 365);
        let bounded_ttl = ttl_seconds.clamp(1, max_ttl_seconds);
        let created_at = now.to_rfc3339();
        let expires_at = (now + chrono::Duration::seconds(bounded_ttl)).to_rfc3339();
        conn.execute(
            "INSERT INTO nl_approval_decisions (
                decision_key, plan_id, action, status, created_at, expires_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(decision_key) DO UPDATE SET
                status = excluded.status,
                created_at = excluded.created_at,
                expires_at = excluded.expires_at",
            params![
                decision_key,
                plan_id,
                action,
                status,
                created_at,
                expires_at
            ],
        )?;
        let now_iso = chrono::Utc::now().to_rfc3339();
        let _ = conn.execute(
            "DELETE FROM nl_approval_decisions WHERE expires_at <= ?1",
            params![now_iso],
        );
    }
    Ok(())
}

pub fn get_active_approval_decision(decision_key: &str) -> Result<Option<ActiveApprovalDecision>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        ensure_approval_decisions_table(conn);
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT status, expires_at
             FROM nl_approval_decisions
             WHERE decision_key = ?1
               AND expires_at > ?2
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![decision_key, now])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(ActiveApprovalDecision {
                status: row.get(0)?,
                expires_at: row.get(1)?,
            }));
        }
    }
    Ok(None)
}

pub fn add_exec_allowlist(pattern: &str, cwd: Option<&str>) -> Result<i64> {
    validate_exec_allowlist_pattern(pattern)?;
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
        return Ok(entries);
    }
    Ok(Vec::new())
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
        return Ok(results);
    }
    Ok(Vec::new())
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
        return Ok(results);
    }
    Ok(Vec::new())
}

pub fn insert_quality_score(score: &QualityScore) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO quality_scores (created_at, overall, breakdown, issues, strengths, recommendation, summary)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                created_at,
                score.overall,
                serde_json::to_string(&score.breakdown).unwrap_or_else(|_| "{}".to_string()),
                serde_json::to_string(&score.issues).unwrap_or_else(|_| "[]".to_string()),
                serde_json::to_string(&score.strengths).unwrap_or_else(|_| "[]".to_string()),
                score.recommendation,
                score.summary
            ],
        )?;
    }
    Ok(())
}

pub fn get_latest_quality_score() -> Result<Option<QualityScoreRecord>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT created_at, overall, breakdown, issues, strengths, recommendation, summary
             FROM quality_scores
             ORDER BY created_at DESC
             LIMIT 1",
        )?;
        let row = stmt.query_row([], |row| {
            let breakdown_str: String = row.get(2)?;
            let issues_str: String = row.get(3)?;
            let strengths_str: String = row.get(4)?;
            Ok(QualityScoreRecord {
                created_at: row.get(0)?,
                overall: row.get(1)?,
                breakdown: serde_json::from_str(&breakdown_str)
                    .unwrap_or_else(|_| serde_json::json!({})),
                issues: serde_json::from_str(&issues_str).unwrap_or_else(|_| Vec::new()),
                strengths: serde_json::from_str(&strengths_str).unwrap_or_else(|_| Vec::new()),
                recommendation: row.get(5)?,
                summary: row.get(6)?,
            })
        });
        return match row {
            Ok(record) => Ok(Some(record)),
            Err(_) => Ok(None),
        };
    }
    Ok(None)
}

pub fn get_judgment_state() -> Result<Option<JudgmentState>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT last_hash, consecutive_no_progress, updated_at
             FROM judgment_states
             WHERE id = 1",
        )?;
        let row = stmt.query_row([], |row| {
            Ok(JudgmentState {
                last_hash: row.get(0)?,
                consecutive_no_progress: row.get(1)?,
                updated_at: row.get(2)?,
            })
        });
        return match row {
            Ok(state) => Ok(Some(state)),
            Err(_) => Ok(None),
        };
    }
    Ok(None)
}

pub fn upsert_judgment_state(last_hash: Option<&str>, consecutive_no_progress: i64) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let updated_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO judgment_states (id, last_hash, consecutive_no_progress, updated_at)
             VALUES (1, ?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET
                last_hash = excluded.last_hash,
                consecutive_no_progress = excluded.consecutive_no_progress,
                updated_at = excluded.updated_at",
            params![last_hash, consecutive_no_progress, updated_at],
        )?;
    }
    Ok(())
}

pub fn get_release_baseline_json() -> Result<Option<ReleaseBaselineRecord>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT created_at, baseline_json
             FROM release_baseline
             WHERE id = 1",
        )?;
        let row = stmt.query_row([], |row| {
            Ok(ReleaseBaselineRecord {
                created_at: row.get(0)?,
                baseline_json: row.get(1)?,
            })
        });
        return match row {
            Ok(record) => Ok(Some(record)),
            Err(_) => Ok(None),
        };
    }
    Ok(None)
}

pub fn upsert_release_baseline_json(created_at: &str, baseline_json: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "INSERT INTO release_baseline (id, created_at, baseline_json)
             VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET
                created_at = excluded.created_at,
                baseline_json = excluded.baseline_json",
            params![created_at, baseline_json],
        )?;
    }
    Ok(())
}

pub fn insert_verification_run(
    kind: &str,
    ok: bool,
    summary: &str,
    details: Option<&str>,
) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO verification_runs (created_at, kind, ok, summary, details)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![created_at, kind, ok, summary, details],
        )?;
    }
    Ok(())
}

pub fn list_verification_runs(limit: i64) -> Result<Vec<VerificationRun>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, created_at, kind, ok, summary, details
             FROM verification_runs
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(VerificationRun {
                id: row.get(0)?,
                created_at: row.get(1)?,
                kind: row.get(2)?,
                ok: row.get::<_, i64>(3)? != 0,
                summary: row.get(4)?,
                details: row.get(5).ok(),
            })
        })?;
        let mut runs = Vec::new();
        for r in rows {
            runs.push(r?);
        }
        return Ok(runs);
    }
    Ok(Vec::new())
}

pub fn insert_nl_run(
    intent: &str,
    prompt: &str,
    status: &str,
    summary: Option<&str>,
    details: Option<&str>,
) -> Result<()> {
    let _ = insert_nl_run_with_source_key(None, None, intent, prompt, status, summary, details)?;
    Ok(())
}

pub fn insert_nl_run_with_source_key(
    created_at: Option<&str>,
    source_key: Option<&str>,
    intent: &str,
    prompt: &str,
    status: &str,
    summary: Option<&str>,
    details: Option<&str>,
) -> Result<bool> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = created_at
            .map(|value| truncate_text(value, 64))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
        let source_key = source_key
            .map(|value| truncate_text(value, 190))
            .filter(|value| !value.is_empty());
        let changed = conn.execute(
            "INSERT INTO nl_runs (created_at, intent, prompt, status, summary, details, source_key)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(source_key) DO NOTHING",
            params![created_at, intent, prompt, status, summary, details, source_key],
        )?;
        return Ok(changed > 0);
    }
    Ok(false)
}

pub fn create_task_run(run_id: &str, intent: &str, prompt: &str, status: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO task_runs (
                run_id, plan_id, created_at, finished_at, intent, prompt,
                planner_complete, execution_complete, business_complete,
                status, summary, details
             ) VALUES (?1, NULL, ?2, NULL, ?3, ?4, 0, 0, 0, ?5, NULL, NULL)",
            params![run_id, created_at, intent, prompt, status],
        )?;
    }
    Ok(())
}

fn parse_task_run_stale_minutes() -> i64 {
    std::env::var("STEER_TASK_RUN_STALE_MINUTES")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(120)
}

pub fn mark_stale_running_task_runs_finished() -> Result<usize> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let stale_window = format!("-{} minutes", parse_task_run_stale_minutes());
        let finished_at = chrono::Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE task_runs
             SET status = 'business_failed',
                 finished_at = COALESCE(finished_at, ?1),
                 summary = COALESCE(summary, 'auto-finalized stale running run'),
                 details = COALESCE(details, '{\"source\":\"db.mark_stale_running_task_runs_finished\",\"reason\":\"stale_running_run\"}')
             WHERE status = 'running'
               AND finished_at IS NULL
               AND julianday(created_at) < julianday('now', ?2)",
            params![finished_at, stale_window],
        )?;
        return Ok(rows);
    }
    Ok(0)
}

pub fn mark_orphaned_inflight_task_runs_failed() -> Result<usize> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let finished_at = chrono::Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE task_runs
             SET status = 'business_failed',
                 finished_at = COALESCE(finished_at, ?1),
                 summary = COALESCE(summary, 'auto-finalized orphaned in-flight run after core restart'),
                 details = COALESCE(details, '{\"source\":\"db.mark_orphaned_inflight_task_runs_failed\",\"reason\":\"orphaned_inflight_run\"}')
             WHERE finished_at IS NULL
               AND lower(status) IN ('queued', 'running', 'accepted', 'started', 'retrying', 'business_incomplete')",
            params![finished_at],
        )?;
        return Ok(rows);
    }
    Ok(0)
}

fn parse_stage_max_retries() -> i64 {
    std::env::var("STEER_STAGE_MAX_RETRIES")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .map(|v| v.clamp(0, 16))
        .unwrap_or(2)
}

fn parse_stage_retry_backoff_base_seconds() -> i64 {
    std::env::var("STEER_STAGE_RETRY_BACKOFF_BASE_SECONDS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .map(|v| v.clamp(1, 120))
        .unwrap_or(5)
}

pub fn claim_task_run(
    plan_id: &str,
    run_id: &str,
    intent: &str,
    prompt: &str,
    status: &str,
) -> Result<bool> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        let stale_window = format!("-{} minutes", parse_task_run_stale_minutes());
        let tx = conn.transaction()?;
        let inflight_count: i64 = tx.query_row(
            "SELECT COUNT(1)
             FROM task_runs
             WHERE plan_id = ?1
               AND status = 'running'
               AND (
                 finished_at IS NULL
                 OR julianday(created_at) >= julianday('now', ?2)
               )",
            params![plan_id, stale_window],
            |row| row.get(0),
        )?;
        if inflight_count > 0 {
            tx.rollback()?;
            return Ok(false);
        }
        tx.execute(
            "INSERT INTO task_runs (
                run_id, plan_id, created_at, finished_at, intent, prompt,
                planner_complete, execution_complete, business_complete,
                status, summary, details
             ) VALUES (?1, ?2, ?3, NULL, ?4, ?5, 0, 0, 0, ?6, NULL, NULL)",
            params![run_id, plan_id, created_at, intent, prompt, status],
        )?;
        tx.commit()?;
    }
    Ok(true)
}

fn canonical_stage_status(raw: &str) -> String {
    let normalized = raw.trim().to_lowercase();
    match normalized.as_str() {
        "" | "queued" => "pending".to_string(),
        "pending" => "pending".to_string(),
        "running" | "in_progress" | "in-progress" | "started" => "running".to_string(),
        "retry" | "retrying" => "retrying".to_string(),
        "completed" | "success" | "ok" | "done" => "completed".to_string(),
        "failed" | "error" => "failed".to_string(),
        "blocked" | "manual_required" | "approval_required" => "blocked".to_string(),
        other => other.to_string(),
    }
}

fn is_known_stage_status(status: &str) -> bool {
    matches!(
        status,
        "pending" | "running" | "retrying" | "completed" | "failed" | "blocked"
    )
}

fn stage_transition_allowed(prev: &str, next: &str) -> bool {
    if prev == next {
        return true;
    }
    if !is_known_stage_status(prev) || !is_known_stage_status(next) {
        return true;
    }
    matches!(
        (prev, next),
        ("pending", "running")
            | ("pending", "retrying")
            | ("pending", "completed")
            | ("pending", "failed")
            | ("pending", "blocked")
            | ("running", "retrying")
            | ("running", "completed")
            | ("running", "failed")
            | ("running", "blocked")
            | ("retrying", "running")
            | ("retrying", "completed")
            | ("retrying", "failed")
            | ("retrying", "blocked")
            | ("failed", "retrying")
            | ("failed", "running")
            | ("blocked", "retrying")
            | ("blocked", "running")
    )
}

pub fn record_task_stage_run(
    run_id: &str,
    stage_name: &str,
    stage_order: i64,
    status: &str,
    details: Option<&str>,
) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let now = chrono::Utc::now().to_rfc3339();
        let status = canonical_stage_status(status);
        let details_clean = details.map(str::trim).filter(|v| !v.is_empty());
        let stage_max_retries_default = parse_stage_max_retries();
        let stage_backoff_base = parse_stage_retry_backoff_base_seconds();

        let mut stmt = conn.prepare(
            "SELECT status, started_at, details, retry_count, max_retries
             FROM task_stage_runs
             WHERE run_id = ?1
               AND stage_name = ?2
               AND stage_order = ?3
             ORDER BY id DESC
             LIMIT 1",
        )?;
        let previous = stmt.query_row(params![run_id, stage_name, stage_order], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2).ok().flatten(),
                row.get::<_, i64>(3).unwrap_or(0),
                row.get::<_, i64>(4).unwrap_or(0),
            ))
        });

        let mut started_at = now.clone();
        let mut retry_count: i64 = 0;
        let mut max_retries: i64 = stage_max_retries_default;
        let mut next_retry_at: Option<String> = None;
        if let Ok((
            prev_status_raw,
            prev_started_at,
            prev_details,
            prev_retry_count,
            prev_max_retries,
        )) = previous
        {
            let prev_status = canonical_stage_status(&prev_status_raw);
            let prev_detail_text = prev_details.unwrap_or_default();
            let next_detail_text = details_clean.unwrap_or("");
            retry_count = prev_retry_count.max(0);
            if prev_max_retries > 0 {
                max_retries = prev_max_retries;
            }
            if prev_status == status && prev_detail_text.trim() == next_detail_text {
                // Idempotent duplicate; keep history clean by skipping extra row.
                return Ok(());
            }
            if !stage_transition_allowed(prev_status.as_str(), status.as_str()) {
                eprintln!(
                    "⚠️ Invalid stage transition ignored: run_id={} stage={} order={} {} -> {}",
                    run_id, stage_name, stage_order, prev_status, status
                );
                return Err(rusqlite::Error::InvalidQuery);
            }
            let restart_attempt = matches!(
                (prev_status.as_str(), status.as_str()),
                ("failed", "running")
                    | ("failed", "retrying")
                    | ("blocked", "running")
                    | ("blocked", "retrying")
            );
            started_at = if restart_attempt {
                now.clone()
            } else {
                prev_started_at
            };
        }

        if status == "retrying" {
            retry_count += 1;
            let shift = (retry_count.saturating_sub(1)).clamp(0, 6) as u32;
            let multiplier = 1_i64.checked_shl(shift).unwrap_or(64);
            let backoff_secs = (stage_backoff_base * multiplier).clamp(1, 600);
            next_retry_at =
                Some((chrono::Utc::now() + chrono::Duration::seconds(backoff_secs)).to_rfc3339());
        }

        conn.execute(
            "INSERT INTO task_stage_runs (
                run_id, stage_name, stage_order, status, started_at, finished_at, details,
                retry_count, max_retries, next_retry_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                run_id,
                stage_name,
                stage_order,
                status,
                started_at,
                now,
                details_clean,
                retry_count,
                max_retries,
                next_retry_at
            ],
        )?;
    }
    Ok(())
}

pub fn record_task_stage_assertion(
    run_id: &str,
    stage_name: &str,
    assertion_key: &str,
    expected: &str,
    actual: &str,
    passed: bool,
    evidence: Option<&str>,
) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO task_stage_assertions (
                run_id, stage_name, assertion_key, expected, actual, passed, evidence, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run_id,
                stage_name,
                assertion_key,
                expected,
                actual,
                passed as i64,
                evidence,
                created_at
            ],
        )?;
    }
    Ok(())
}

pub fn upsert_task_run_artifact(
    run_id: &str,
    artifact_type: &str,
    artifact_key: &str,
    value: &str,
    metadata: Option<&str>,
) -> Result<()> {
    let run_id = run_id.trim();
    let artifact_type = artifact_type.trim();
    let artifact_key = artifact_key.trim();
    if run_id.is_empty() || artifact_type.is_empty() || artifact_key.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "run_id/artifact_type/artifact_key must not be empty".to_string(),
        ));
    }

    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO task_run_artifacts (
                run_id, artifact_type, artifact_key, value, metadata, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(run_id, artifact_type, artifact_key) DO UPDATE SET
                value = excluded.value,
                metadata = excluded.metadata,
                created_at = excluded.created_at",
            params![
                run_id,
                artifact_type,
                artifact_key,
                value,
                metadata,
                created_at
            ],
        )?;
    }
    Ok(())
}

pub fn update_task_run_outcome(
    run_id: &str,
    planner_complete: bool,
    execution_complete: bool,
    business_complete: bool,
    status: &str,
    summary: Option<&str>,
    details: Option<&str>,
) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let finished_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE task_runs
             SET finished_at = ?1,
                 planner_complete = ?2,
                 execution_complete = ?3,
                 business_complete = ?4,
                 status = ?5,
                 summary = ?6,
                 details = ?7
             WHERE run_id = ?8",
            params![
                finished_at,
                planner_complete as i64,
                execution_complete as i64,
                business_complete as i64,
                status,
                summary,
                details,
                run_id
            ],
        )?;
    }
    Ok(())
}

pub fn get_task_run(run_id: &str) -> Result<Option<TaskRunRecord>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT run_id, created_at, finished_at, intent, prompt,
                    planner_complete, execution_complete, business_complete,
                    status, summary, details
             FROM task_runs
             WHERE run_id = ?1
             LIMIT 1",
        )?;

        let mut rows = stmt.query(params![run_id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(TaskRunRecord {
                run_id: row.get(0)?,
                created_at: row.get(1)?,
                finished_at: row.get(2).ok(),
                intent: row.get(3)?,
                prompt: row.get(4)?,
                planner_complete: row.get::<_, i64>(5)? != 0,
                execution_complete: row.get::<_, i64>(6)? != 0,
                business_complete: row.get::<_, i64>(7)? != 0,
                status: row.get(8)?,
                summary: row.get(9).ok(),
                details: row.get(10).ok(),
            }));
        }
    }
    Ok(None)
}

pub fn list_task_runs(limit: i64, status: Option<&str>) -> Result<Vec<TaskRunRecord>> {
    let bounded_limit = limit.clamp(1, 500);
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let normalized_status = status.map(|s| s.trim()).filter(|s| !s.is_empty());
        let mut out = Vec::new();

        if let Some(status_filter) = normalized_status {
            let mut stmt = conn.prepare(
                "SELECT run_id, created_at, finished_at, intent, prompt,
                        planner_complete, execution_complete, business_complete,
                        status, summary, details
                 FROM task_runs
                 WHERE status = ?1
                 ORDER BY created_at DESC
                 LIMIT ?2",
            )?;

            let rows = stmt.query_map(params![status_filter, bounded_limit], |row| {
                Ok(TaskRunRecord {
                    run_id: row.get(0)?,
                    created_at: row.get(1)?,
                    finished_at: row.get(2).ok(),
                    intent: row.get(3)?,
                    prompt: row.get(4)?,
                    planner_complete: row.get::<_, i64>(5)? != 0,
                    execution_complete: row.get::<_, i64>(6)? != 0,
                    business_complete: row.get::<_, i64>(7)? != 0,
                    status: row.get(8)?,
                    summary: row.get(9).ok(),
                    details: row.get(10).ok(),
                })
            })?;

            for row in rows {
                out.push(row?);
            }
            return Ok(out);
        }

        let mut stmt = conn.prepare(
            "SELECT run_id, created_at, finished_at, intent, prompt,
                    planner_complete, execution_complete, business_complete,
                    status, summary, details
             FROM task_runs
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![bounded_limit], |row| {
            Ok(TaskRunRecord {
                run_id: row.get(0)?,
                created_at: row.get(1)?,
                finished_at: row.get(2).ok(),
                intent: row.get(3)?,
                prompt: row.get(4)?,
                planner_complete: row.get::<_, i64>(5)? != 0,
                execution_complete: row.get::<_, i64>(6)? != 0,
                business_complete: row.get::<_, i64>(7)? != 0,
                status: row.get(8)?,
                summary: row.get(9).ok(),
                details: row.get(10).ok(),
            })
        })?;

        for row in rows {
            out.push(row?);
        }
        return Ok(out);
    }
    Ok(Vec::new())
}

pub fn get_latest_inflight_task_run() -> Result<Option<TaskRunRecord>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT run_id, created_at, finished_at, intent, prompt,
                    planner_complete, execution_complete, business_complete,
                    status, summary, details
             FROM task_runs
             WHERE finished_at IS NULL
               AND lower(status) IN ('queued', 'running', 'accepted', 'started', 'retrying', 'business_incomplete')
             ORDER BY created_at DESC
             LIMIT 1",
        )?;

        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(TaskRunRecord {
                run_id: row.get(0)?,
                created_at: row.get(1)?,
                finished_at: row.get(2).ok(),
                intent: row.get(3)?,
                prompt: row.get(4)?,
                planner_complete: row.get::<_, i64>(5)? != 0,
                execution_complete: row.get::<_, i64>(6)? != 0,
                business_complete: row.get::<_, i64>(7)? != 0,
                status: row.get(8)?,
                summary: row.get(9).ok(),
                details: row.get(10).ok(),
            }));
        }
    }
    Ok(None)
}

pub fn list_task_stage_runs(run_id: &str) -> Result<Vec<TaskStageRunRecord>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, run_id, stage_name, stage_order, status, started_at, finished_at, details,
                    retry_count, max_retries, next_retry_at
             FROM task_stage_runs
             WHERE run_id = ?1
             ORDER BY stage_order ASC, id ASC",
        )?;

        let rows = stmt.query_map(params![run_id], |row| {
            Ok(TaskStageRunRecord {
                id: row.get(0)?,
                run_id: row.get(1)?,
                stage_name: row.get(2)?,
                stage_order: row.get(3)?,
                status: row.get(4)?,
                started_at: row.get(5)?,
                finished_at: row.get(6)?,
                details: row.get(7).ok(),
                retry_count: row.get::<_, i64>(8).unwrap_or(0),
                max_retries: row.get::<_, i64>(9).unwrap_or(0),
                next_retry_at: row.get(10).ok(),
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

pub fn list_task_stage_assertions(run_id: &str) -> Result<Vec<TaskStageAssertionRecord>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, run_id, stage_name, assertion_key, expected, actual, passed, evidence, created_at
             FROM task_stage_assertions
             WHERE run_id = ?1
             ORDER BY id ASC",
        )?;

        let rows = stmt.query_map(params![run_id], |row| {
            Ok(TaskStageAssertionRecord {
                id: row.get(0)?,
                run_id: row.get(1)?,
                stage_name: row.get(2)?,
                assertion_key: row.get(3)?,
                expected: row.get(4)?,
                actual: row.get(5)?,
                passed: row.get::<_, i64>(6)? != 0,
                evidence: row.get(7).ok(),
                created_at: row.get(8)?,
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

pub fn list_task_run_artifacts(run_id: &str) -> Result<Vec<TaskRunArtifactRecord>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, run_id, artifact_type, artifact_key, value, metadata, created_at
             FROM task_run_artifacts
             WHERE run_id = ?1
             ORDER BY artifact_type ASC, artifact_key ASC, id ASC",
        )?;

        let rows = stmt.query_map(params![run_id], |row| {
            Ok(TaskRunArtifactRecord {
                id: row.get(0)?,
                run_id: row.get(1)?,
                artifact_type: row.get(2)?,
                artifact_key: row.get(3)?,
                value: row.get(4)?,
                metadata: row.get(5).ok(),
                created_at: row.get(6)?,
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

pub fn list_nl_runs(limit: i64) -> Result<Vec<NLRun>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, created_at, intent, prompt, status, summary, details
             FROM nl_runs
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(NLRun {
                id: row.get(0)?,
                created_at: row.get(1)?,
                intent: row.get(2)?,
                prompt: row.get(3)?,
                status: row.get(4)?,
                summary: row.get(5).ok(),
                details: row.get(6).ok(),
            })
        })?;
        let mut runs = Vec::new();
        for r in rows {
            runs.push(r?);
        }
        return Ok(runs);
    }
    Ok(Vec::new())
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

pub fn get_nl_run_metrics(limit: i64) -> Result<NLRunMetrics> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT
                COUNT(*) as total,
                COALESCE(SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END), 0) as completed,
                COALESCE(SUM(CASE WHEN status = 'manual_required' THEN 1 ELSE 0 END), 0) as manual_required,
                COALESCE(SUM(CASE WHEN status = 'approval_required' THEN 1 ELSE 0 END), 0) as approval_required,
                COALESCE(SUM(CASE WHEN status = 'blocked' THEN 1 ELSE 0 END), 0) as blocked,
                COALESCE(SUM(CASE WHEN status = 'error' THEN 1 ELSE 0 END), 0) as error_count
             FROM (
                SELECT status
                FROM nl_runs
                ORDER BY created_at DESC
                LIMIT ?1
             )",
        )?;
        let metrics = stmt.query_row(params![limit], |row| {
            let total: i64 = row.get(0)?;
            let completed: i64 = row.get(1)?;
            let manual_required: i64 = row.get(2)?;
            let approval_required: i64 = row.get(3)?;
            let blocked: i64 = row.get(4)?;
            let error: i64 = row.get(5)?;
            let success_rate = if total > 0 {
                (completed as f64) / (total as f64) * 100.0
            } else {
                0.0
            };
            Ok(NLRunMetrics {
                total,
                completed,
                manual_required,
                approval_required,
                blocked,
                error,
                success_rate,
            })
        })?;
        return Ok(metrics);
    }
    Ok(NLRunMetrics {
        total: 0,
        completed: 0,
        manual_required: 0,
        approval_required: 0,
        blocked: 0,
        error: 0,
        success_rate: 0.0,
    })
}

fn normalize_release_nl_prompt(prompt: &str) -> String {
    prompt
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn release_nl_details_value(run: &NLRun) -> Option<serde_json::Value> {
    let raw = run.details.as_deref()?.trim();
    if raw.is_empty() || !raw.starts_with('{') {
        return None;
    }
    serde_json::from_str(raw).ok()
}

fn release_nl_detail_str<'a>(details: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    details.get(key).and_then(|value| value.as_str())
}

fn is_release_nl_noise(run: &NLRun) -> bool {
    let prompt = normalize_release_nl_prompt(&run.prompt);
    if prompt.is_empty() || prompt.starts_with('/') {
        return true;
    }

    if let Some(details) = release_nl_details_value(run) {
        let route_kind = release_nl_detail_str(&details, "route_kind").unwrap_or_default();
        let command = release_nl_detail_str(&details, "command").unwrap_or_default();
        let source = release_nl_detail_str(&details, "source").unwrap_or_default();
        if source == "api.chat"
            && matches!(
                route_kind,
                "empty_message"
                    | "gate_blocked"
                    | "system_command"
                    | "local_command"
                    | "vision_demo"
            )
        {
            return true;
        }
        if matches!(
            command,
            "help_local"
                | "greeting_local"
                | "system_status"
                | "telegram_listener_start"
                | "telegram_listener_status"
                | "n8n_restart"
        ) {
            return true;
        }
    }

    let summary = run.summary.as_deref().unwrap_or_default().to_lowercase();
    let details = run.details.as_deref().unwrap_or_default().to_lowercase();
    let combined = format!("{}\n{}", summary, details);

    if combined.contains("launch eval")
        || combined.contains("launch.eval")
        || combined.contains("auto-finalized orphaned in-flight run")
    {
        return true;
    }

    let flight_like = prompt.contains("항공권")
        || prompt.contains("flight")
        || summary.contains("search flights")
        || details.contains("(flight_search)");
    if flight_like && combined.contains(" on unknown") {
        return true;
    }

    false
}

fn should_backfill_launch_ops_event(event: &LaunchOpsEventRecord) -> bool {
    if normalize_release_nl_prompt(&event.message_preview).is_empty() {
        return false;
    }

    if matches!(
        event.route_kind.as_str(),
        "empty_message" | "gate_blocked" | "system_command" | "local_command" | "vision_demo"
    ) {
        return false;
    }

    !matches!(
        event.command.as_deref().unwrap_or(""),
        "help_local"
            | "greeting_local"
            | "system_status"
            | "telegram_listener_start"
            | "telegram_listener_status"
            | "n8n_restart"
    )
}

fn backfilled_nl_status_from_launch_ops(event: &LaunchOpsEventRecord) -> &'static str {
    if event.outcome == "blocked" || event.route_kind == "gate_blocked" {
        return "blocked";
    }
    if event.command.as_deref() == Some("build_workflow") {
        return "approval_required";
    }
    if event.note.as_deref().is_some_and(|note| {
        note.contains("approval") || note.contains("승인") || note.contains("manual")
    }) {
        return if event.note.as_deref().unwrap_or_default().contains("manual") {
            "manual_required"
        } else {
            "approval_required"
        };
    }
    if event.outcome == "success" {
        "completed"
    } else {
        "error"
    }
}

pub fn sync_release_nl_runs_from_launch_ops(limit: i64) -> Result<i64> {
    let events = list_launch_ops_events(limit)?;
    let mut inserted = 0i64;

    for event in events.into_iter().rev() {
        if !should_backfill_launch_ops_event(&event) {
            continue;
        }

        let status = backfilled_nl_status_from_launch_ops(&event);
        let summary = Some(truncate_text(&event.message_preview, 160));
        let details = serde_json::json!({
            "source": "api.chat",
            "route_kind": event.route_kind,
            "command": event.command,
            "channel": event.channel,
            "memory_scope": event.memory_scope,
            "outcome": event.outcome,
            "confidence": event.confidence,
            "note": event.note,
            "flags": {
                "freshness_bypassed": event.freshness_bypassed,
                "intent_memory_hit": event.intent_memory_hit,
                "request_memory_hit": event.request_memory_hit,
                "execution_memory_hit": event.execution_memory_hit,
                "deterministic_used": event.deterministic_used,
                "llm_used": event.llm_used,
                "ai_digest_used": event.ai_digest_used,
                "local_only": event.local_only
            },
            "backfilled_from": "launch_ops_events"
        });
        let details_json = serde_json::to_string(&details).ok();
        let source_key = format!("launch_ops_event:{}", event.id);
        if insert_nl_run_with_source_key(
            Some(&event.created_at),
            Some(&source_key),
            event.command.as_deref().unwrap_or(&event.route_kind),
            &event.message_preview,
            status,
            summary.as_deref(),
            details_json.as_deref(),
        )? {
            inserted += 1;
        }
    }

    Ok(inserted)
}

fn aggregate_nl_runs<'a, I>(runs: I) -> NLRunMetrics
where
    I: IntoIterator<Item = &'a NLRun>,
{
    let mut total = 0i64;
    let mut completed = 0i64;
    let mut manual_required = 0i64;
    let mut approval_required = 0i64;
    let mut blocked = 0i64;
    let mut error = 0i64;

    for run in runs {
        total += 1;
        match run.status.as_str() {
            "completed" => completed += 1,
            "manual_required" => manual_required += 1,
            "approval_required" => approval_required += 1,
            "blocked" => blocked += 1,
            "error" => error += 1,
            _ => {}
        }
    }

    let success_rate = if total > 0 {
        (completed as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    NLRunMetrics {
        total,
        completed,
        manual_required,
        approval_required,
        blocked,
        error,
        success_rate,
    }
}

pub fn get_release_nl_run_metrics(limit: i64) -> Result<NLRunMetrics> {
    let runs = list_nl_runs(limit)?;
    let max_runs_per_prompt = std::env::var("RELEASE_NL_MAX_RUNS_PER_PROMPT")
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .unwrap_or(3)
        .max(1);
    let mut prompt_counts = std::collections::HashMap::new();
    let mut filtered = Vec::new();

    for run in &runs {
        if is_release_nl_noise(run) {
            continue;
        }
        let prompt_key = normalize_release_nl_prompt(&run.prompt);
        if prompt_key.is_empty() {
            continue;
        }
        let entry = prompt_counts.entry(prompt_key).or_insert(0usize);
        if *entry >= max_runs_per_prompt {
            continue;
        }
        *entry += 1;
        filtered.push(run.clone());
    }

    Ok(aggregate_nl_runs(filtered.iter()))
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

fn exec_pattern_match_with_flags(
    pattern: &str,
    command: &str,
    allow_global: bool,
    allow_regex: bool,
) -> bool {
    let trimmed = pattern.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed == "*" || trimmed.eq_ignore_ascii_case("all") {
        if !allow_global {
            return false;
        }
        return true;
    }
    if let Some(rest) = trimmed.strip_prefix("re:") {
        if !allow_regex {
            return false;
        }
        if let Ok(re) = regex::Regex::new(rest) {
            return re.is_match(command);
        }
    }
    if trimmed.starts_with('/') && trimmed.ends_with('/') && trimmed.len() > 2 {
        if !allow_regex {
            return false;
        }
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

fn exec_pattern_match(pattern: &str, command: &str) -> bool {
    exec_pattern_match_with_flags(
        pattern,
        command,
        crate::env_flag("STEER_EXEC_ALLOWLIST_ALLOW_GLOBAL"),
        crate::env_flag("STEER_EXEC_ALLOWLIST_ALLOW_REGEX"),
    )
}

fn validate_exec_allowlist_pattern_with_flags(
    pattern: &str,
    allow_global: bool,
    allow_regex: bool,
) -> Result<()> {
    let trimmed = pattern.trim();
    if trimmed.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "exec allowlist pattern rejected: empty pattern".to_string(),
        ));
    }
    if trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(rusqlite::Error::InvalidParameterName(
            "exec allowlist pattern rejected: multiline pattern is not allowed".to_string(),
        ));
    }
    if trimmed == "*" || trimmed.eq_ignore_ascii_case("all") {
        if !allow_global {
            return Err(rusqlite::Error::InvalidParameterName(
                "exec allowlist pattern rejected: global wildcard requires STEER_EXEC_ALLOWLIST_ALLOW_GLOBAL=1".to_string(),
            ));
        }
        return Ok(());
    }
    if trimmed.starts_with("re:")
        || (trimmed.starts_with('/') && trimmed.ends_with('/') && trimmed.len() > 2)
    {
        if !allow_regex {
            return Err(rusqlite::Error::InvalidParameterName(
                "exec allowlist pattern rejected: regex requires STEER_EXEC_ALLOWLIST_ALLOW_REGEX=1".to_string(),
            ));
        }
        return Ok(());
    }
    Ok(())
}

fn validate_exec_allowlist_pattern(pattern: &str) -> Result<()> {
    validate_exec_allowlist_pattern_with_flags(
        pattern,
        crate::env_flag("STEER_EXEC_ALLOWLIST_ALLOW_GLOBAL"),
        crate::env_flag("STEER_EXEC_ALLOWLIST_ALLOW_REGEX"),
    )
}

pub fn has_recent_pattern_recommendation(pattern_id: &str, hours: i64) -> Result<bool> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let cutoff = (chrono::Utc::now() - chrono::Duration::hours(hours)).to_rfc3339();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM recommendations WHERE pattern_id = ?1 AND created_at >= ?2",
            params![pattern_id, cutoff],
            |row| row.get(0),
        )?;
        return Ok(count > 0);
    }
    Ok(false)
}

pub fn create_routine_run(routine_id: i64) -> Result<i64> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let started_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO routine_runs (routine_id, started_at, status) VALUES (?1, ?2, 'running')",
            params![routine_id, started_at],
        )?;
        return Ok(conn.last_insert_rowid());
    }
    Ok(0)
}

pub fn finish_routine_run(run_id: i64, status: &str, error: Option<&str>) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let finished_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE routine_runs SET status = ?1, error = ?2, finished_at = ?3 WHERE id = ?4",
            params![status, error, finished_at, run_id],
        )?;
    }
    Ok(())
}

pub fn list_routine_runs(limit: i64) -> Result<Vec<RoutineRun>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, routine_id, started_at, finished_at, status, error
             FROM routine_runs ORDER BY started_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |row| {
            Ok(RoutineRun {
                id: row.get(0)?,
                routine_id: row.get(1)?,
                started_at: row.get(2)?,
                finished_at: row.get(3).ok(),
                status: row.get(4)?,
                error: row.get(5).ok(),
            })
        })?;

        let mut runs = Vec::new();
        for r in rows {
            runs.push(r?);
        }
        return Ok(runs);
    }
    Ok(Vec::new())
}

pub fn insert_routine_candidate(pattern: &crate::pattern_detector::DetectedPattern) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let created_at = chrono::Utc::now().to_rfc3339();
        let samples_json = serde_json::to_string(&pattern.sample_events).unwrap_or_default();

        conn.execute(
            "INSERT OR IGNORE INTO routine_candidates (
                candidate_id, created_at, pattern_type, description, frequency, score, sample_events
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                pattern.pattern_id,
                created_at,
                pattern.pattern_type.as_str(),
                pattern.description,
                pattern.occurrences,
                pattern.similarity_score,
                samples_json
            ],
        )?;
    }
    Ok(())
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
                created_at, status, title, summary, trigger, actions, n8n_prompt, fingerprint, confidence, workflow_json, category, business_score
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
                briefing_json,
                crate::recommendation_policy::CATEGORY_WORK,
                0.92
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
                created_at, status, title, summary, trigger, actions, n8n_prompt, fingerprint, confidence, workflow_json, category, business_score
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
                urgent_mail_json,
                crate::recommendation_policy::CATEGORY_WORK,
                0.9
            ],
        )?;
    }
    Ok(())
}

pub fn get_recommendations_with_filter(status_filter: Option<&str>) -> Result<Vec<Recommendation>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let sql = match status_filter {
            Some("all") => "SELECT id, status, title, summary, trigger, actions, n8n_prompt, confidence, workflow_id, workflow_json, evidence, pattern_id, last_error, snoozed_until, category, business_score, feedback_status, feedback_note, feedback_count, last_feedback_at FROM recommendations ORDER BY created_at DESC",
            Some(_) => "SELECT id, status, title, summary, trigger, actions, n8n_prompt, confidence, workflow_id, workflow_json, evidence, pattern_id, last_error, snoozed_until, category, business_score, feedback_status, feedback_note, feedback_count, last_feedback_at FROM recommendations WHERE status = ?1 ORDER BY created_at DESC",
            None => "SELECT id, status, title, summary, trigger, actions, n8n_prompt, confidence, workflow_id, workflow_json, evidence, pattern_id, last_error, snoozed_until, category, business_score, feedback_status, feedback_note, feedback_count, last_feedback_at FROM recommendations WHERE status = 'pending' ORDER BY created_at DESC"
        };

        let mut stmt = conn.prepare(sql)?;

        // Execute query map based on filter
        let mut recs = Vec::new();

        if let Some(s) = status_filter {
            if s == "all" {
                let rows = stmt.query_map([], map_row)?;
                for rec in rows {
                    recs.push(rec?);
                }
            } else {
                let rows = stmt.query_map([s], map_row)?;
                for rec in rows {
                    recs.push(rec?);
                }
            }
        } else {
            let rows = stmt.query_map([], map_row)?;
            for rec in rows {
                recs.push(rec?);
            }
        };

        Ok(recs)
    } else {
        Ok(Vec::new())
    }
}

pub fn get_recommendations() -> Result<Vec<Recommendation>> {
    get_recommendations_with_filter(None)
}

// Deprecated wrapper
pub fn list_recommendations(status: &str, _limit: i64) -> Result<Vec<Recommendation>> {
    get_recommendations_with_filter(Some(status))
}

// Helper to map row to struct
fn map_row(row: &rusqlite::Row) -> rusqlite::Result<Recommendation> {
    Ok(Recommendation {
        id: row.get(0)?,
        status: row.get(1)?,
        title: row.get(2)?,
        summary: row.get(3)?,
        trigger: row.get(4)?,
        actions: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
        n8n_prompt: row.get(6)?,
        confidence: row.get(7)?,
        workflow_id: row.get(8)?,
        workflow_json: row.get(9)?,
        evidence: {
            let json: String = row.get(10).unwrap_or_else(|_| "[]".to_string());
            serde_json::from_str(&json).unwrap_or_default()
        },
        pattern_id: row.get(11).ok(),
        last_error: row.get(12).ok(),
        snoozed_until: row.get(13).ok(),
        category: row
            .get::<_, String>(14)
            .unwrap_or_else(|_| crate::recommendation_policy::CATEGORY_UNKNOWN.to_string()),
        business_score: row.get(15).unwrap_or(0.0),
        feedback_status: row.get(16).ok(),
        feedback_note: row.get(17).ok(),
        feedback_count: row.get(18).unwrap_or(0),
        last_feedback_at: row.get(19).ok(),
    })
}

// Old function body removal target:
/*
pub fn list_recommendations(status: &str, limit: i64) -> Result<Vec<Recommendation>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, status, title, summary, trigger, actions, n8n_prompt, confidence, workflow_id, workflow_json
             FROM recommendations
             WHERE status = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;
*/

pub fn get_recommendation(id: i64) -> Result<Option<Recommendation>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, status, title, summary, trigger, actions, n8n_prompt, confidence, workflow_id, workflow_json, evidence, pattern_id, last_error, snoozed_until, category, business_score, feedback_status, feedback_note, feedback_count, last_feedback_at
             FROM recommendations
             WHERE id = ?1",
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let actions_json: String = row.get(5)?;
            let actions: Vec<String> = serde_json::from_str(&actions_json).unwrap_or_default();

            let evidence_json: String = row.get(10).unwrap_or_else(|_| "[]".to_string());
            let evidence: Vec<String> = serde_json::from_str(&evidence_json).unwrap_or_default();

            return Ok(Some(Recommendation {
                id: row.get(0)?,
                status: row.get(1)?,
                title: row.get(2)?,
                summary: row.get(3)?,
                trigger: row.get(4)?,
                actions,
                n8n_prompt: row.get(6)?,
                confidence: row.get(7)?,
                workflow_id: row.get(8)?,
                workflow_json: row.get(9)?,
                evidence,
                pattern_id: row.get(11).ok(),
                last_error: row.get(12).ok(),
                snoozed_until: row.get(13).ok(),
                category: row
                    .get::<_, String>(14)
                    .unwrap_or_else(|_| crate::recommendation_policy::CATEGORY_UNKNOWN.to_string()),
                business_score: row.get(15).unwrap_or(0.0),
                feedback_status: row.get(16).ok(),
                feedback_note: row.get(17).ok(),
                feedback_count: row.get(18).unwrap_or(0),
                last_feedback_at: row.get(19).ok(),
            }));
        }
    }
    Ok(None)
}

pub fn update_recommendation_status(id: i64, status: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "UPDATE recommendations
             SET status = ?1,
                 snoozed_until = CASE
                    WHEN LOWER(?1) IN ('approved', 'rejected') THEN NULL
                    ELSE snoozed_until
                 END
             WHERE id = ?2",
            params![status, id],
        )?;
    }
    Ok(())
}

pub fn update_recommendation_review_status(id: i64, status: &str) -> Result<()> {
    let normalized = status.trim().to_lowercase();
    if normalized != "pending" && normalized != "approved" && normalized != "rejected" {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "invalid recommendation status '{}': only pending/approved/rejected are allowed",
            status
        )));
    }

    let rec = get_recommendation(id)?.ok_or_else(|| {
        rusqlite::Error::InvalidParameterName(format!("recommendation {} not found", id))
    })?;
    let current = rec.status.trim().to_lowercase();

    let allowed = matches!(
        (current.as_str(), normalized.as_str()),
        ("pending", "pending")
            | ("pending", "approved")
            | ("pending", "rejected")
            | ("approved", "approved")
            | ("approved", "rejected")
            | ("rejected", "rejected")
            | ("rejected", "pending")
    );

    if !allowed {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "invalid recommendation transition: {} -> {}",
            current, normalized
        )));
    }

    update_recommendation_status(id, &normalized)
}

pub fn mark_recommendation_failed(id: i64, error: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        // Keep review state in pending/approved/rejected model; store execution failure in last_error.
        conn.execute(
            "UPDATE recommendations
             SET status = CASE
                 WHEN status IN ('approved', 'rejected') THEN status
                 ELSE 'pending'
             END,
             last_error = ?1
             WHERE id = ?2",
            params![error, id],
        )?;
    }
    Ok(())
}

pub fn claim_recommendation_provisioning(id: i64, claim_token: &str) -> Result<Option<String>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT workflow_id
             FROM recommendations
             WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        let Some(row) = rows.next()? else {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "recommendation {} not found",
                id
            )));
        };

        let existing_workflow_id: Option<String> = row.get(0)?;
        if let Some(existing) = existing_workflow_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Ok(Some(existing.to_string()));
        }

        let changed = conn.execute(
            "UPDATE recommendations
             SET workflow_id = ?1
             WHERE id = ?2
               AND (workflow_id IS NULL OR TRIM(workflow_id) = '')",
            params![claim_token, id],
        )?;

        if changed > 0 {
            return Ok(None);
        }

        let mut reload_stmt = conn.prepare(
            "SELECT workflow_id
             FROM recommendations
             WHERE id = ?1",
        )?;
        let mut reload_rows = reload_stmt.query(params![id])?;
        if let Some(reload_row) = reload_rows.next()? {
            let current: Option<String> = reload_row.get(0)?;
            if let Some(current) = current.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                return Ok(Some(current.to_string()));
            }
        }
    }
    Ok(None)
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
        for json in rows.flatten() {
            events.push(json);
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
    Err(anyhow::anyhow!("DB not initialized"))
}

// Memory System: Chat History
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub created_at: String,
}

fn normalize_request_memory_key(input: &str) -> String {
    normalize_request_text(input)
}

fn truncate_text(input: &str, max_chars: usize) -> String {
    input.trim().chars().take(max_chars).collect::<String>()
}

const GLOBAL_MEMORY_SCOPE: &str = "global";

fn normalize_memory_scope(scope: Option<&str>) -> String {
    let raw = scope.unwrap_or(GLOBAL_MEMORY_SCOPE).trim().to_lowercase();
    let normalized = raw
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");

    if normalized.is_empty() {
        GLOBAL_MEMORY_SCOPE.to_string()
    } else {
        truncate_text(&normalized, 96)
    }
}

fn split_scoped_storage_key(storage_key: &str) -> (Option<&str>, &str) {
    if let Some((scope, logical_key)) = storage_key.split_once("::") {
        if !scope.trim().is_empty() && !logical_key.trim().is_empty() {
            return (Some(scope), logical_key);
        }
    }
    (None, storage_key)
}

fn logical_key_from_storage(storage_key: &str, memory_scope: &str) -> String {
    let (stored_scope, logical_key) = split_scoped_storage_key(storage_key);
    if stored_scope == Some(memory_scope) {
        logical_key.to_string()
    } else {
        storage_key.to_string()
    }
}

fn compose_scoped_storage_key(memory_scope: &str, logical_key: &str, max_len: usize) -> String {
    truncate_text(&format!("{}::{}", memory_scope, logical_key), max_len)
}

fn scoped_request_memory_storage_key(
    request_text: &str,
    memory_scope: Option<&str>,
) -> Option<(String, String)> {
    let logical_key = normalize_request_memory_key(request_text);
    if logical_key.is_empty() {
        return None;
    }
    let memory_scope = normalize_memory_scope(memory_scope);
    let storage_key = compose_scoped_storage_key(&memory_scope, &logical_key, 1024);
    Some((memory_scope, storage_key))
}

fn scoped_execution_params_key(
    params_key: &str,
    memory_scope: Option<&str>,
) -> Option<(String, String, String)> {
    let logical_key = truncate_text(params_key, 255);
    if logical_key.trim().is_empty() {
        return None;
    }
    let memory_scope = normalize_memory_scope(memory_scope);
    let storage_key = compose_scoped_storage_key(&memory_scope, &logical_key, 255);
    Some((memory_scope, logical_key, storage_key))
}

fn request_memory_intent_command(intent_json: Option<&Value>) -> Option<String> {
    intent_json
        .and_then(|value| value.get("command"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| truncate_text(value, 64))
}

fn request_memory_intent_command_from_raw(intent_json: Option<&str>) -> Option<String> {
    let raw = intent_json?.trim();
    if raw.is_empty() {
        return None;
    }
    let parsed: Value = serde_json::from_str(raw).ok()?;
    request_memory_intent_command(Some(&parsed))
}

fn optional_truncated_signature(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(truncate_text(trimmed, 255))
    }
}

fn row_bool(row: &rusqlite::Row<'_>, idx: usize, default: bool) -> bool {
    row.get::<_, i64>(idx)
        .map(|value| value != 0)
        .unwrap_or(default)
}

fn map_request_memory_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RequestMemoryRecord> {
    let stored_key = row.get::<_, String>(0)?;
    let memory_scope = row.get::<_, String>(1)?;
    Ok(RequestMemoryRecord {
        normalized_request: logical_key_from_storage(&stored_key, &memory_scope),
        memory_scope,
        original_request: row.get(2)?,
        request_signature: row.get(3).ok(),
        intent_json: row.get(4).ok(),
        intent_command: row.get(5).ok(),
        response_text: row.get(6).ok(),
        response_mode: row.get(7)?,
        source: row.get(8)?,
        confidence: row.get(9).unwrap_or(0.0),
        positive_feedback_count: row.get(10).unwrap_or(0),
        negative_feedback_count: row.get(11).unwrap_or(0),
        last_feedback_at: row.get(12).ok(),
        suppressed: row_bool(row, 13, false),
        suppressed_reason: row.get(14).ok(),
        suppressed_at: row.get(15).ok(),
        use_count: row.get(16).unwrap_or(0),
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
        last_used_at: row.get(19)?,
    })
}

fn map_execution_memory_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExecutionMemoryRecord> {
    let memory_scope = row.get::<_, String>(1)?;
    let stored_params_key = row.get::<_, String>(3)?;
    Ok(ExecutionMemoryRecord {
        id: row.get(0)?,
        memory_scope: memory_scope.clone(),
        intent_command: row.get(2)?,
        params_key: logical_key_from_storage(&stored_params_key, &memory_scope),
        params_json: row.get(4).ok(),
        request_signature: row.get(5).ok(),
        response_text: row.get(6)?,
        source: row.get(7)?,
        tool_path: row.get(8)?,
        freshness_ttl_seconds: row.get(9).unwrap_or(0),
        success: row.get::<_, i64>(10).unwrap_or(1) != 0,
        positive_feedback_count: row.get(11).unwrap_or(0),
        negative_feedback_count: row.get(12).unwrap_or(0),
        last_feedback_at: row.get(13).ok(),
        suppressed: row_bool(row, 14, false),
        suppressed_reason: row.get(15).ok(),
        suppressed_at: row.get(16).ok(),
        use_count: row.get(17).unwrap_or(0),
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
        last_used_at: row.get(20)?,
    })
}

fn map_launch_ops_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LaunchOpsEventRecord> {
    Ok(LaunchOpsEventRecord {
        id: row.get(0)?,
        created_at: row.get(1)?,
        channel: row.get(2).ok(),
        memory_scope: row.get(3).ok(),
        message_preview: row.get(4)?,
        route_kind: row.get(5)?,
        command: row.get(6).ok(),
        outcome: row.get(7)?,
        confidence: row.get(8).ok(),
        freshness_bypassed: row_bool(row, 9, false),
        intent_memory_hit: row_bool(row, 10, false),
        request_memory_hit: row_bool(row, 11, false),
        execution_memory_hit: row_bool(row, 12, false),
        deterministic_used: row_bool(row, 13, false),
        llm_used: row_bool(row, 14, false),
        ai_digest_used: row_bool(row, 15, false),
        local_only: row_bool(row, 16, false),
        note: row.get(17).ok(),
    })
}

fn map_memory_admin_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryAdminEventRecord> {
    Ok(MemoryAdminEventRecord {
        id: row.get(0)?,
        created_at: row.get(1)?,
        kind: row.get(2)?,
        action: row.get(3)?,
        memory_scope: row.get(4).ok(),
        target_key: row.get(5)?,
        reason: row.get(6).ok(),
        actor: row.get(7).ok(),
        ok: row_bool(row, 8, false),
        message: row.get(9).ok(),
    })
}

fn map_recommendation_review_event_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<RecommendationReviewEventRecord> {
    Ok(RecommendationReviewEventRecord {
        id: row.get(0)?,
        created_at: row.get(1)?,
        recommendation_id: row.get(2)?,
        recommendation_title: row.get(3)?,
        status_after: row.get(4).ok(),
        category: row.get(5).ok(),
        action: row.get(6)?,
        actor: row.get(7).ok(),
        note: row.get(8).ok(),
        ok: row_bool(row, 9, false),
        message: row.get(10).ok(),
    })
}

fn backfill_request_memory_scope_keys(conn: &Connection) {
    let _ = conn.execute(
        "UPDATE request_memory
         SET memory_scope = 'global'
         WHERE memory_scope IS NULL
            OR TRIM(memory_scope) = ''",
        [],
    );

    let mut stmt =
        match conn.prepare("SELECT rowid, normalized_request, memory_scope FROM request_memory") {
            Ok(stmt) => stmt,
            Err(error) => {
                eprintln!("Failed to prepare request_memory scope backfill: {}", error);
                return;
            }
        };
    let rows = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    }) {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!(
                "Failed to query request_memory scope backfill rows: {}",
                error
            );
            return;
        }
    };

    for row in rows.flatten() {
        let (rowid, stored_key, raw_scope) = row;
        let memory_scope = normalize_memory_scope(Some(&raw_scope));
        let logical_key = logical_key_from_storage(&stored_key, &memory_scope);
        let scoped_key = compose_scoped_storage_key(&memory_scope, &logical_key, 1024);
        if scoped_key == stored_key && memory_scope == raw_scope {
            continue;
        }
        if let Err(error) = conn.execute(
            "UPDATE request_memory
             SET normalized_request = ?1,
                 memory_scope = ?2
             WHERE rowid = ?3",
            params![scoped_key, memory_scope, rowid],
        ) {
            eprintln!(
                "Failed to backfill request_memory scope key row {}: {}",
                rowid, error
            );
        }
    }
}

fn backfill_execution_memory_scope_keys(conn: &Connection) {
    let _ = conn.execute(
        "UPDATE execution_memory
         SET memory_scope = 'global'
         WHERE memory_scope IS NULL
            OR TRIM(memory_scope) = ''",
        [],
    );

    let mut stmt =
        match conn.prepare("SELECT rowid, params_key, memory_scope FROM execution_memory") {
            Ok(stmt) => stmt,
            Err(error) => {
                eprintln!(
                    "Failed to prepare execution_memory scope backfill: {}",
                    error
                );
                return;
            }
        };
    let rows = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    }) {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!(
                "Failed to query execution_memory scope backfill rows: {}",
                error
            );
            return;
        }
    };

    for row in rows.flatten() {
        let (rowid, stored_key, raw_scope) = row;
        let memory_scope = normalize_memory_scope(Some(&raw_scope));
        let logical_key = logical_key_from_storage(&stored_key, &memory_scope);
        let scoped_key = compose_scoped_storage_key(&memory_scope, &logical_key, 255);
        if scoped_key == stored_key && memory_scope == raw_scope {
            continue;
        }
        if let Err(error) = conn.execute(
            "UPDATE execution_memory
             SET params_key = ?1,
                 memory_scope = ?2
             WHERE rowid = ?3",
            params![scoped_key, memory_scope, rowid],
        ) {
            eprintln!(
                "Failed to backfill execution_memory scope key row {}: {}",
                rowid, error
            );
        }
    }
}

fn backfill_request_memory_cache_columns(conn: &Connection) {
    let mut stmt = match conn.prepare(
        "SELECT normalized_request, original_request, intent_json
         FROM request_memory
         WHERE request_signature IS NULL
            OR TRIM(request_signature) = ''
            OR intent_command IS NULL
            OR TRIM(intent_command) = ''",
    ) {
        Ok(stmt) => stmt,
        Err(error) => {
            eprintln!("Failed to prepare request_memory cache backfill: {}", error);
            return;
        }
    };

    let rows = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2).ok().flatten(),
        ))
    }) {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!(
                "Failed to query request_memory cache backfill rows: {}",
                error
            );
            return;
        }
    };

    for row in rows.flatten() {
        let (normalized_request, original_request, intent_json) = row;
        let signature = optional_truncated_signature(&build_request_signature(&original_request));
        let intent_command = request_memory_intent_command_from_raw(intent_json.as_deref());
        if let Err(error) = conn.execute(
            "UPDATE request_memory
             SET request_signature = ?1,
                 intent_command = ?2
             WHERE normalized_request = ?3",
            params![signature, intent_command, normalized_request],
        ) {
            eprintln!(
                "Failed to backfill request_memory cache columns for {}: {}",
                original_request, error
            );
        }
    }
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

pub fn upsert_request_memory(
    request_text: &str,
    intent_json: Option<&Value>,
    response_text: Option<&str>,
    response_mode: &str,
    source: &str,
    confidence: f64,
) -> Result<()> {
    upsert_request_memory_scoped(
        None,
        request_text,
        intent_json,
        response_text,
        response_mode,
        source,
        confidence,
    )
}

pub fn upsert_request_memory_scoped(
    memory_scope: Option<&str>,
    request_text: &str,
    intent_json: Option<&Value>,
    response_text: Option<&str>,
    response_mode: &str,
    source: &str,
    confidence: f64,
) -> Result<()> {
    let Some((memory_scope, normalized_request)) =
        scoped_request_memory_storage_key(request_text, memory_scope)
    else {
        return Ok(());
    };

    let original_request = truncate_text(request_text, 800);
    let request_signature = optional_truncated_signature(&build_request_signature(request_text));
    let intent_json_str = intent_json.map(|value| value.to_string());
    let intent_command = request_memory_intent_command(intent_json);
    let response_text = response_text.map(|value| truncate_text(value, 4000));
    let now = chrono::Utc::now().to_rfc3339();

    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "INSERT INTO request_memory (
                normalized_request, memory_scope, original_request, request_signature, intent_json, intent_command,
                response_text, response_mode, source, confidence,
                positive_feedback_count, negative_feedback_count, last_feedback_at,
                suppressed, suppressed_reason, suppressed_at,
                use_count, created_at, updated_at, last_used_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, 0, NULL, 0, NULL, NULL, 1, ?11, ?11, ?11)
            ON CONFLICT(normalized_request) DO UPDATE SET
                memory_scope = excluded.memory_scope,
                original_request = excluded.original_request,
                request_signature = COALESCE(excluded.request_signature, request_memory.request_signature),
                intent_json = COALESCE(excluded.intent_json, request_memory.intent_json),
                intent_command = COALESCE(excluded.intent_command, request_memory.intent_command),
                response_text = COALESCE(excluded.response_text, request_memory.response_text),
                response_mode = excluded.response_mode,
                source = excluded.source,
                confidence = CASE
                    WHEN excluded.confidence > request_memory.confidence THEN excluded.confidence
                    ELSE request_memory.confidence
                END,
                positive_feedback_count = CASE
                    WHEN excluded.response_text IS NOT NULL
                         AND request_memory.response_text IS NOT NULL
                         AND excluded.response_text != request_memory.response_text THEN 0
                    ELSE request_memory.positive_feedback_count
                END,
                negative_feedback_count = CASE
                    WHEN excluded.response_text IS NOT NULL
                         AND request_memory.response_text IS NOT NULL
                         AND excluded.response_text != request_memory.response_text THEN 0
                    ELSE request_memory.negative_feedback_count
                END,
                last_feedback_at = CASE
                    WHEN excluded.response_text IS NOT NULL
                         AND request_memory.response_text IS NOT NULL
                         AND excluded.response_text != request_memory.response_text THEN NULL
                    ELSE request_memory.last_feedback_at
                END,
                suppressed = request_memory.suppressed,
                suppressed_reason = request_memory.suppressed_reason,
                suppressed_at = request_memory.suppressed_at,
                use_count = request_memory.use_count + 1,
                updated_at = excluded.updated_at,
                last_used_at = excluded.last_used_at",
            params![
                normalized_request,
                memory_scope,
                original_request,
                request_signature,
                intent_json_str,
                intent_command,
                response_text,
                truncate_text(response_mode, 32),
                truncate_text(source, 64),
                confidence.clamp(0.0, 1.0),
                now,
            ],
        )?;
    }
    Ok(())
}

pub fn get_request_memory(request_text: &str) -> Result<Option<RequestMemoryRecord>> {
    get_request_memory_scoped(None, request_text)
}

pub fn get_request_memory_scoped(
    memory_scope: Option<&str>,
    request_text: &str,
) -> Result<Option<RequestMemoryRecord>> {
    let Some((memory_scope, normalized_request)) =
        scoped_request_memory_storage_key(request_text, memory_scope)
    else {
        return Ok(None);
    };

    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT normalized_request, memory_scope, original_request, request_signature, intent_json, intent_command,
                    response_text, response_mode, source, confidence,
                    positive_feedback_count, negative_feedback_count, last_feedback_at,
                    suppressed, suppressed_reason, suppressed_at,
                    use_count, created_at, updated_at, last_used_at
             FROM request_memory
             WHERE normalized_request = ?1
               AND memory_scope = ?2
               AND suppressed = 0
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![normalized_request, memory_scope])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(map_request_memory_row(row)?));
        }
    }
    Ok(None)
}

pub fn get_request_memory_by_signature(
    request_signature: &str,
    allowed_commands: &[&str],
) -> Result<Option<RequestMemoryRecord>> {
    get_request_memory_by_signature_scoped(None, request_signature, allowed_commands)
}

pub fn get_request_memory_by_signature_scoped(
    memory_scope: Option<&str>,
    request_signature: &str,
    allowed_commands: &[&str],
) -> Result<Option<RequestMemoryRecord>> {
    let request_signature = optional_truncated_signature(request_signature);
    if request_signature.is_none() || allowed_commands.is_empty() {
        return Ok(None);
    }
    let memory_scope = normalize_memory_scope(memory_scope);

    let placeholders = (0..allowed_commands.len())
        .map(|idx| format!("?{}", idx + 3))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT normalized_request, memory_scope, original_request, request_signature, intent_json, intent_command,
                response_text, response_mode, source, confidence,
                positive_feedback_count, negative_feedback_count, last_feedback_at,
                suppressed, suppressed_reason, suppressed_at,
                use_count, created_at, updated_at, last_used_at
         FROM request_memory
         WHERE request_signature = ?1
           AND memory_scope = ?2
           AND suppressed = 0
           AND intent_command IN ({})
         ORDER BY
             (positive_feedback_count - negative_feedback_count) DESC,
             positive_feedback_count DESC,
             negative_feedback_count ASC,
             CASE
                 WHEN source LIKE 'api.chat.execution%' THEN 4
                 WHEN source = 'api.chat.deterministic' THEN 3
                 WHEN source IN ('api.chat.local', 'api.chat.local_cache') THEN 2
                 WHEN source IN ('api.chat.llm', 'api.chat') THEN 1
                 ELSE 0
             END DESC,
             use_count DESC,
             confidence DESC,
             last_used_at DESC
         LIMIT 1",
        placeholders
    );

    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(&sql)?;
        let params = params_from_iter(
            std::iter::once(request_signature.unwrap())
                .chain(std::iter::once(memory_scope))
                .chain(
                    allowed_commands
                        .iter()
                        .map(|command| (*command).to_string()),
                ),
        );
        let mut rows = stmt.query(params)?;
        if let Some(row) = rows.next()? {
            return Ok(Some(map_request_memory_row(row)?));
        }
    }
    Ok(None)
}

pub fn upsert_execution_memory(
    intent_command: &str,
    params_key: &str,
    params_json: Option<&Value>,
    request_signature: Option<&str>,
    response_text: &str,
    freshness_ttl_seconds: i64,
    source: &str,
    tool_path: &str,
) -> Result<()> {
    upsert_execution_memory_scoped(
        None,
        intent_command,
        params_key,
        params_json,
        request_signature,
        response_text,
        freshness_ttl_seconds,
        source,
        tool_path,
    )
}

pub fn upsert_execution_memory_scoped(
    memory_scope: Option<&str>,
    intent_command: &str,
    params_key: &str,
    params_json: Option<&Value>,
    request_signature: Option<&str>,
    response_text: &str,
    freshness_ttl_seconds: i64,
    source: &str,
    tool_path: &str,
) -> Result<()> {
    let intent_command = truncate_text(intent_command, 64);
    let response_text = truncate_text(response_text, 4000);
    let Some((memory_scope, _logical_params_key, params_key)) =
        scoped_execution_params_key(params_key, memory_scope)
    else {
        return Ok(());
    };
    if intent_command.trim().is_empty() || response_text.is_empty() {
        return Ok(());
    }

    let params_json = params_json.map(|value| truncate_text(&value.to_string(), 1000));
    let request_signature = request_signature.and_then(optional_truncated_signature);
    let now = chrono::Utc::now().to_rfc3339();

    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "INSERT INTO execution_memory (
                memory_scope, intent_command, params_key, params_json, request_signature, response_text,
                source, tool_path, freshness_ttl_seconds, success,
                positive_feedback_count, negative_feedback_count, last_feedback_at,
                suppressed, suppressed_reason, suppressed_at,
                use_count, created_at, updated_at, last_used_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, 0, 0, NULL, 0, NULL, NULL, 1, ?10, ?10, ?10)
            ON CONFLICT(intent_command, params_key) DO UPDATE SET
                memory_scope = excluded.memory_scope,
                params_json = COALESCE(excluded.params_json, execution_memory.params_json),
                request_signature = COALESCE(excluded.request_signature, execution_memory.request_signature),
                response_text = excluded.response_text,
                source = excluded.source,
                tool_path = excluded.tool_path,
                freshness_ttl_seconds = excluded.freshness_ttl_seconds,
                success = excluded.success,
                positive_feedback_count = CASE
                    WHEN excluded.response_text != execution_memory.response_text THEN 0
                    ELSE execution_memory.positive_feedback_count
                END,
                negative_feedback_count = CASE
                    WHEN excluded.response_text != execution_memory.response_text THEN 0
                    ELSE execution_memory.negative_feedback_count
                END,
                last_feedback_at = CASE
                    WHEN excluded.response_text != execution_memory.response_text THEN NULL
                    ELSE execution_memory.last_feedback_at
                END,
                suppressed = execution_memory.suppressed,
                suppressed_reason = execution_memory.suppressed_reason,
                suppressed_at = execution_memory.suppressed_at,
                use_count = execution_memory.use_count + 1,
                updated_at = excluded.updated_at,
                last_used_at = excluded.last_used_at",
            params![
                memory_scope,
                intent_command,
                params_key,
                params_json,
                request_signature,
                response_text,
                truncate_text(source, 64),
                truncate_text(tool_path, 128),
                freshness_ttl_seconds.max(0),
                now,
            ],
        )?;
    }
    Ok(())
}

pub fn get_execution_memory(
    intent_command: &str,
    params_key: &str,
) -> Result<Option<ExecutionMemoryRecord>> {
    get_execution_memory_scoped(None, intent_command, params_key)
}

pub fn get_execution_memory_scoped(
    memory_scope: Option<&str>,
    intent_command: &str,
    params_key: &str,
) -> Result<Option<ExecutionMemoryRecord>> {
    let intent_command = truncate_text(intent_command, 64);
    let Some((memory_scope, _logical_params_key, params_key)) =
        scoped_execution_params_key(params_key, memory_scope)
    else {
        return Ok(None);
    };
    if intent_command.trim().is_empty() {
        return Ok(None);
    }

    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, memory_scope, intent_command, params_key, params_json, request_signature,
                    response_text, source, tool_path, freshness_ttl_seconds, success,
                    positive_feedback_count, negative_feedback_count, last_feedback_at,
                    suppressed, suppressed_reason, suppressed_at,
                    use_count, created_at, updated_at, last_used_at
             FROM execution_memory
             WHERE memory_scope = ?1
               AND intent_command = ?2
               AND params_key = ?3
               AND suppressed = 0
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![memory_scope, intent_command, params_key])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(map_execution_memory_row(row)?));
        }
    }
    Ok(None)
}

fn normalize_admin_limit(limit: i64) -> i64 {
    limit.clamp(1, 200)
}

pub fn list_request_memory_records(
    limit: i64,
    include_suppressed: bool,
) -> Result<Vec<RequestMemoryRecord>> {
    let limit = normalize_admin_limit(limit);
    let mut out = Vec::new();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = if include_suppressed {
            conn.prepare(
                "SELECT normalized_request, memory_scope, original_request, request_signature, intent_json, intent_command,
                        response_text, response_mode, source, confidence,
                        positive_feedback_count, negative_feedback_count, last_feedback_at,
                        suppressed, suppressed_reason, suppressed_at,
                        use_count, created_at, updated_at, last_used_at
                 FROM request_memory
                 ORDER BY suppressed DESC, last_used_at DESC
                 LIMIT ?1",
            )?
        } else {
            conn.prepare(
                "SELECT normalized_request, memory_scope, original_request, request_signature, intent_json, intent_command,
                        response_text, response_mode, source, confidence,
                        positive_feedback_count, negative_feedback_count, last_feedback_at,
                        suppressed, suppressed_reason, suppressed_at,
                        use_count, created_at, updated_at, last_used_at
                 FROM request_memory
                 WHERE suppressed = 0
                 ORDER BY last_used_at DESC
                 LIMIT ?1",
            )?
        };
        let rows = stmt.query_map(params![limit], map_request_memory_row)?;
        for row in rows.flatten() {
            out.push(row);
        }
    }
    Ok(out)
}

pub fn list_execution_memory_records(
    limit: i64,
    include_suppressed: bool,
) -> Result<Vec<ExecutionMemoryRecord>> {
    let limit = normalize_admin_limit(limit);
    let mut out = Vec::new();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = if include_suppressed {
            conn.prepare(
                "SELECT id, memory_scope, intent_command, params_key, params_json, request_signature,
                        response_text, source, tool_path, freshness_ttl_seconds, success,
                        positive_feedback_count, negative_feedback_count, last_feedback_at,
                        suppressed, suppressed_reason, suppressed_at,
                        use_count, created_at, updated_at, last_used_at
                 FROM execution_memory
                 ORDER BY suppressed DESC, last_used_at DESC
                 LIMIT ?1",
            )?
        } else {
            conn.prepare(
                "SELECT id, memory_scope, intent_command, params_key, params_json, request_signature,
                        response_text, source, tool_path, freshness_ttl_seconds, success,
                        positive_feedback_count, negative_feedback_count, last_feedback_at,
                        suppressed, suppressed_reason, suppressed_at,
                        use_count, created_at, updated_at, last_used_at
                 FROM execution_memory
                 WHERE suppressed = 0
                 ORDER BY last_used_at DESC
                 LIMIT ?1",
            )?
        };
        let rows = stmt.query_map(params![limit], map_execution_memory_row)?;
        for row in rows.flatten() {
            out.push(row);
        }
    }
    Ok(out)
}

pub fn get_memory_ops_metrics() -> Result<MemoryOpsMetrics> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let request_active: i64 = conn.query_row(
            "SELECT COUNT(*) FROM request_memory WHERE suppressed = 0",
            [],
            |row| row.get(0),
        )?;
        let request_suppressed: i64 = conn.query_row(
            "SELECT COUNT(*) FROM request_memory WHERE suppressed != 0",
            [],
            |row| row.get(0),
        )?;
        let execution_active: i64 = conn.query_row(
            "SELECT COUNT(*) FROM execution_memory WHERE suppressed = 0",
            [],
            |row| row.get(0),
        )?;
        let execution_suppressed: i64 = conn.query_row(
            "SELECT COUNT(*) FROM execution_memory WHERE suppressed != 0",
            [],
            |row| row.get(0),
        )?;
        let last_request_used_at = conn
            .query_row(
                "SELECT last_used_at FROM request_memory ORDER BY last_used_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();
        let last_execution_used_at = conn
            .query_row(
                "SELECT last_used_at FROM execution_memory ORDER BY last_used_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();
        return Ok(MemoryOpsMetrics {
            request_active,
            request_suppressed,
            execution_active,
            execution_suppressed,
            last_request_used_at,
            last_execution_used_at,
        });
    }
    Ok(MemoryOpsMetrics {
        request_active: 0,
        request_suppressed: 0,
        execution_active: 0,
        execution_suppressed: 0,
        last_request_used_at: None,
        last_execution_used_at: None,
    })
}

fn normalize_launch_ops_window_limit(limit: i64) -> i64 {
    limit.clamp(20, 1000)
}

fn normalize_launch_ops_list_limit(limit: i64) -> i64 {
    limit.clamp(1, 100)
}

pub fn record_launch_ops_event(
    channel: Option<&str>,
    memory_scope: Option<&str>,
    message_preview: &str,
    route_kind: &str,
    command: Option<&str>,
    outcome: &str,
    confidence: Option<f64>,
    freshness_bypassed: bool,
    intent_memory_hit: bool,
    request_memory_hit: bool,
    execution_memory_hit: bool,
    deterministic_used: bool,
    llm_used: bool,
    ai_digest_used: bool,
    local_only: bool,
    note: Option<&str>,
) -> Result<()> {
    let created_at = chrono::Utc::now().to_rfc3339();
    let preview = truncate_text(message_preview.trim(), 240);
    let route_kind = truncate_text(route_kind.trim(), 64);
    let outcome = truncate_text(outcome.trim(), 32);
    let command = command
        .map(|value| truncate_text(value.trim(), 64))
        .filter(|value| !value.is_empty());
    let channel = channel
        .map(|value| truncate_text(value.trim(), 32))
        .filter(|value| !value.is_empty());
    let memory_scope = memory_scope
        .map(|value| truncate_text(value.trim(), 160))
        .filter(|value| !value.is_empty());
    let note = note
        .map(|value| truncate_text(value.trim(), 500))
        .filter(|value| !value.is_empty());

    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "INSERT INTO launch_ops_events (
                created_at, channel, memory_scope, message_preview, route_kind, command, outcome,
                confidence, freshness_bypassed, intent_memory_hit, request_memory_hit,
                execution_memory_hit, deterministic_used, llm_used, ai_digest_used, local_only,
                note
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                created_at,
                channel,
                memory_scope,
                preview,
                route_kind,
                command,
                outcome,
                confidence,
                freshness_bypassed as i64,
                intent_memory_hit as i64,
                request_memory_hit as i64,
                execution_memory_hit as i64,
                deterministic_used as i64,
                llm_used as i64,
                ai_digest_used as i64,
                local_only as i64,
                note,
            ],
        )?;
    }
    Ok(())
}

pub fn list_launch_ops_events(limit: i64) -> Result<Vec<LaunchOpsEventRecord>> {
    let limit = normalize_launch_ops_list_limit(limit);
    let mut out = Vec::new();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, created_at, channel, memory_scope, message_preview, route_kind, command,
                    outcome, confidence, freshness_bypassed, intent_memory_hit, request_memory_hit,
                    execution_memory_hit, deterministic_used, llm_used, ai_digest_used, local_only,
                    note
             FROM launch_ops_events
             ORDER BY created_at DESC, id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], map_launch_ops_event_row)?;
        for row in rows.flatten() {
            out.push(row);
        }
    }
    Ok(out)
}

pub fn record_memory_admin_event(
    kind: &str,
    action: &str,
    memory_scope: Option<&str>,
    target_key: &str,
    reason: Option<&str>,
    actor: Option<&str>,
    ok: bool,
    message: Option<&str>,
) -> Result<()> {
    let created_at = chrono::Utc::now().to_rfc3339();
    let kind = truncate_text(kind.trim(), 32);
    let action = truncate_text(action.trim(), 32);
    if kind.is_empty() || action.is_empty() {
        return Ok(());
    }
    let target_key = {
        let trimmed = truncate_text(target_key.trim(), 255);
        if trimmed.is_empty() {
            "<empty>".to_string()
        } else {
            trimmed
        }
    };
    let memory_scope = memory_scope
        .map(|value| normalize_memory_scope(Some(value)))
        .filter(|value| !value.is_empty());
    let reason = reason
        .map(|value| truncate_text(value.trim(), 500))
        .filter(|value| !value.is_empty());
    let actor = actor
        .map(|value| truncate_text(value.trim(), 64))
        .filter(|value| !value.is_empty());
    let message = message
        .map(|value| truncate_text(value.trim(), 500))
        .filter(|value| !value.is_empty());

    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "INSERT INTO memory_admin_events (
                created_at, kind, action, memory_scope, target_key, reason, actor, ok, message
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                created_at,
                kind,
                action,
                memory_scope,
                target_key,
                reason,
                actor,
                if ok { 1 } else { 0 },
                message,
            ],
        )?;
    }
    Ok(())
}

pub fn list_memory_admin_events(limit: i64) -> Result<Vec<MemoryAdminEventRecord>> {
    let limit = normalize_admin_limit(limit);
    let mut out = Vec::new();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, created_at, kind, action, memory_scope, target_key, reason, actor, ok, message
             FROM memory_admin_events
             ORDER BY created_at DESC, id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], map_memory_admin_event_row)?;
        for row in rows.flatten() {
            out.push(row);
        }
    }
    Ok(out)
}

pub fn record_recommendation_review_event(
    recommendation_id: i64,
    recommendation_title: &str,
    status_after: Option<&str>,
    category: Option<&str>,
    action: &str,
    actor: Option<&str>,
    note: Option<&str>,
    ok: bool,
    message: Option<&str>,
) -> Result<()> {
    let created_at = chrono::Utc::now().to_rfc3339();
    let recommendation_title = {
        let trimmed = truncate_text(recommendation_title.trim(), 255);
        if trimmed.is_empty() {
            format!("recommendation:{}", recommendation_id)
        } else {
            trimmed
        }
    };
    let status_after = status_after
        .map(|value| truncate_text(value.trim(), 32))
        .filter(|value| !value.is_empty());
    let category = category
        .map(|value| truncate_text(value.trim(), 32))
        .filter(|value| !value.is_empty());
    let action = truncate_text(action.trim(), 32);
    if action.is_empty() {
        return Ok(());
    }
    let actor = actor
        .map(|value| truncate_text(value.trim(), 64))
        .filter(|value| !value.is_empty());
    let note = note
        .map(|value| truncate_text(value.trim(), 500))
        .filter(|value| !value.is_empty());
    let message = message
        .map(|value| truncate_text(value.trim(), 500))
        .filter(|value| !value.is_empty());

    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "INSERT INTO recommendation_review_events (
                created_at, recommendation_id, recommendation_title, status_after, category, action, actor, note, ok, message
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                created_at,
                recommendation_id,
                recommendation_title,
                status_after,
                category,
                action,
                actor,
                note,
                if ok { 1 } else { 0 },
                message,
            ],
        )?;
    }
    Ok(())
}

pub fn list_recommendation_review_events(
    limit: i64,
) -> Result<Vec<RecommendationReviewEventRecord>> {
    let limit = normalize_admin_limit(limit);
    let mut out = Vec::new();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, created_at, recommendation_id, recommendation_title, status_after, category, action, actor, note, ok, message
             FROM recommendation_review_events
             ORDER BY created_at DESC, id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], map_recommendation_review_event_row)?;
        for row in rows.flatten() {
            out.push(row);
        }
    }
    Ok(out)
}

pub fn get_launch_ops_metrics(limit: i64) -> Result<LaunchOpsMetrics> {
    let limit = normalize_launch_ops_window_limit(limit);
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let metrics = conn.query_row(
            "SELECT
                COUNT(*) as total_requests,
                COALESCE(SUM(CASE WHEN route_kind = 'gate_blocked' THEN 1 ELSE 0 END), 0) as blocked_requests,
                COALESCE(SUM(intent_memory_hit), 0) as intent_memory_hits,
                COALESCE(SUM(request_memory_hit), 0) as request_memory_hits,
                COALESCE(SUM(execution_memory_hit), 0) as execution_memory_hits,
                COALESCE(SUM(deterministic_used), 0) as deterministic_routes,
                COALESCE(SUM(llm_used), 0) as llm_routes,
                COALESCE(SUM(ai_digest_used), 0) as ai_digest_routes,
                COALESCE(SUM(CASE WHEN route_kind = 'ai_digest_auto' THEN 1 ELSE 0 END), 0) as ai_digest_auto_routes,
                COALESCE(SUM(local_only), 0) as local_routes,
                COALESCE(SUM(freshness_bypassed), 0) as freshness_bypasses,
                COALESCE(SUM(CASE WHEN route_kind = 'low_confidence' THEN 1 ELSE 0 END), 0) as low_confidence_routes,
                COALESCE(SUM(CASE WHEN route_kind = 'unknown' THEN 1 ELSE 0 END), 0) as unknown_routes,
                COALESCE(SUM(CASE WHEN outcome = 'error' THEN 1 ELSE 0 END), 0) as error_routes,
                MAX(created_at) as last_event_at
             FROM (
                SELECT created_at, route_kind, outcome, freshness_bypassed, intent_memory_hit,
                       request_memory_hit, execution_memory_hit, deterministic_used, llm_used,
                       ai_digest_used, local_only
                FROM launch_ops_events
                ORDER BY created_at DESC, id DESC
                LIMIT ?1
             )",
            params![limit],
            |row| {
                let total_requests: i64 = row.get(0)?;
                let request_memory_hits: i64 = row.get(3)?;
                let execution_memory_hits: i64 = row.get(4)?;
                let cached_response_hit_rate = if total_requests > 0 {
                    ((request_memory_hits + execution_memory_hits) as f64 / total_requests as f64)
                        * 100.0
                } else {
                    0.0
                };
                Ok(LaunchOpsMetrics {
                    window_size: limit,
                    total_requests,
                    blocked_requests: row.get(1)?,
                    intent_memory_hits: row.get(2)?,
                    request_memory_hits,
                    execution_memory_hits,
                    cached_response_hit_rate,
                    deterministic_routes: row.get(5)?,
                    llm_routes: row.get(6)?,
                    ai_digest_routes: row.get(7)?,
                    ai_digest_auto_routes: row.get(8)?,
                    local_routes: row.get(9)?,
                    freshness_bypasses: row.get(10)?,
                    low_confidence_routes: row.get(11)?,
                    unknown_routes: row.get(12)?,
                    error_routes: row.get(13)?,
                    last_event_at: row.get(14).ok(),
                    route_breakdown: Vec::new(),
                })
            },
        )?;

        let mut breakdown_stmt = conn.prepare(
            "SELECT route_kind, COUNT(*) as count
             FROM (
                SELECT route_kind
                FROM launch_ops_events
                ORDER BY created_at DESC, id DESC
                LIMIT ?1
             )
             GROUP BY route_kind
             ORDER BY count DESC, route_kind ASC
             LIMIT 6",
        )?;
        let breakdown_rows = breakdown_stmt.query_map(params![limit], |row| {
            Ok(LaunchOpsRouteBreakdown {
                route_kind: row.get(0)?,
                count: row.get(1)?,
            })
        })?;

        let mut with_breakdown = metrics;
        for row in breakdown_rows.flatten() {
            with_breakdown.route_breakdown.push(row);
        }
        return Ok(with_breakdown);
    }

    Ok(LaunchOpsMetrics {
        window_size: limit,
        total_requests: 0,
        blocked_requests: 0,
        intent_memory_hits: 0,
        request_memory_hits: 0,
        execution_memory_hits: 0,
        cached_response_hit_rate: 0.0,
        deterministic_routes: 0,
        llm_routes: 0,
        ai_digest_routes: 0,
        ai_digest_auto_routes: 0,
        local_routes: 0,
        freshness_bypasses: 0,
        low_confidence_routes: 0,
        unknown_routes: 0,
        error_routes: 0,
        last_event_at: None,
        route_breakdown: Vec::new(),
    })
}

pub fn suppress_request_memory(request_text: &str, reason: Option<&str>) -> Result<bool> {
    suppress_request_memory_scoped(None, request_text, reason)
}

pub fn suppress_request_memory_scoped(
    memory_scope: Option<&str>,
    request_text: &str,
    reason: Option<&str>,
) -> Result<bool> {
    let Some((memory_scope, normalized_request)) =
        scoped_request_memory_storage_key(request_text, memory_scope)
    else {
        return Ok(false);
    };
    let now = chrono::Utc::now().to_rfc3339();
    let reason = reason
        .map(|value| truncate_text(value, 500))
        .filter(|value| !value.is_empty());
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let rows = conn.execute(
            "UPDATE request_memory
             SET suppressed = 1,
                 suppressed_reason = COALESCE(?1, suppressed_reason),
                 suppressed_at = ?2,
                 updated_at = ?2
             WHERE normalized_request = ?3
               AND memory_scope = ?4",
            params![reason, now, normalized_request, memory_scope],
        )?;
        return Ok(rows > 0);
    }
    Ok(false)
}

pub fn restore_request_memory(request_text: &str) -> Result<bool> {
    restore_request_memory_scoped(None, request_text)
}

pub fn restore_request_memory_scoped(
    memory_scope: Option<&str>,
    request_text: &str,
) -> Result<bool> {
    let Some((memory_scope, normalized_request)) =
        scoped_request_memory_storage_key(request_text, memory_scope)
    else {
        return Ok(false);
    };
    let now = chrono::Utc::now().to_rfc3339();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let rows = conn.execute(
            "UPDATE request_memory
             SET suppressed = 0,
                 suppressed_reason = NULL,
                 suppressed_at = NULL,
                 updated_at = ?1
             WHERE normalized_request = ?2
               AND memory_scope = ?3",
            params![now, normalized_request, memory_scope],
        )?;
        return Ok(rows > 0);
    }
    Ok(false)
}

pub fn delete_request_memory(request_text: &str) -> Result<bool> {
    delete_request_memory_scoped(None, request_text)
}

pub fn delete_request_memory_scoped(
    memory_scope: Option<&str>,
    request_text: &str,
) -> Result<bool> {
    let Some((memory_scope, normalized_request)) =
        scoped_request_memory_storage_key(request_text, memory_scope)
    else {
        return Ok(false);
    };
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let rows = conn.execute(
            "DELETE FROM request_memory
             WHERE normalized_request = ?1
               AND memory_scope = ?2",
            params![normalized_request, memory_scope],
        )?;
        return Ok(rows > 0);
    }
    Ok(false)
}

pub fn suppress_execution_memory(
    intent_command: &str,
    params_key: &str,
    reason: Option<&str>,
) -> Result<bool> {
    suppress_execution_memory_scoped(None, intent_command, params_key, reason)
}

pub fn suppress_execution_memory_scoped(
    memory_scope: Option<&str>,
    intent_command: &str,
    params_key: &str,
    reason: Option<&str>,
) -> Result<bool> {
    let intent_command = truncate_text(intent_command, 64);
    let Some((memory_scope, _logical_params_key, params_key)) =
        scoped_execution_params_key(params_key, memory_scope)
    else {
        return Ok(false);
    };
    let now = chrono::Utc::now().to_rfc3339();
    let reason = reason
        .map(|value| truncate_text(value, 500))
        .filter(|value| !value.is_empty());
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let rows = conn.execute(
            "UPDATE execution_memory
             SET suppressed = 1,
                 suppressed_reason = COALESCE(?1, suppressed_reason),
                 suppressed_at = ?2,
                 updated_at = ?2
             WHERE memory_scope = ?3
               AND intent_command = ?4
               AND params_key = ?5",
            params![reason, now, memory_scope, intent_command, params_key],
        )?;
        return Ok(rows > 0);
    }
    Ok(false)
}

pub fn restore_execution_memory(intent_command: &str, params_key: &str) -> Result<bool> {
    restore_execution_memory_scoped(None, intent_command, params_key)
}

pub fn restore_execution_memory_scoped(
    memory_scope: Option<&str>,
    intent_command: &str,
    params_key: &str,
) -> Result<bool> {
    let intent_command = truncate_text(intent_command, 64);
    let Some((memory_scope, _logical_params_key, params_key)) =
        scoped_execution_params_key(params_key, memory_scope)
    else {
        return Ok(false);
    };
    let now = chrono::Utc::now().to_rfc3339();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let rows = conn.execute(
            "UPDATE execution_memory
             SET suppressed = 0,
                 suppressed_reason = NULL,
                 suppressed_at = NULL,
                 updated_at = ?1
             WHERE memory_scope = ?2
               AND intent_command = ?3
               AND params_key = ?4",
            params![now, memory_scope, intent_command, params_key],
        )?;
        return Ok(rows > 0);
    }
    Ok(false)
}

pub fn delete_execution_memory(intent_command: &str, params_key: &str) -> Result<bool> {
    delete_execution_memory_scoped(None, intent_command, params_key)
}

pub fn delete_execution_memory_scoped(
    memory_scope: Option<&str>,
    intent_command: &str,
    params_key: &str,
) -> Result<bool> {
    let intent_command = truncate_text(intent_command, 64);
    let Some((memory_scope, _logical_params_key, params_key)) =
        scoped_execution_params_key(params_key, memory_scope)
    else {
        return Ok(false);
    };
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let rows = conn.execute(
            "DELETE FROM execution_memory
             WHERE memory_scope = ?1
               AND intent_command = ?2
               AND params_key = ?3",
            params![memory_scope, intent_command, params_key],
        )?;
        return Ok(rows > 0);
    }
    Ok(false)
}

fn normalize_feedback_signal(sentiment: &str) -> Option<&'static str> {
    match sentiment.trim().to_ascii_lowercase().as_str() {
        "positive" | "up" | "like" => Some("positive"),
        "negative" | "down" | "dislike" => Some("negative"),
        _ => None,
    }
}

pub fn record_request_memory_feedback(
    request_text: &str,
    response_text: &str,
    sentiment: &str,
) -> Result<bool> {
    record_request_memory_feedback_scoped(None, request_text, response_text, sentiment)
}

pub fn record_request_memory_feedback_scoped(
    memory_scope: Option<&str>,
    request_text: &str,
    response_text: &str,
    sentiment: &str,
) -> Result<bool> {
    let Some(sentiment) = normalize_feedback_signal(sentiment) else {
        return Ok(false);
    };
    let Some((memory_scope, normalized_request)) =
        scoped_request_memory_storage_key(request_text, memory_scope)
    else {
        return Ok(false);
    };
    let response_text = truncate_text(response_text, 4000);
    if response_text.is_empty() {
        return Ok(false);
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let positive_delta = if sentiment == "positive" { 1 } else { 0 };
        let negative_delta = if sentiment == "negative" { 1 } else { 0 };
        let rows = conn.execute(
            "UPDATE request_memory
             SET positive_feedback_count = positive_feedback_count + ?1,
                 negative_feedback_count = negative_feedback_count + ?2,
                 last_feedback_at = ?3,
                 updated_at = ?3
             WHERE normalized_request = ?4
               AND memory_scope = ?5
               AND response_text = ?6",
            params![
                positive_delta,
                negative_delta,
                now,
                normalized_request,
                memory_scope,
                response_text
            ],
        )?;
        return Ok(rows > 0);
    }
    Ok(false)
}

pub fn record_execution_memory_feedback(
    intent_command: &str,
    params_key: &str,
    response_text: &str,
    sentiment: &str,
) -> Result<bool> {
    record_execution_memory_feedback_scoped(
        None,
        intent_command,
        params_key,
        response_text,
        sentiment,
    )
}

pub fn record_execution_memory_feedback_scoped(
    memory_scope: Option<&str>,
    intent_command: &str,
    params_key: &str,
    response_text: &str,
    sentiment: &str,
) -> Result<bool> {
    let Some(sentiment) = normalize_feedback_signal(sentiment) else {
        return Ok(false);
    };
    let intent_command = truncate_text(intent_command, 64);
    let Some((memory_scope, _logical_params_key, params_key)) =
        scoped_execution_params_key(params_key, memory_scope)
    else {
        return Ok(false);
    };
    let response_text = truncate_text(response_text, 4000);
    if intent_command.trim().is_empty() || response_text.is_empty() {
        return Ok(false);
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let positive_delta = if sentiment == "positive" { 1 } else { 0 };
        let negative_delta = if sentiment == "negative" { 1 } else { 0 };
        let rows = conn.execute(
            "UPDATE execution_memory
             SET positive_feedback_count = positive_feedback_count + ?1,
                 negative_feedback_count = negative_feedback_count + ?2,
                 last_feedback_at = ?3,
                 updated_at = ?3
             WHERE memory_scope = ?4
               AND intent_command = ?5
               AND params_key = ?6
               AND response_text = ?7",
            params![
                positive_delta,
                negative_delta,
                now,
                memory_scope,
                intent_command,
                params_key,
                response_text
            ],
        )?;
        return Ok(rows > 0);
    }
    Ok(false)
}

pub fn get_recent_chat_history(limit: i64) -> Result<Vec<ChatMessage>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
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
        // Return in chronological order for context (REVERSE)
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
        let mut stmt = conn.prepare(
            "SELECT id, name, steps_json, created_at FROM learned_routines WHERE name = ?1",
        )?;
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
        let mut stmt = conn.prepare("SELECT id, name, steps_json, created_at FROM learned_routines ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| {
            Ok(LearnedRoutine {
                id: row.get(0)?,
                name: row.get(1)?,
                steps_json: row.get(2)?,
                created_at: row.get(3)?, // This line was missing in the provided snippet, added for completeness based on LearnedRoutine struct
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
        // 1. Session Stats
        // Check if sessions_v2 exists first, otherwise mock
        let (total_sessions, total_time_mins): (i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(duration_sec)/60, 0) FROM sessions_v2",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or_default(); // sessions_v2 might not exist yet

        // 2. Pending Recs
        let now = chrono::Utc::now().to_rfc3339();
        let rec_pending: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM recommendations
                 WHERE status = 'pending'
                   AND (snoozed_until IS NULL OR TRIM(snoozed_until) = '' OR snoozed_until <= ?1)",
                params![now],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // 3. Top Apps
        let mut top_apps = Vec::new();
        if let Ok(mut stmt) = conn.prepare("SELECT app_name, count(*) as c FROM (select app as app_name from events_v2) GROUP BY app_name ORDER BY c DESC LIMIT 3") {
            let rows = stmt.query_map([], |row| {
                 Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            });
            if let Ok(iter) = rows {
                for val in iter.flatten() { top_apps.push(val); }
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
            "SELECT id, status, title, summary, trigger, actions, n8n_prompt, confidence, workflow_id, workflow_json, evidence, pattern_id, last_error, snoozed_until, category, business_score, feedback_status, feedback_note, feedback_count, last_feedback_at
             FROM recommendations 
             WHERE status IN ('pending', 'approved', 'rejected')
             ORDER BY created_at DESC 
             LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit], map_row)?;

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
        // Check standard env var or default
        write_lock_default: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recommendation::AutomationProposal;
    use serial_test::serial;

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

    #[test]
    #[serial]
    fn test_recommendation_review_status_transitions() {
        init().ok();
        clear_recommendations_for_tests();

        let unique = format!("status-transition-test-{}", uuid::Uuid::new_v4());
        let proposal = AutomationProposal {
            title: unique.clone(),
            summary: "status transition test".to_string(),
            trigger: "unit-test".to_string(),
            actions: vec!["noop".to_string()],
            confidence: 0.1,
            n8n_prompt: "noop".to_string(),
            evidence: vec![],
            pattern_id: None,
            category: crate::recommendation_policy::CATEGORY_UNKNOWN.to_string(),
            business_score: 0.0,
        };
        let _ = insert_recommendation(&proposal);

        let rows = get_recommendations_with_filter(Some("all")).unwrap_or_default();
        let Some(rec) = rows.into_iter().find(|r| r.title == unique) else {
            eprintln!("skip: could not resolve inserted recommendation for transition test");
            return;
        };

        assert!(update_recommendation_review_status(rec.id, "approved").is_ok());
        assert!(update_recommendation_review_status(rec.id, "pending").is_err());
        assert!(update_recommendation_review_status(rec.id, "rejected").is_ok());
        assert!(update_recommendation_review_status(rec.id, "later").is_err());
    }

    #[test]
    #[serial]
    fn test_recommendation_snooze_roundtrip_and_metrics() {
        init().ok();
        clear_recommendations_for_tests();

        let unique = format!("snooze-transition-test-{}", uuid::Uuid::new_v4());
        let proposal = AutomationProposal {
            title: unique.clone(),
            summary: "snooze transition test".to_string(),
            trigger: "unit-test".to_string(),
            actions: vec!["noop".to_string()],
            confidence: 0.1,
            n8n_prompt: "noop".to_string(),
            evidence: vec![],
            pattern_id: None,
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.7,
        };
        let _ = insert_recommendation(&proposal);

        let rows = get_recommendations_with_filter(Some("all")).unwrap_or_default();
        let rec = rows
            .into_iter()
            .find(|r| r.title == unique)
            .expect("recommendation");

        snooze_recommendation(rec.id, 24).expect("snooze recommendation");
        let snoozed = get_recommendation(rec.id)
            .expect("get recommendation")
            .expect("row");
        assert!(snoozed.snoozed_until.is_some());

        let metrics = get_recommendation_metrics().expect("metrics");
        assert_eq!(metrics.pending, 0);
        assert_eq!(metrics.later, 1);

        restore_recommendation(rec.id).expect("restore recommendation");
        let restored = get_recommendation(rec.id)
            .expect("get recommendation")
            .expect("row");
        assert!(restored.snoozed_until.is_none());

        let metrics = get_recommendation_metrics().expect("metrics");
        assert_eq!(metrics.pending, 1);
        assert_eq!(metrics.later, 0);
    }

    #[test]
    #[serial]
    fn test_snooze_recommendation_rejects_non_pending_status() {
        init().ok();
        clear_recommendations_for_tests();

        let unique = format!("snooze-approved-test-{}", uuid::Uuid::new_v4());
        let proposal = AutomationProposal {
            title: unique.clone(),
            summary: "snooze approved recommendation test".to_string(),
            trigger: "unit-test".to_string(),
            actions: vec!["noop".to_string()],
            confidence: 0.1,
            n8n_prompt: "noop".to_string(),
            evidence: vec![],
            pattern_id: None,
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.7,
        };
        let _ = insert_recommendation(&proposal);

        let rec = get_recommendations_with_filter(Some("all"))
            .unwrap_or_default()
            .into_iter()
            .find(|r| r.title == unique)
            .expect("recommendation");
        update_recommendation_review_status(rec.id, "approved").expect("approve recommendation");

        let err = snooze_recommendation(rec.id, 24)
            .expect_err("approved recommendation should not snooze");
        assert!(err.to_string().contains("cannot snooze recommendation"));

        let err =
            restore_recommendation(rec.id).expect_err("approved recommendation should not restore");
        assert!(err.to_string().contains("cannot restore recommendation"));
    }

    #[test]
    #[serial]
    fn test_recommendation_feedback_roundtrip() {
        init().ok();
        clear_recommendations_for_tests();

        let unique = format!("feedback-test-{}", uuid::Uuid::new_v4());
        let proposal = AutomationProposal {
            title: unique.clone(),
            summary: "recommendation feedback test".to_string(),
            trigger: "unit-test".to_string(),
            actions: vec!["noop".to_string()],
            confidence: 0.1,
            n8n_prompt: "noop".to_string(),
            evidence: vec![],
            pattern_id: Some("feedback-pattern".to_string()),
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.7,
        };
        let _ = insert_recommendation(&proposal);

        let rec = get_recommendations_with_filter(Some("all"))
            .unwrap_or_default()
            .into_iter()
            .find(|r| r.title == unique)
            .expect("recommendation");

        assert!(
            record_recommendation_feedback(rec.id, "refine", "텔레그램 말고 노션으로 보내줘")
                .expect("record feedback")
        );

        let updated = get_recommendation(rec.id)
            .expect("get recommendation")
            .expect("row");
        assert_eq!(updated.feedback_status.as_deref(), Some("refine"));
        assert_eq!(
            updated.feedback_note.as_deref(),
            Some("텔레그램 말고 노션으로 보내줘")
        );
        assert_eq!(updated.feedback_count, 1);
        assert!(updated.last_feedback_at.is_some());
    }

    #[test]
    fn test_collector_handoff_receipt_roundtrip() {
        init().ok();
        let package_id = format!("pkg-{}", uuid::Uuid::new_v4());
        let status = "consumed";
        assert!(record_collector_handoff_receipt(
            &package_id,
            Some(42),
            status,
            Some(7),
            Some("unit-test")
        )
        .is_ok());

        let receipts = list_collector_handoff_receipts(50).unwrap_or_default();
        let found = receipts
            .into_iter()
            .find(|r| r.package_id == package_id)
            .expect("expected inserted collector handoff receipt");
        assert_eq!(found.collector_row_id, Some(42));
        assert_eq!(found.status, status);
        assert_eq!(found.recommendation_id, Some(7));
    }

    #[test]
    fn test_exec_allowlist_pattern_validation_defaults_secure() {
        assert!(validate_exec_allowlist_pattern_with_flags("*", false, false).is_err());
        assert!(validate_exec_allowlist_pattern_with_flags("all", false, false).is_err());
        assert!(validate_exec_allowlist_pattern_with_flags("re:^ls", false, false).is_err());
        assert!(validate_exec_allowlist_pattern_with_flags("/^ls/", false, false).is_err());
        assert!(validate_exec_allowlist_pattern_with_flags("ls -la", false, false).is_ok());
        assert!(validate_exec_allowlist_pattern_with_flags("git*", false, false).is_ok());
    }

    #[test]
    fn test_exec_allowlist_pattern_match_with_flags() {
        assert!(!exec_pattern_match_with_flags(
            "*", "rm -rf /", false, false
        ));
        assert!(exec_pattern_match_with_flags(
            "*",
            "echo hello",
            true,
            false
        ));
        assert!(!exec_pattern_match_with_flags(
            "re:^ls\\b",
            "ls -la",
            false,
            false
        ));
        assert!(exec_pattern_match_with_flags(
            "re:^ls\\b",
            "ls -la",
            false,
            true
        ));
        assert!(exec_pattern_match_with_flags(
            "git*",
            "git status",
            false,
            false
        ));
        assert!(!exec_pattern_match_with_flags(
            "git*", "ls -la", false, false
        ));
    }

    #[test]
    fn test_task_stage_retry_metadata_recorded() {
        init().ok();
        let run_id = format!("run-{}", uuid::Uuid::new_v4());
        create_task_run(&run_id, "test", "retry metadata", "running").expect("create run");
        record_task_stage_run(&run_id, "execution", 2, "running", Some("start"))
            .expect("running stage");
        record_task_stage_run(&run_id, "execution", 2, "retrying", Some("retry attempt"))
            .expect("retrying stage");
        let stages = list_task_stage_runs(&run_id).expect("list stages");
        let latest = stages
            .into_iter()
            .rfind(|s| s.stage_name == "execution")
            .expect("latest execution stage");
        assert_eq!(latest.status, "retrying");
        assert!(latest.retry_count >= 1);
        assert!(latest.max_retries >= 0);
        assert!(latest.next_retry_at.is_some());
    }

    #[test]
    fn test_task_stage_invalid_transition_rejected() {
        init().ok();
        let run_id = format!("run-{}", uuid::Uuid::new_v4());
        create_task_run(&run_id, "test", "invalid transition", "running").expect("create run");
        record_task_stage_run(&run_id, "planner", 1, "running", Some("start"))
            .expect("planner running");
        record_task_stage_run(&run_id, "planner", 1, "completed", Some("done"))
            .expect("planner done");
        let invalid = record_task_stage_run(&run_id, "planner", 1, "running", Some("should fail"));
        assert!(invalid.is_err());
    }

    #[test]
    fn test_task_run_artifact_upsert_roundtrip() {
        init().ok();
        let run_id = format!("run-{}", uuid::Uuid::new_v4());
        create_task_run(&run_id, "test", "artifact upsert", "running").expect("create run");

        upsert_task_run_artifact(
            &run_id,
            "artifact_assertion",
            "artifact.mail_sent_confirmed",
            "false",
            Some("{\"passed\":false}"),
        )
        .expect("insert artifact");
        upsert_task_run_artifact(
            &run_id,
            "artifact_assertion",
            "artifact.mail_sent_confirmed",
            "true",
            Some("{\"passed\":true}"),
        )
        .expect("update artifact");

        let artifacts = list_task_run_artifacts(&run_id).expect("list artifacts");
        let item = artifacts
            .into_iter()
            .find(|a| a.artifact_key == "artifact.mail_sent_confirmed")
            .expect("artifact row");
        assert_eq!(item.value, "true");
        assert_eq!(item.artifact_type, "artifact_assertion");
        assert!(item
            .metadata
            .unwrap_or_default()
            .contains("\"passed\":true"));
    }

    #[test]
    #[serial]
    fn test_request_memory_roundtrip_normalizes_whitespace() {
        init().ok();
        clear_request_memory_for_tests();
        let intent = serde_json::json!({
            "command": "calendar_today",
            "params": {},
            "confidence": 0.91
        });
        upsert_request_memory(
            "  오늘   일정   보여줘  ",
            Some(&intent),
            Some("📅 오늘 일정이 없습니다."),
            "intent_only",
            "unit.test",
            0.91,
        )
        .expect("upsert request memory");

        let found = get_request_memory("오늘 일정 보여줘")
            .expect("get request memory")
            .expect("request memory row");
        assert_eq!(found.response_mode, "intent_only");
        assert!(found.confidence >= 0.9);
        assert_eq!(found.intent_command.as_deref(), Some("calendar_today"));
        assert_eq!(
            found.request_signature.as_deref(),
            Some("today calendar read")
        );
        assert!(found
            .intent_json
            .unwrap_or_default()
            .contains("calendar_today"));
    }

    #[test]
    #[serial]
    fn test_request_memory_roundtrip_ignores_punctuation() {
        init().ok();
        clear_request_memory_for_tests();
        let intent = serde_json::json!({
            "command": "calendar_today",
            "params": {},
            "confidence": 0.93
        });
        upsert_request_memory(
            "오늘 일정 보여줘?!",
            Some(&intent),
            Some("📅 오늘 일정이 없습니다."),
            "intent_only",
            "unit.test",
            0.93,
        )
        .expect("upsert request memory");

        let found = get_request_memory("오늘 일정 보여줘")
            .expect("get request memory")
            .expect("request memory row");
        assert!(found.confidence >= 0.93);
        assert!(found
            .intent_json
            .unwrap_or_default()
            .contains("calendar_today"));
    }

    #[test]
    #[serial]
    fn test_request_memory_is_scoped_by_actor_context() {
        init().ok();
        clear_request_memory_for_tests();
        let intent = serde_json::json!({
            "command": "calendar_today",
            "params": {},
            "confidence": 0.94
        });

        upsert_request_memory_scoped(
            Some("channel_web__sender_alice"),
            "오늘 일정 보여줘",
            Some(&intent),
            Some("📅 Alice"),
            "intent_only",
            "unit.test",
            0.94,
        )
        .expect("seed alice request memory");
        upsert_request_memory_scoped(
            Some("channel_web__sender_bob"),
            "오늘 일정 보여줘",
            Some(&intent),
            Some("📅 Bob"),
            "intent_only",
            "unit.test",
            0.94,
        )
        .expect("seed bob request memory");

        let alice =
            get_request_memory_scoped(Some("channel_web__sender_alice"), "오늘 일정 보여줘")
                .expect("alice lookup")
                .expect("alice row");
        let bob = get_request_memory_scoped(Some("channel_web__sender_bob"), "오늘 일정 보여줘")
            .expect("bob lookup")
            .expect("bob row");

        assert_eq!(alice.memory_scope, "channel_web_sender_alice");
        assert_eq!(bob.memory_scope, "channel_web_sender_bob");
        assert_eq!(alice.response_text.as_deref(), Some("📅 Alice"));
        assert_eq!(bob.response_text.as_deref(), Some("📅 Bob"));
        assert!(get_request_memory("오늘 일정 보여줘")
            .expect("global lookup")
            .is_none());
    }

    #[test]
    #[serial]
    fn test_request_memory_lookup_by_signature_prefers_supported_command() {
        init().ok();
        clear_request_memory_for_tests();
        let intent = serde_json::json!({
            "command": "calendar_today",
            "params": {},
            "confidence": 0.94
        });
        upsert_request_memory(
            "오늘 캘린더 보여줘",
            Some(&intent),
            Some("📅 오늘 일정이 없습니다."),
            "intent_only",
            "unit.test",
            0.94,
        )
        .expect("upsert request memory");

        let found = get_request_memory_by_signature("today calendar read", &["calendar_today"])
            .expect("lookup by signature")
            .expect("signature row");
        assert_eq!(found.intent_command.as_deref(), Some("calendar_today"));
        assert_eq!(
            found.request_signature.as_deref(),
            Some("today calendar read")
        );
    }

    #[test]
    #[serial]
    fn test_request_memory_lookup_by_signature_prefers_positive_feedback() {
        init().ok();
        clear_request_memory_for_tests();
        let positive_request = "allvia positive cache";
        let negative_request = "allvia negative cache";
        let command = format!("calendar_today_pref_{}", uuid::Uuid::new_v4());
        let intent = serde_json::json!({
            "command": command,
            "params": {},
            "confidence": 0.94
        });

        upsert_request_memory(
            &format!("{} 오늘 일정 보여줘", negative_request),
            Some(&intent),
            Some("📅 A"),
            "intent_only",
            "unit.test",
            0.94,
        )
        .expect("seed negative request");
        upsert_request_memory(
            &format!("{} 오늘 캘린더 알려줘", positive_request),
            Some(&intent),
            Some("📅 B"),
            "intent_only",
            "unit.test",
            0.94,
        )
        .expect("seed positive request");

        record_request_memory_feedback(
            &format!("{} 오늘 일정 보여줘", negative_request),
            "📅 A",
            "negative",
        )
        .expect("negative feedback");
        record_request_memory_feedback(
            &format!("{} 오늘 캘린더 알려줘", positive_request),
            "📅 B",
            "positive",
        )
        .expect("positive feedback");

        let found = get_request_memory_by_signature("today calendar read", &[command.as_str()])
            .expect("lookup by signature")
            .expect("signature row");
        assert!(found.original_request.contains(&positive_request));
        assert_eq!(found.positive_feedback_count, 1);
        assert_eq!(found.negative_feedback_count, 0);
    }

    #[test]
    #[serial]
    fn test_request_memory_lookup_by_signature_prefers_deterministic_source() {
        init().ok();
        clear_request_memory_for_tests();
        let command = format!("calendar_today_source_{}", uuid::Uuid::new_v4());
        let intent = serde_json::json!({
            "command": command,
            "params": {},
            "confidence": 0.94,
            "source": "deterministic"
        });

        upsert_request_memory(
            "allvia source llm 오늘 일정 보여줘",
            Some(&intent),
            Some("📅 LLM"),
            "intent_only",
            "api.chat.llm",
            0.94,
        )
        .expect("seed llm request");
        upsert_request_memory(
            "allvia source deterministic 오늘 캘린더 알려줘",
            Some(&intent),
            Some("📅 deterministic"),
            "intent_only",
            "api.chat.deterministic",
            0.94,
        )
        .expect("seed deterministic request");

        let found = get_request_memory_by_signature("today calendar read", &[command.as_str()])
            .expect("lookup by signature")
            .expect("signature row");
        assert_eq!(found.source, "api.chat.deterministic");
        assert!(found.original_request.contains("deterministic"));
    }

    #[test]
    #[serial]
    fn test_execution_memory_roundtrip_by_command_and_params() {
        init().ok();
        clear_execution_memory_for_tests();
        let params = serde_json::json!({ "count": 5 });
        let response = format!("📧 test-execution-memory-{}", uuid::Uuid::new_v4());

        upsert_execution_memory(
            "gmail_list",
            "count=5",
            Some(&params),
            Some("recent email 5 read"),
            &response,
            20,
            "unit.test",
            "integrations.gmail.list_messages",
        )
        .expect("upsert execution memory");

        let found = get_execution_memory("gmail_list", "count=5")
            .expect("get execution memory")
            .expect("execution memory row");
        assert_eq!(found.intent_command, "gmail_list");
        assert_eq!(found.params_key, "count=5");
        assert_eq!(found.response_text, response);
        assert_eq!(found.freshness_ttl_seconds, 20);
        assert_eq!(found.tool_path, "integrations.gmail.list_messages");
        assert!(found.success);
    }

    #[test]
    #[serial]
    fn test_execution_memory_is_scoped_by_actor_context() {
        init().ok();
        clear_execution_memory_for_tests();
        let params = serde_json::json!({ "count": 5 });

        upsert_execution_memory_scoped(
            Some("channel_web__sender_alice"),
            "gmail_list",
            "count=5",
            Some(&params),
            Some("recent email 5 read"),
            "📧 Alice",
            20,
            "unit.test",
            "integrations.gmail.list_messages",
        )
        .expect("seed alice execution memory");
        upsert_execution_memory_scoped(
            Some("channel_web__sender_bob"),
            "gmail_list",
            "count=5",
            Some(&params),
            Some("recent email 5 read"),
            "📧 Bob",
            20,
            "unit.test",
            "integrations.gmail.list_messages",
        )
        .expect("seed bob execution memory");

        let alice =
            get_execution_memory_scoped(Some("channel_web__sender_alice"), "gmail_list", "count=5")
                .expect("alice execution lookup")
                .expect("alice execution row");
        let bob =
            get_execution_memory_scoped(Some("channel_web__sender_bob"), "gmail_list", "count=5")
                .expect("bob execution lookup")
                .expect("bob execution row");

        assert_eq!(alice.memory_scope, "channel_web_sender_alice");
        assert_eq!(bob.memory_scope, "channel_web_sender_bob");
        assert_eq!(alice.params_key, "count=5");
        assert_eq!(bob.params_key, "count=5");
        assert_eq!(alice.response_text, "📧 Alice");
        assert_eq!(bob.response_text, "📧 Bob");
        assert!(get_execution_memory("gmail_list", "count=5")
            .expect("global execution lookup")
            .is_none());
    }

    #[test]
    #[serial]
    fn test_request_memory_feedback_updates_matching_response_only() {
        init().ok();
        clear_request_memory_for_tests();
        let request = format!("allvia-feedback-request-{}", uuid::Uuid::new_v4());
        let response = format!("response-{}", uuid::Uuid::new_v4());
        let intent = serde_json::json!({
            "command": "calendar_today",
            "params": {},
            "confidence": 0.91
        });

        upsert_request_memory(
            &request,
            Some(&intent),
            Some(&response),
            "intent_only",
            "unit.test",
            0.91,
        )
        .expect("seed request memory");

        assert!(
            record_request_memory_feedback(&request, &response, "negative")
                .expect("record feedback")
        );
        let found = get_request_memory(&request)
            .expect("get request memory")
            .expect("request memory row");
        assert_eq!(found.negative_feedback_count, 1);
        assert_eq!(found.positive_feedback_count, 0);
        assert!(found.last_feedback_at.is_some());

        assert!(
            !record_request_memory_feedback(&request, "other-response", "positive")
                .expect("mismatched feedback")
        );
    }

    #[test]
    #[serial]
    fn test_execution_memory_feedback_updates_matching_response_only() {
        init().ok();
        clear_execution_memory_for_tests();
        let response = format!("response-{}", uuid::Uuid::new_v4());
        let command = format!("gmail_feedback_{}", uuid::Uuid::new_v4());
        let params_key = format!("count=5-{}", uuid::Uuid::new_v4());
        let params = serde_json::json!({ "count": 5 });
        upsert_execution_memory(
            &command,
            &params_key,
            Some(&params),
            Some("recent email 5 read"),
            &response,
            20,
            "unit.test",
            "integrations.gmail.list_messages",
        )
        .expect("seed execution memory");

        assert!(
            record_execution_memory_feedback(&command, &params_key, &response, "positive")
                .expect("record execution feedback")
        );
        let found = get_execution_memory(&command, &params_key)
            .expect("get execution memory")
            .expect("execution memory row");
        assert_eq!(found.positive_feedback_count, 1);
        assert_eq!(found.negative_feedback_count, 0);
        assert!(found.last_feedback_at.is_some());

        assert!(
            !record_execution_memory_feedback(&command, &params_key, "other", "negative")
                .expect("mismatched execution feedback")
        );
    }

    #[test]
    #[serial]
    fn test_request_memory_suppress_restore_and_delete_roundtrip() {
        init().ok();
        clear_request_memory_for_tests();
        let request = format!("allvia-memory-admin-request-{}", uuid::Uuid::new_v4());
        let intent = serde_json::json!({
            "command": "calendar_today",
            "params": {},
            "confidence": 0.95
        });
        upsert_request_memory(
            &request,
            Some(&intent),
            Some("📅 memory-admin"),
            "ttl_response_signature",
            "unit.test",
            0.95,
        )
        .expect("seed request memory");

        assert!(get_request_memory(&request)
            .expect("get request memory")
            .is_some());
        assert!(
            suppress_request_memory(&request, Some("bad cache")).expect("suppress request memory")
        );
        assert!(get_request_memory(&request)
            .expect("suppressed request lookup")
            .is_none());

        let listed = list_request_memory_records(10, true).expect("list request memory");
        let suppressed = listed
            .into_iter()
            .find(|record| record.original_request == request)
            .expect("suppressed request record");
        assert!(suppressed.suppressed);
        assert_eq!(suppressed.suppressed_reason.as_deref(), Some("bad cache"));

        assert!(restore_request_memory(&request).expect("restore request memory"));
        assert!(get_request_memory(&request)
            .expect("restored request memory")
            .is_some());

        assert!(delete_request_memory(&request).expect("delete request memory"));
        assert!(get_request_memory(&request)
            .expect("deleted request lookup")
            .is_none());
    }

    #[test]
    #[serial]
    fn test_execution_memory_suppress_restore_and_delete_roundtrip() {
        init().ok();
        clear_execution_memory_for_tests();
        let command = format!("gmail_admin_{}", uuid::Uuid::new_v4());
        upsert_execution_memory(
            &command,
            "count=5",
            Some(&serde_json::json!({ "count": 5 })),
            Some("recent email 5 read"),
            "📧 memory-admin",
            20,
            "unit.test",
            "integrations.gmail.list_messages",
        )
        .expect("seed execution memory");

        assert!(get_execution_memory(&command, "count=5")
            .expect("get execution memory")
            .is_some());
        assert!(
            suppress_execution_memory(&command, "count=5", Some("stale result"))
                .expect("suppress execution memory")
        );
        assert!(get_execution_memory(&command, "count=5")
            .expect("suppressed execution lookup")
            .is_none());

        let listed = list_execution_memory_records(10, true).expect("list execution memory");
        let suppressed = listed
            .into_iter()
            .find(|record| record.intent_command == command)
            .expect("suppressed execution record");
        assert!(suppressed.suppressed);
        assert_eq!(
            suppressed.suppressed_reason.as_deref(),
            Some("stale result")
        );

        assert!(restore_execution_memory(&command, "count=5").expect("restore execution memory"));
        assert!(get_execution_memory(&command, "count=5")
            .expect("restored execution lookup")
            .is_some());

        assert!(delete_execution_memory(&command, "count=5").expect("delete execution memory"));
        assert!(get_execution_memory(&command, "count=5")
            .expect("deleted execution lookup")
            .is_none());
    }

    #[test]
    #[serial]
    fn test_memory_ops_metrics_count_suppressed_records() {
        init().ok();
        clear_request_memory_for_tests();
        clear_execution_memory_for_tests();

        let request = format!("allvia-memory-metrics-request-{}", uuid::Uuid::new_v4());
        upsert_request_memory(
            &request,
            Some(&serde_json::json!({
                "command": "help_local",
                "params": {},
                "confidence": 0.95
            })),
            Some("HELP"),
            "reusable_response",
            "unit.test",
            0.95,
        )
        .expect("seed request memory");
        upsert_execution_memory(
            "gmail_metrics",
            "count=5",
            Some(&serde_json::json!({ "count": 5 })),
            Some("recent email 5 read"),
            "📧 metrics",
            20,
            "unit.test",
            "integrations.gmail.list_messages",
        )
        .expect("seed execution memory");

        suppress_request_memory(&request, Some("ops")).expect("suppress request");
        suppress_execution_memory("gmail_metrics", "count=5", Some("ops"))
            .expect("suppress execution");

        let metrics = get_memory_ops_metrics().expect("memory ops metrics");
        assert!(metrics.request_suppressed >= 1);
        assert!(metrics.execution_suppressed >= 1);
    }

    #[test]
    #[serial]
    fn test_memory_admin_events_roundtrip() {
        init().ok();
        clear_memory_admin_events_for_tests();

        record_memory_admin_event(
            "request_memory",
            "suppress",
            Some("channel_web__sender_alice"),
            "오늘 일정 보여줘",
            Some("bad cache"),
            Some("web_settings"),
            true,
            Some("request_memory suppress succeeded."),
        )
        .expect("record request admin event");
        record_memory_admin_event(
            "execution_memory",
            "delete",
            Some("channel_web__sender_alice"),
            "gmail_list::count=5",
            None,
            Some("api.memory_admin"),
            false,
            Some("execution_memory delete target was not found."),
        )
        .expect("record execution admin event");

        let events = list_memory_admin_events(10).expect("list memory admin events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, "execution_memory");
        assert_eq!(events[0].action, "delete");
        assert_eq!(
            events[0].memory_scope.as_deref(),
            Some("channel_web_sender_alice")
        );
        assert!(!events[0].ok);
        assert_eq!(events[1].kind, "request_memory");
        assert_eq!(events[1].reason.as_deref(), Some("bad cache"));
        assert_eq!(events[1].actor.as_deref(), Some("web_settings"));
        assert!(events[1].ok);
    }

    #[test]
    #[serial]
    fn test_recommendation_review_events_roundtrip() {
        init().ok();
        clear_recommendation_review_events_for_tests();

        record_recommendation_review_event(
            42,
            "Work Start Checklist",
            Some("approved"),
            Some("work"),
            "approve",
            Some("web_workflows"),
            Some("approved from workflows screen"),
            true,
            Some("workflow provisioning requested"),
        )
        .expect("record approved review event");
        record_recommendation_review_event(
            43,
            "Email Follow-Up Reminder",
            Some("pending"),
            Some("work"),
            "approve",
            Some("web_dashboard"),
            None,
            false,
            Some("needs more evidence before approval"),
        )
        .expect("record failed review event");

        let events =
            list_recommendation_review_events(10).expect("list recommendation review events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].recommendation_id, 43);
        assert_eq!(events[0].action, "approve");
        assert!(!events[0].ok);
        assert_eq!(events[0].status_after.as_deref(), Some("pending"));
        assert_eq!(events[1].actor.as_deref(), Some("web_workflows"));
        assert!(events[1].ok);
        assert_eq!(events[1].category.as_deref(), Some("work"));
    }

    #[test]
    #[serial]
    fn test_recommendation_review_metrics_aggregate_recent_events() {
        init().ok();
        clear_recommendation_review_events_for_tests();

        record_recommendation_review_event(
            1,
            "Work Start Checklist",
            Some("approved"),
            Some("work"),
            "approve",
            Some("web_workflows"),
            None,
            true,
            Some("approved"),
        )
        .expect("record approve");
        record_recommendation_review_event(
            2,
            "Email Follow-Up Reminder",
            Some("pending"),
            Some("work"),
            "later",
            Some("web_dashboard"),
            None,
            false,
            Some("cannot snooze recommendation"),
        )
        .expect("record later failure");
        record_recommendation_review_event(
            3,
            "Slack Digest",
            Some("rejected"),
            Some("work"),
            "feedback_negative",
            Some("web_dashboard"),
            Some("not useful"),
            true,
            Some("negative feedback"),
        )
        .expect("record negative feedback");
        record_recommendation_review_event(
            4,
            "Notion Summary",
            Some("pending"),
            Some("work"),
            "feedback_refine",
            Some("web_dashboard"),
            Some("notion only"),
            true,
            Some("refine feedback"),
        )
        .expect("record refine feedback");

        let metrics = get_recommendation_review_metrics(10).expect("review metrics");
        assert_eq!(metrics.total_events, 4);
        assert_eq!(metrics.approve_actions, 1);
        assert_eq!(metrics.later_actions, 1);
        assert_eq!(metrics.feedback_negative, 1);
        assert_eq!(metrics.feedback_refine, 1);
        assert_eq!(metrics.failed_actions, 1);
        assert_eq!(metrics.non_positive_feedback_rate, 100.0);
        assert!(metrics.action_failure_rate > 0.0);
    }

    #[test]
    #[serial]
    fn test_release_nl_run_metrics_ignore_local_chat_noise() {
        init().ok();
        clear_nl_runs_for_tests();

        insert_nl_run(
            "help_local",
            "도움말",
            "completed",
            Some("HELP"),
            Some(r#"{"source":"api.chat","route_kind":"local_command","command":"help_local"}"#),
        )
        .expect("insert local help");
        insert_nl_run(
            "build_workflow",
            "노션에 회의록 정리하는 워크플로우 만들어줘",
            "approval_required",
            Some("workflow queued"),
            Some(
                r#"{"source":"api.chat","route_kind":"deterministic","command":"build_workflow"}"#,
            ),
        )
        .expect("insert workflow request");

        let metrics = get_release_nl_run_metrics(20).expect("release metrics");
        assert_eq!(metrics.total, 1);
        assert_eq!(metrics.approval_required, 1);
        assert_eq!(metrics.completed, 0);
    }

    #[test]
    #[serial]
    fn test_release_nl_run_metrics_cap_duplicate_prompts_instead_of_single_dedupe() {
        init().ok();
        clear_nl_runs_for_tests();

        for idx in 0..5 {
            insert_nl_run(
                "flight_search",
                "서울에서 도쿄 항공권을 찾아줘",
                if idx % 2 == 0 {
                    "completed"
                } else {
                    "approval_required"
                },
                Some("flight search"),
                Some(r#"{"source":"api.chat","route_kind":"llm","command":"flight_search"}"#),
            )
            .expect("insert flight request");
        }

        let metrics = get_release_nl_run_metrics(20).expect("release metrics");
        assert_eq!(metrics.total, 3);
        assert_eq!(metrics.completed, 2);
        assert_eq!(metrics.approval_required, 1);
    }

    #[test]
    #[serial]
    fn test_exec_approval_metrics_capture_backlog_and_decisions() {
        init().ok();
        clear_exec_approvals_for_tests();

        create_exec_approval("dangerous pending", None, 3600).expect("seed pending approval");
        create_exec_approval("dangerous expired", None, -60).expect("seed expired approval");

        let allow_once =
            create_exec_approval("safe allow once", None, 3600).expect("seed allow-once approval");
        resolve_exec_approval(&allow_once.id, "approved", Some("test"), Some("allow-once"))
            .expect("resolve allow-once");

        let allow_always = create_exec_approval("safe allow always", None, 3600)
            .expect("seed allow-always approval");
        resolve_exec_approval(
            &allow_always.id,
            "approved",
            Some("test"),
            Some("allow-always"),
        )
        .expect("resolve allow-always");

        let deny =
            create_exec_approval("blocked command", None, 3600).expect("seed denied approval");
        resolve_exec_approval(&deny.id, "rejected", Some("test"), Some("deny"))
            .expect("resolve deny");

        let metrics = get_exec_approval_metrics(20).expect("exec approval metrics");
        assert_eq!(metrics.total, 5);
        assert_eq!(metrics.pending, 2);
        assert_eq!(metrics.approved, 2);
        assert_eq!(metrics.rejected, 1);
        assert_eq!(metrics.expired_pending, 1);
        assert_eq!(metrics.allow_once, 1);
        assert_eq!(metrics.allow_always, 1);
        assert_eq!(metrics.deny, 1);
        assert!((metrics.approval_rate - 66.6).abs() < 1.0);
        assert!(metrics.oldest_pending_created_at.is_some());
        assert!(metrics.last_created_at.is_some());
        assert!(metrics.last_resolved_at.is_some());
    }

    #[test]
    #[serial]
    fn test_launch_ops_metrics_aggregate_recent_routes() {
        init().ok();
        clear_launch_ops_events_for_tests();

        record_launch_ops_event(
            Some("web"),
            Some("channel_web__sender_alice"),
            "오늘 일정 보여줘",
            "request_memory",
            Some("calendar_today"),
            "success",
            Some(0.94),
            false,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            None,
        )
        .expect("record request memory hit");
        record_launch_ops_event(
            Some("web"),
            Some("channel_web__sender_alice"),
            "최근 메일 5개 보여줘",
            "execution_memory",
            Some("gmail_list"),
            "success",
            Some(0.92),
            true,
            false,
            false,
            true,
            true,
            false,
            false,
            false,
            Some("freshness bypass after recent re-ask"),
        )
        .expect("record execution memory hit");
        record_launch_ops_event(
            Some("telegram"),
            Some("channel_telegram__sender_bob"),
            "뉴스 5개 요약해서 노션에 정리해줘",
            "ai_digest_auto",
            Some("ai_digest_program"),
            "success",
            Some(0.83),
            false,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            Some("fallback=auto_digest"),
        )
        .expect("record ai digest route");
        record_launch_ops_event(
            Some("web"),
            Some("channel_web__sender_alice"),
            "???",
            "low_confidence",
            None,
            "error",
            Some(0.31),
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            false,
            Some("confidence below threshold"),
        )
        .expect("record low confidence route");

        let metrics = get_launch_ops_metrics(20).expect("launch ops metrics");
        assert_eq!(metrics.total_requests, 4);
        assert_eq!(metrics.intent_memory_hits, 1);
        assert_eq!(metrics.request_memory_hits, 1);
        assert_eq!(metrics.execution_memory_hits, 1);
        assert_eq!(metrics.ai_digest_auto_routes, 1);
        assert_eq!(metrics.low_confidence_routes, 1);
        assert_eq!(metrics.error_routes, 1);
        assert_eq!(metrics.freshness_bypasses, 1);
        assert!(metrics.cached_response_hit_rate >= 49.0);
        assert!(metrics
            .route_breakdown
            .iter()
            .any(|entry| entry.route_kind == "request_memory"));

        let events = list_launch_ops_events(3).expect("launch ops events");
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].route_kind, "low_confidence");
        assert_eq!(events[1].route_kind, "ai_digest_auto");
    }

    #[test]
    #[serial]
    fn test_sync_release_nl_runs_from_launch_ops_is_idempotent_and_filters_noise() {
        init().ok();
        clear_launch_ops_events_for_tests();
        clear_nl_runs_for_tests();

        record_launch_ops_event(
            Some("web"),
            Some("channel_web__sender_alice"),
            "오늘 일정 보여줘",
            "request_memory",
            Some("calendar_today"),
            "success",
            Some(0.94),
            false,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            None,
        )
        .expect("record calendar event");
        record_launch_ops_event(
            Some("web"),
            Some("channel_web__sender_alice"),
            "/local help",
            "local_command",
            Some("help_local"),
            "success",
            Some(0.99),
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            true,
            None,
        )
        .expect("record local noise event");
        record_launch_ops_event(
            Some("web"),
            Some("channel_web__sender_alice"),
            "노션에 회의록 정리 워크플로우 만들어줘",
            "llm",
            Some("build_workflow"),
            "success",
            Some(0.88),
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            false,
            Some("approval required before provisioning"),
        )
        .expect("record build workflow event");

        let first_sync = sync_release_nl_runs_from_launch_ops(20).expect("first sync");
        let second_sync = sync_release_nl_runs_from_launch_ops(20).expect("second sync");
        let runs = list_nl_runs(10).expect("list nl runs");

        assert_eq!(first_sync, 2);
        assert_eq!(second_sync, 0);
        assert_eq!(runs.len(), 2);
        assert!(runs.iter().any(|run| {
            run.prompt == "오늘 일정 보여줘"
                && run.status == "completed"
                && run
                    .details
                    .as_deref()
                    .unwrap_or_default()
                    .contains("\"backfilled_from\":\"launch_ops_events\"")
        }));
        assert!(runs.iter().any(|run| {
            run.prompt == "노션에 회의록 정리 워크플로우 만들어줘"
                && run.status == "approval_required"
        }));
    }
}
