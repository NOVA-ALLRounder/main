use super::*;

pub fn build_routine_candidates(
    sessions: &[RoutineSession],
    n_min: usize,
    n_max: usize,
    min_support: i64,
    max_patterns: usize,
    max_evidence: usize,
) -> Vec<RoutineCandidate> {
    if max_patterns == 0 {
        return Vec::new();
    }

    #[derive(Default)]
    struct PatternStats {
        support: i64,
        session_ids: Vec<String>,
        session_set: HashSet<String>,
        weekday_counts: HashMap<u32, i64>,
        last_seen: Option<DateTime<Utc>>,
    }

    let mut stats: HashMap<Vec<String>, PatternStats> = HashMap::new();

    for session in sessions {
        if session.key_events.len() < n_min {
            continue;
        }

        let patterns = unique_ngrams(&session.key_events, n_min, n_max);
        if patterns.is_empty() {
            continue;
        }

        let weekday = session.start_ts.weekday().num_days_from_monday();
        for pattern in patterns {
            let entry = stats.entry(pattern).or_default();
            if entry.session_set.contains(&session.session_id) {
                continue;
            }

            entry.session_set.insert(session.session_id.clone());
            entry.session_ids.push(session.session_id.clone());
            entry.support += 1;
            *entry.weekday_counts.entry(weekday).or_insert(0) += 1;
            if entry.last_seen.map(|v| session.end_ts > v).unwrap_or(true) {
                entry.last_seen = Some(session.end_ts);
            }
        }
    }

    let now = Utc::now();
    let mut out = Vec::new();

    for (events, entry) in stats {
        if entry.support < min_support {
            continue;
        }

        let last_seen = entry.last_seen.unwrap_or(now);
        let confidence = compute_confidence(entry.support, &entry.weekday_counts, last_seen, now);
        let pattern_json = json!({
            "type": "ngram",
            "events": events,
            "n": events.len()
        })
        .to_string();

        let mut hasher = Sha256::new();
        hasher.update(pattern_json.as_bytes());
        let pattern_id = format!("{:x}", hasher.finalize());

        let evidence = if max_evidence == 0 {
            Vec::new()
        } else {
            let ids = entry.session_ids;
            if ids.len() <= max_evidence {
                ids
            } else {
                let keep_from = ids.len() - max_evidence;
                ids.into_iter().skip(keep_from).collect()
            }
        };

        out.push(RoutineCandidate {
            pattern_id,
            pattern_json,
            support: entry.support,
            confidence,
            last_seen_ts: format_utc_ts(last_seen),
            evidence_session_ids: evidence,
        });
    }

    out.sort_by(|a, b| {
        b.support
            .cmp(&a.support)
            .then_with(|| b.confidence.total_cmp(&a.confidence))
    });
    out.truncate(max_patterns);
    out
}

pub fn clear_routine_candidates(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM collector_routine_candidates", [])?;
    Ok(())
}

pub fn insert_routine_candidate(conn: &Connection, candidate: &RoutineCandidate) -> Result<()> {
    conn.execute(
        "INSERT INTO collector_routine_candidates (
            pattern_id, pattern_json, support, confidence, last_seen_ts, evidence_session_ids
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            candidate.pattern_id,
            candidate.pattern_json,
            candidate.support,
            candidate.confidence,
            candidate.last_seen_ts,
            serde_json::to_string(&candidate.evidence_session_ids)?
        ],
    )?;
    Ok(())
}
