// Context Engine - Track and analyze user context

use crate::jarvis::models::*;
use crate::platform::get_platform;
use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::interval;

/// Context Engine - Tracks user activity and maintains context awareness
pub struct ContextEngine {
    current_context: Arc<RwLock<UserContext>>,
    db_path: Option<String>,
    monitoring: Arc<RwLock<bool>>,
}

#[derive(Debug, Clone)]
pub struct ActivityLog {
    pub timestamp: u64,
    pub app_name: String,
    pub window_title: String,
    pub duration_ms: u64,
}

impl ContextEngine {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            current_context: Arc::new(RwLock::new(UserContext::new())),
            db_path: None,
            monitoring: Arc::new(RwLock::new(false)),
        })
    }

    /// Initialize with database connection
    pub async fn with_db(db_path: &str) -> Result<Self> {
        Ok(Self {
            current_context: Arc::new(RwLock::new(UserContext::new())),
            db_path: Some(db_path.to_string()),
            monitoring: Arc::new(RwLock::new(false)),
        })
    }

    /// Start monitoring user context
    pub async fn start_monitoring(&self) -> Result<()> {
        {
            let mut monitoring = self.monitoring.write().await;
            if *monitoring {
                return Ok(()); // Already monitoring
            }
            *monitoring = true;
        }

        log::info!("Starting context monitoring");

        let current_context = self.current_context.clone();
        let db_path = self.db_path.clone();
        let monitoring = self.monitoring.clone();

        // Spawn monitoring task
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));

            loop {
                interval.tick().await;

                // Check if still monitoring
                if !*monitoring.read().await {
                    log::info!("Context monitoring stopped");
                    break;
                }

                // Get current active app
                if let Ok(platform) = std::panic::catch_unwind(|| get_platform()) {
                    match platform.app_controller().get_active_app() {
                        Ok((app_name, window_title)) => {
                            let mut context = current_context.write().await;

                            // Check if app changed
                            let app_changed = context.active_app.as_ref() != Some(&app_name);

                            if app_changed {
                                log::debug!(
                                    "Context changed: {} - {}",
                                    app_name,
                                    window_title
                                );

                                // Log activity to database in blocking task
                                if let Some(db_path_str) = &db_path {
                                    let db_path_clone = db_path_str.clone();
                                    let app_clone = app_name.clone();
                                    let window_clone = window_title.clone();

                                    tokio::task::spawn_blocking(move || {
                                        if let Ok(conn) = rusqlite::Connection::open(&db_path_clone) {
                                            let timestamp = SystemTime::now()
                                                .duration_since(UNIX_EPOCH)
                                                .unwrap()
                                                .as_secs();

                                            let _ = conn.execute(
                                                "INSERT INTO activity_logs (timestamp, app_name, window_title, duration_ms) VALUES (?1, ?2, ?3, ?4)",
                                                rusqlite::params![timestamp, &app_clone, &window_clone, 0],
                                            );
                                        }
                                    });
                                }

                                // Update context
                                context.update_app(app_name);
                                context.update_window(window_title);
                            }

                            context.increment_activity();
                        }
                        Err(e) => {
                            log::warn!("Failed to get active app: {}", e);
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop monitoring
    pub async fn stop_monitoring(&self) {
        *self.monitoring.write().await = false;
        log::info!("Context monitoring will stop after next check");
    }

    /// Get current context
    pub async fn get_context(&self) -> UserContext {
        self.current_context.read().await.clone()
    }

    /// Update context manually
    pub async fn update_context(&self, context: UserContext) {
        *self.current_context.write().await = context;
    }

    /// Get recent activity from database
    pub async fn get_recent_activity(&self, limit: usize) -> Result<Vec<ActivityLog>> {
        let db_path = self
            .db_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not initialized"))?
            .clone();

        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)?;

            let mut stmt = conn.prepare(
                "SELECT timestamp, app_name, window_title, duration_ms
                 FROM activity_logs
                 ORDER BY timestamp DESC
                 LIMIT ?1",
            )?;

            let logs = stmt
                .query_map([limit], |row| {
                    Ok(ActivityLog {
                        timestamp: row.get(0)?,
                        app_name: row.get(1)?,
                        window_title: row.get(2)?,
                        duration_ms: row.get(3)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(logs)
        })
        .await?
    }

    /// Get activity statistics
    pub async fn get_activity_stats(&self) -> Result<ActivityStats> {
        let db_path = self
            .db_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not initialized"))?
            .clone();

        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)?;

            // Get most used apps
            let mut stmt = conn.prepare(
                "SELECT app_name, COUNT(*) as count
                 FROM activity_logs
                 GROUP BY app_name
                 ORDER BY count DESC
                 LIMIT 10",
            )?;

            let apps = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            // Get total activity count
            let total_activities: u64 = conn.query_row(
                "SELECT COUNT(*) FROM activity_logs",
                [],
                |row| row.get(0),
            )?;

            Ok(ActivityStats {
                total_activities,
                top_apps: apps,
            })
        })
        .await?
    }

    /// Check if user is currently active
    pub async fn is_user_active(&self) -> bool {
        let context = self.current_context.read().await;
        !context.is_idle(300) // 5 minutes threshold
    }

    /// Get context summary for AI
    pub async fn get_context_summary(&self) -> String {
        let context = self.current_context.read().await;

        let app = context
            .active_app
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("Unknown");
        let window = context
            .active_window_title
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("Unknown");

        format!(
            "Current App: {}\nWindow: {}\nActivity Count: {}\nIdle: {}",
            app,
            window,
            context.activity_count,
            context.is_idle(300)
        )
    }
}

#[derive(Debug, Clone)]
pub struct ActivityStats {
    pub total_activities: u64,
    pub top_apps: Vec<(String, u64)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_context_engine_creation() {
        let engine = ContextEngine::new().await.unwrap();
        let context = engine.get_context().await;

        assert!(context.active_app.is_none());
        assert_eq!(context.activity_count, 0);
    }

    #[tokio::test]
    async fn test_context_update() {
        let engine = ContextEngine::new().await.unwrap();

        let mut context = UserContext::new();
        context.update_app("TestApp".to_string());
        context.update_window("Test Window".to_string());

        engine.update_context(context.clone()).await;

        let retrieved = engine.get_context().await;
        assert_eq!(retrieved.active_app, Some("TestApp".to_string()));
        assert_eq!(retrieved.active_window_title, Some("Test Window".to_string()));
    }

    #[tokio::test]
    async fn test_context_summary() {
        let engine = ContextEngine::new().await.unwrap();

        let mut context = UserContext::new();
        context.update_app("VSCode".to_string());
        context.update_window("main.rs".to_string());
        context.activity_count = 42;

        engine.update_context(context).await;

        let summary = engine.get_context_summary().await;
        assert!(summary.contains("VSCode"));
        assert!(summary.contains("main.rs"));
        assert!(summary.contains("42"));
    }

    #[tokio::test]
    async fn test_user_activity_check() {
        let engine = ContextEngine::new().await.unwrap();

        let mut context = UserContext::new();
        context.increment_activity();

        engine.update_context(context).await;

        // Should be active (just updated)
        assert!(engine.is_user_active().await);
    }
}
