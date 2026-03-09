use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessionize_splits_on_idle_and_gap() {
        let base = Utc::now();
        let events = vec![
            SessionEventRow {
                ts: base,
                event_type: "app.open".to_string(),
                priority: "P1".to_string(),
                app: "A".to_string(),
                resource_type: "doc".to_string(),
                resource_id: "1".to_string(),
                payload: json!({}),
            },
            SessionEventRow {
                ts: base + Duration::seconds(10),
                event_type: "os.idle_start".to_string(),
                priority: "P1".to_string(),
                app: "A".to_string(),
                resource_type: "doc".to_string(),
                resource_id: "1".to_string(),
                payload: json!({}),
            },
            SessionEventRow {
                ts: base + Duration::seconds(1000),
                event_type: "app.open".to_string(),
                priority: "P1".to_string(),
                app: "B".to_string(),
                resource_type: "doc".to_string(),
                resource_id: "2".to_string(),
                payload: json!({}),
            },
        ];

        let sessions = sessionize_events(&events, 900);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].len(), 1);
        assert_eq!(sessions[1].len(), 1);
    }

    #[test]
    fn routine_candidates_require_support() {
        let now = Utc::now();
        let sessions = vec![
            RoutineSession {
                session_id: "s1".to_string(),
                start_ts: now,
                end_ts: now,
                key_events: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            },
            RoutineSession {
                session_id: "s2".to_string(),
                start_ts: now,
                end_ts: now,
                key_events: vec!["a".to_string(), "b".to_string(), "d".to_string()],
            },
        ];

        let candidates = build_routine_candidates(&sessions, 2, 3, 2, 100, 10);
        assert!(!candidates.is_empty());
        assert!(candidates
            .iter()
            .any(|c| c.pattern_json.contains("\"a\",\"b\"")));
    }

    #[test]
    fn handoff_retry_backoff_stops_at_max_attempts() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        ensure_pipeline_tables(&conn).expect("tables");
        let now = format_utc_ts(Utc::now());
        conn.execute(
            "INSERT INTO collector_handoff_queue (
                package_id, created_at, status, payload_json, payload_size, expires_at, error
             ) VALUES (?1, ?2, 'pending', ?3, ?4, NULL, NULL)",
            params!["pkg-1", now, "{\"routine_candidates\":[{}]}", 24i64],
        )
        .expect("insert handoff");
        let id = conn.last_insert_rowid();

        let first = mark_handoff_failed_with_backoff(&conn, id, "parse failed", 2, 1)
            .expect("first failure update");
        assert_eq!(first.attempt_count, 1);
        assert!(!first.terminal);
        assert!(first.next_retry_at.is_some());

        let second = mark_handoff_failed_with_backoff(&conn, id, "parse failed", 2, 1)
            .expect("second failure update");
        assert_eq!(second.attempt_count, 2);
        assert!(second.terminal);
        assert!(second.next_retry_at.is_none());
    }

    #[test]
    fn fetch_retryable_handoff_respects_next_retry_at() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        ensure_pipeline_tables(&conn).expect("tables");
        let now = Utc::now();
        let created = format_utc_ts(now);
        let future = format_utc_ts(now + Duration::seconds(60));
        conn.execute(
            "INSERT INTO collector_handoff_queue (
                package_id, created_at, status, payload_json, payload_size, expires_at, error,
                attempt_count, next_retry_at
             ) VALUES (?1, ?2, 'failed', ?3, ?4, NULL, ?5, ?6, ?7)",
            params![
                "pkg-future",
                created,
                "{\"routine_candidates\":[{}]}",
                24i64,
                "failed",
                1i64,
                future
            ],
        )
        .expect("insert failed row");

        let none = fetch_latest_retryable_handoff(&conn, 3).expect("query");
        assert!(none.is_none());

        let past = format_utc_ts(now - Duration::seconds(1));
        conn.execute(
            "UPDATE collector_handoff_queue SET next_retry_at = ?1 WHERE package_id = 'pkg-future'",
            params![past],
        )
        .expect("update retry ts");

        let row = fetch_latest_retryable_handoff(&conn, 3)
            .expect("query row")
            .expect("row exists");
        assert_eq!(row.package_id, "pkg-future");
        assert_eq!(row.status.to_lowercase(), "failed");
    }

    #[test]
    fn claim_retryable_handoff_skips_live_processing_lease() {
        let mut conn = Connection::open_in_memory().expect("in-memory db");
        ensure_pipeline_tables(&conn).expect("tables");
        let now = Utc::now();
        let created_pending = format_utc_ts(now - Duration::seconds(1));
        let created_processing = format_utc_ts(now);
        let lease_future = format_utc_ts(now + Duration::seconds(60));

        conn.execute(
            "INSERT INTO collector_handoff_queue (
                package_id, created_at, status, payload_json, payload_size, expires_at, error
             ) VALUES (?1, ?2, 'pending', ?3, ?4, NULL, NULL)",
            params![
                "pkg-pending",
                created_pending,
                "{\"routine_candidates\":[{}]}",
                24i64
            ],
        )
        .expect("insert pending row");

        conn.execute(
            "INSERT INTO collector_handoff_queue (
                package_id, created_at, status, payload_json, payload_size, expires_at, error,
                attempt_count, lease_until, claimed_by
             ) VALUES (?1, ?2, 'processing', ?3, ?4, NULL, NULL, ?5, ?6, ?7)",
            params![
                "pkg-processing",
                created_processing,
                "{\"routine_candidates\":[{}]}",
                24i64,
                1i64,
                lease_future,
                "worker-x"
            ],
        )
        .expect("insert processing row");

        let claimed = claim_retryable_handoff(&mut conn, 5, "worker-a", 120)
            .expect("claim")
            .expect("claimed row");
        assert_eq!(claimed.package_id, "pkg-pending");
        assert_eq!(claimed.status, "processing");
        assert_eq!(claimed.claimed_by.as_deref(), Some("worker-a"));
    }
}
