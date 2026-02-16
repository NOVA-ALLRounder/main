use super::*;

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

pub fn get_recommendations_with_filter(status_filter: Option<&str>) -> Result<Vec<Recommendation>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let sql = match status_filter {
            Some("all") => {
                "SELECT id, status, title, summary, trigger, actions, n8n_prompt, confidence, workflow_id, workflow_json, evidence, pattern_id, last_error FROM recommendations ORDER BY created_at DESC"
            }
            Some(_) => {
                "SELECT id, status, title, summary, trigger, actions, n8n_prompt, confidence, workflow_id, workflow_json, evidence, pattern_id, last_error FROM recommendations WHERE status = ?1 ORDER BY created_at DESC"
            }
            None => {
                "SELECT id, status, title, summary, trigger, actions, n8n_prompt, confidence, workflow_id, workflow_json, evidence, pattern_id, last_error FROM recommendations WHERE status = 'pending' ORDER BY created_at DESC"
            }
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

pub(crate) fn map_row(row: &rusqlite::Row) -> rusqlite::Result<Recommendation> {
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
    })
}

pub fn get_recommendation(id: i64) -> Result<Option<Recommendation>> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, status, title, summary, trigger, actions, n8n_prompt, confidence, workflow_id, workflow_json, evidence, pattern_id, last_error
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
            }));
        }
    }
    Ok(None)
}

pub fn update_recommendation_status(id: i64, status: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "UPDATE recommendations SET status = ?1 WHERE id = ?2",
            params![status, id],
        )?;
    }
    Ok(())
}

pub fn mark_recommendation_failed(id: i64, error: &str) -> Result<()> {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "UPDATE recommendations SET status = 'failed', last_error = ?1 WHERE id = ?2",
            params![error, id],
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
             SET status = 'approved', workflow_id = ?1, workflow_json = ?2, approved_at = ?3
             WHERE id = ?4",
            params![workflow_id, workflow_json, approved_at, id],
        )?;
    }
    Ok(())
}
