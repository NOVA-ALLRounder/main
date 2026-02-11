// Session management (Clawdbot pattern) with SQLite persistence
// Phase 3: Write-through cache pattern

use crate::infrastructure::database::{load_sessions, save_session};
use crate::jarvis::models::UserContext;
use anyhow::Result;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub type SessionKey = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub key: SessionKey,
    pub context: UserContext,
    pub state: SessionState,
    pub created_at: SystemTime,
    pub last_activity: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionState {
    Active,
    Idle,
    Stopped,
}

#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<DashMap<SessionKey, Session>>,
    idle_timeout: Duration,
    persist: bool, // Enable/disable persistence
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            idle_timeout: Duration::from_secs(24 * 3600), // 24 hours
            persist: false,
        }
    }

    pub fn with_idle_timeout(idle_timeout: Duration) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            idle_timeout,
            persist: false,
        }
    }

    /// Create with database persistence enabled
    pub fn with_persistence() -> Result<Self> {
        let manager = Self {
            sessions: Arc::new(DashMap::new()),
            idle_timeout: Duration::from_secs(24 * 3600),
            persist: true,
        };

        // Load existing sessions from database
        manager.load_from_db()?;

        Ok(manager)
    }

    /// Load sessions from database
    fn load_from_db(&self) -> Result<()> {
        if !self.persist {
            return Ok(());
        }

        let sessions = load_sessions()?;

        let mut loaded = 0;
        for (key, context_json, state_str, created_at, last_activity) in sessions {
            let context: UserContext = serde_json::from_str(&context_json)
                .unwrap_or_else(|_| UserContext::new());

            let state = match state_str.as_str() {
                "idle" => SessionState::Idle,
                "stopped" => SessionState::Stopped,
                _ => SessionState::Active,
            };

            let session = Session {
                key: key.clone(),
                context,
                state,
                created_at: UNIX_EPOCH + Duration::from_secs(created_at as u64),
                last_activity: UNIX_EPOCH + Duration::from_secs(last_activity as u64),
            };

            self.sessions.insert(key, session);
            loaded += 1;
        }

        log::info!("Loaded {} sessions from database", loaded);
        Ok(())
    }

    /// Save session to database
    fn save_to_db(&self, session: &Session) -> Result<()> {
        if !self.persist {
            return Ok(());
        }

        let context_json = serde_json::to_string(&session.context)?;
        let state_str = match session.state {
            SessionState::Active => "active",
            SessionState::Idle => "idle",
            SessionState::Stopped => "stopped",
        };

        let created_ts = session.created_at.duration_since(UNIX_EPOCH)?.as_secs() as i64;
        let activity_ts = session.last_activity.duration_since(UNIX_EPOCH)?.as_secs() as i64;

        save_session(
            &session.key,
            &context_json,
            state_str,
            created_ts,
            activity_ts,
        )?;

        Ok(())
    }

    /// Create or get a session
    pub fn get_or_create(&self, key: impl Into<SessionKey>) -> Session {
        let key = key.into();

        let session = self.sessions
            .entry(key.clone())
            .or_insert_with(|| Session {
                key: key.clone(),
                context: UserContext::new(),
                state: SessionState::Active,
                created_at: SystemTime::now(),
                last_activity: SystemTime::now(),
            })
            .clone();

        // Save to database (write-through)
        if let Err(e) = self.save_to_db(&session) {
            log::warn!("Failed to save session to database: {}", e);
        }

        session
    }

    /// Update session context
    pub fn update_context(&self, key: impl AsRef<str>, context: UserContext) -> Result<()> {
        let key = key.as_ref();
        if let Some(mut session) = self.sessions.get_mut(key) {
            session.context = context;
            session.last_activity = SystemTime::now();
            session.state = SessionState::Active;

            // Save to database (write-through)
            let session_clone = session.clone();
            drop(session); // Release lock before DB write
            if let Err(e) = self.save_to_db(&session_clone) {
                log::warn!("Failed to save session to database: {}", e);
            }

            Ok(())
        } else {
            anyhow::bail!("Session not found: {}", key)
        }
    }

    /// Get session
    pub fn get(&self, key: impl AsRef<str>) -> Option<Session> {
        let key = key.as_ref();
        self.sessions.get(key).map(|entry| entry.clone())
    }

    /// Update session state
    pub fn set_state(&self, key: impl AsRef<str>, state: SessionState) -> Result<()> {
        let key = key.as_ref();
        if let Some(mut session) = self.sessions.get_mut(key) {
            session.state = state;

            // Save to database (write-through)
            let session_clone = session.clone();
            drop(session); // Release lock before DB write
            if let Err(e) = self.save_to_db(&session_clone) {
                log::warn!("Failed to save session to database: {}", e);
            }

            Ok(())
        } else {
            anyhow::bail!("Session not found: {}", key)
        }
    }

    /// Mark session as active (touch)
    pub fn touch(&self, key: impl AsRef<str>) -> Result<()> {
        let key = key.as_ref();
        if let Some(mut session) = self.sessions.get_mut(key) {
            session.last_activity = SystemTime::now();
            session.state = SessionState::Active;

            // Save to database (write-through)
            let session_clone = session.clone();
            drop(session); // Release lock before DB write
            if let Err(e) = self.save_to_db(&session_clone) {
                log::warn!("Failed to save session to database: {}", e);
            }

            Ok(())
        } else {
            anyhow::bail!("Session not found: {}", key)
        }
    }

    /// Cleanup idle sessions
    pub fn cleanup_idle_sessions(&self) -> usize {
        let now = SystemTime::now();
        let mut removed = 0;

        self.sessions.retain(|_, session| {
            if let Ok(elapsed) = now.duration_since(session.last_activity) {
                if elapsed > self.idle_timeout {
                    removed += 1;
                    return false;
                }
            }
            true
        });

        removed
    }

    /// List all active sessions
    pub fn list_active(&self) -> Vec<Session> {
        self.sessions
            .iter()
            .filter(|entry| entry.value().state == SessionState::Active)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get session count
    pub fn count(&self) -> usize {
        self.sessions.len()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_manager() {
        let manager = SessionManager::new();

        // Create session
        let session = manager.get_or_create("test-session");
        assert_eq!(session.key, "test-session");
        assert_eq!(session.state, SessionState::Active);

        // Update context
        let mut context = UserContext::new();
        context.update_app("notepad.exe".to_string());
        manager.update_context(&session.key, context).unwrap();

        // Get session
        let updated = manager.get("test-session").unwrap();
        assert_eq!(updated.context.active_app, Some("notepad.exe".to_string()));

        // Touch session
        manager.touch("test-session").unwrap();

        // Set state
        manager
            .set_state("test-session", SessionState::Idle)
            .unwrap();
        let idle = manager.get("test-session").unwrap();
        assert_eq!(idle.state, SessionState::Idle);
    }

    #[test]
    fn test_cleanup_idle_sessions() {
        let manager = SessionManager::with_idle_timeout(Duration::from_secs(1));

        manager.get_or_create("session1");
        manager.get_or_create("session2");

        assert_eq!(manager.count(), 2);

        // Wait for sessions to become idle
        std::thread::sleep(Duration::from_secs(2));

        let removed = manager.cleanup_idle_sessions();
        assert_eq!(removed, 2);
        assert_eq!(manager.count(), 0);
    }
}
