use rusqlite::Connection;

use crate::db::{
    backfill_execution_memory_scope_keys, backfill_request_memory_cache_columns,
    backfill_request_memory_scope_keys,
};

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

pub(super) fn run_post_init_repairs(conn: &Connection) {
    ensure_column(
        conn,
        "recommendations",
        "evidence",
        "TEXT NOT NULL DEFAULT '[]'",
    );
    ensure_column(conn, "events_v2", "window_title", "TEXT");
    ensure_column(conn, "events_v2", "browser_url", "TEXT");
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
}
