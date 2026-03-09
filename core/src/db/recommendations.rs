use rusqlite::{params, Result};

use crate::recommendation::AutomationProposal;

use super::get_db_lock;

#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecommendationMetrics {
    pub total: i64,
    pub approved: i64,
    pub rejected: i64,
    pub failed: i64,
    pub pending: i64,
    pub later: i64,
    pub legacy_other: i64,
    pub last_created_at: Option<String>,
}

#[cfg(test)]
pub fn clear_recommendations_for_tests() {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let _ = conn.execute("DELETE FROM recommendations", []);
    }
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
                None::<String>,
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

pub fn seed_advanced_examples() -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let count: i64 =
            conn.query_row("SELECT count(*) FROM recommendations", [], |row| row.get(0))?;

        if count > 0 {
            return Ok(());
        }

        println!("🌱 Seeding advanced workflow templates...");

        let created_at = chrono::Utc::now().to_rfc3339();

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
            None => "SELECT id, status, title, summary, trigger, actions, n8n_prompt, confidence, workflow_id, workflow_json, evidence, pattern_id, last_error, snoozed_until, category, business_score, feedback_status, feedback_note, feedback_count, last_feedback_at FROM recommendations WHERE status = 'pending' ORDER BY created_at DESC",
        };

        let mut stmt = conn.prepare(sql)?;
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

pub fn list_recommendations(status: &str, _limit: i64) -> Result<Vec<Recommendation>> {
    get_recommendations_with_filter(Some(status))
}

pub fn get_recent_recommendations(limit: i64) -> Result<Vec<Recommendation>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, status, title, summary, trigger, actions, n8n_prompt, confidence, workflow_id, workflow_json, evidence, pattern_id, last_error, snoozed_until, category, business_score, feedback_status, feedback_note, feedback_count, last_feedback_at
             FROM recommendations
             WHERE status IN ('pending', 'approved', 'rejected')
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], map_row)?;

        let mut recs = Vec::new();
        for r in rows {
            recs.push(r?);
        }
        return Ok(recs);
    }
    Ok(Vec::new())
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Recommendation> {
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

pub fn mark_recommendation_failed(id: i64, error: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
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
