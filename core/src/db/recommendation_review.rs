use rusqlite::{params, Result};

use super::{
    get_db_lock, get_recommendation, normalize_admin_limit, row_bool, truncate_text,
    update_recommendation_status,
};

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

#[cfg(test)]
pub fn clear_recommendation_review_events_for_tests() {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let _ = conn.execute("DELETE FROM recommendation_review_events", []);
    }
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
