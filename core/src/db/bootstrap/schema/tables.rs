use rusqlite::Connection;

use super::super::ensure_approval_decisions_table;

pub(super) fn apply_base_schema(conn: &Connection) -> anyhow::Result<()> {
    apply_core_tables(conn)?;
    apply_runtime_tables(conn)?;
    apply_indexes(conn)?;
    Ok(())
}

fn apply_core_tables(conn: &Connection) -> anyhow::Result<()> {
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

    ensure_approval_decisions_table(conn);

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

    Ok(())
}

fn apply_runtime_tables(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
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
    )?;

    Ok(())
}

fn apply_indexes(conn: &Connection) -> anyhow::Result<()> {
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
         ON task_run_artifacts(run_id, artifact_type, artifact_key)
        ",
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
    Ok(())
}
