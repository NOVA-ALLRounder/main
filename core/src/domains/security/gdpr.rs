// GDPR Compliance - Data subject rights implementation
//
// Implements GDPR Articles:
// - Article 15: Right of access
// - Article 17: Right to erasure ("right to be forgotten")
// - Article 20: Right to data portability

use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use std::path::PathBuf;

/// User data export (Article 20: Data Portability)
#[derive(Debug, Serialize)]
pub struct UserDataExport {
    pub user_id: String,
    pub export_date: String,
    pub events: Vec<serde_json::Value>,
    pub patterns: Vec<serde_json::Value>,
    pub recommendations: Vec<serde_json::Value>,
    pub sessions: Vec<serde_json::Value>,
    pub approvals: Vec<serde_json::Value>,
}

/// Data erasure result (Article 17: Right to Erasure)
#[derive(Debug, Serialize)]
pub struct DataErasureResult {
    pub user_id: String,
    pub erasure_date: String,
    pub records_deleted: ErasureStats,
    pub success: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ErasureStats {
    pub events: u64,
    pub patterns: u64,
    pub recommendations: u64,
    pub sessions: u64,
    pub approvals: u64,
    pub total: u64,
}

/// GDPR Compliance Manager
pub struct GdprManager {
    event_store: Option<crate::infrastructure::event_store::EventStore>,
}

impl GdprManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            event_store: None,
        })
    }

    pub fn with_event_store(
        mut self,
        event_store: crate::infrastructure::event_store::EventStore,
    ) -> Self {
        self.event_store = Some(event_store);
        self
    }

    /// Export all user data (GDPR Article 20)
    pub async fn export_user_data(&self, user_id: &str) -> Result<UserDataExport> {
        log::info!("?踰 Exporting data for user: {}", user_id);

        // Export from event store
        let events = if let Some(store) = &self.event_store {
            store
                .get_user_events(user_id)
                .await?
                .into_iter()
                .map(|e| serde_json::to_value(e).unwrap())
                .collect()
        } else {
            vec![]
        };

        // Export from main database
        let _conn = crate::db::get_db_connection()?;

        // Patterns
        let patterns: Vec<serde_json::Value> = vec![]; // TODO: Query patterns table

        // Recommendations
        let recommendations: Vec<serde_json::Value> = vec![]; // TODO: Query recommendations table

        // Sessions
        let sessions: Vec<serde_json::Value> = vec![]; // TODO: Query sessions table

        // Approvals
        let approvals: Vec<serde_json::Value> = vec![]; // TODO: Query approvals table

        log::info!(
            "??Exported {} events, {} patterns, {} recommendations",
            events.len(),
            patterns.len(),
            recommendations.len()
        );

        Ok(UserDataExport {
            user_id: user_id.to_string(),
            export_date: Utc::now().to_rfc3339(),
            events,
            patterns,
            recommendations,
            sessions,
            approvals,
        })
    }

    /// Save export to file (JSON format)
    pub async fn export_to_file(
        &self,
        user_id: &str,
        output_path: &PathBuf,
    ) -> Result<usize> {
        let export = self.export_user_data(user_id).await?;
        let json = serde_json::to_string_pretty(&export)?;

        std::fs::write(output_path, &json)?;

        log::info!("?裕?Saved user data export to: {:?}", output_path);

        Ok(json.len())
    }

    /// Erase all user data (GDPR Article 17)
    pub async fn erase_user_data(&self, user_id: &str) -> Result<DataErasureResult> {
        log::warn!("?肉딀닼? Erasing all data for user: {}", user_id);

        let mut errors = Vec::new();
        let mut stats = ErasureStats {
            events: 0,
            patterns: 0,
            recommendations: 0,
            sessions: 0,
            approvals: 0,
            total: 0,
        };

        // Erase from event store
        if let Some(store) = &self.event_store {
            match store.erase_user_data(user_id).await {
                Ok(count) => {
                    stats.events = count;
                    log::info!("   Deleted {} events", count);
                }
                Err(e) => {
                    errors.push(format!("Failed to delete events: {}", e));
                }
            }
        }

        // Erase from main database
        let conn = crate::db::get_db_connection()?;

        // Delete patterns
        match conn.execute("DELETE FROM patterns WHERE user_id = ?1", [user_id]) {
            Ok(count) => {
                stats.patterns = count as u64;
                log::info!("   Deleted {} patterns", count);
            }
            Err(e) => {
                errors.push(format!("Failed to delete patterns: {}", e));
            }
        }

        // Delete recommendations
        match conn.execute("DELETE FROM recommendations WHERE user_id = ?1", [user_id]) {
            Ok(count) => {
                stats.recommendations = count as u64;
                log::info!("   Deleted {} recommendations", count);
            }
            Err(e) => {
                errors.push(format!("Failed to delete recommendations: {}", e));
            }
        }

        // Delete sessions
        match conn.execute("DELETE FROM sessions WHERE user_id = ?1", [user_id]) {
            Ok(count) => {
                stats.sessions = count as u64;
                log::info!("   Deleted {} sessions", count);
            }
            Err(e) => {
                errors.push(format!("Failed to delete sessions: {}", e));
            }
        }

        // Delete approvals
        match conn.execute("DELETE FROM exec_approvals WHERE user_id = ?1", [user_id]) {
            Ok(count) => {
                stats.approvals = count as u64;
                log::info!("   Deleted {} approvals", count);
            }
            Err(e) => {
                errors.push(format!("Failed to delete approvals: {}", e));
            }
        }

        stats.total = stats.events
            + stats.patterns
            + stats.recommendations
            + stats.sessions
            + stats.approvals;

        let success = errors.is_empty();

        if success {
            log::info!("??Successfully erased {} total records", stats.total);
        } else {
            log::error!("??Erasure completed with {} errors", errors.len());
        }

        Ok(DataErasureResult {
            user_id: user_id.to_string(),
            erasure_date: Utc::now().to_rfc3339(),
            records_deleted: stats,
            success,
            errors,
        })
    }

    /// Get user data summary (Article 15: Right of Access)
    pub async fn get_user_data_summary(&self, user_id: &str) -> Result<UserDataSummary> {
        let mut summary = UserDataSummary {
            user_id: user_id.to_string(),
            event_count: 0,
            pattern_count: 0,
            recommendation_count: 0,
            session_count: 0,
            approval_count: 0,
            first_activity: None,
            last_activity: None,
        };

        // Count events from event store
        if let Some(store) = &self.event_store {
            let events = store.get_user_events(user_id).await?;
            summary.event_count = events.len() as u64;

            if let Some(first) = events.first() {
                summary.first_activity = Some(first.metadata.occurred_at.to_rfc3339());
            }
            if let Some(last) = events.last() {
                summary.last_activity = Some(last.metadata.occurred_at.to_rfc3339());
            }
        }

        // Count from main database
        let conn = crate::db::get_db_connection()?;

        summary.pattern_count = conn
            .query_row(
                "SELECT COUNT(*) FROM patterns WHERE user_id = ?1",
                [user_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        summary.recommendation_count = conn
            .query_row(
                "SELECT COUNT(*) FROM recommendations WHERE user_id = ?1",
                [user_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        summary.session_count = conn
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE user_id = ?1",
                [user_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        summary.approval_count = conn
            .query_row(
                "SELECT COUNT(*) FROM exec_approvals WHERE user_id = ?1",
                [user_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(summary)
    }
}

#[derive(Debug, Serialize)]
pub struct UserDataSummary {
    pub user_id: String,
    pub event_count: u64,
    pub pattern_count: u64,
    pub recommendation_count: u64,
    pub session_count: u64,
    pub approval_count: u64,
    pub first_activity: Option<String>,
    pub last_activity: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gdpr_manager_creation() {
        // Test will fail if database not initialized, which is expected
        let _result = GdprManager::new();
    }
}
