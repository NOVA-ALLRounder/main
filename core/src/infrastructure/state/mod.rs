// State Management Infrastructure - Unified session and state management
//
// This module provides a unified approach to managing session state across
// different subsystems (NL automation, JARVIS conversations, etc.)

// Re-export JARVIS SessionManager as the primary session manager
// It has better architecture (DashMap, TTL support, async-ready)
pub use crate::jarvis::session_manager::{
    SessionManager,
    Session,
    SessionState,
    SessionKey,
};

// Local modules
pub mod session;
pub mod nl_store;

// Re-export for backward compatibility
pub use session::*;
pub use nl_store::*;

use anyhow::Result;

/// Initialize the state management system
pub async fn init() -> Result<SessionManager> {
    log::info!("Initializing State Management system...");

    // Create a global session manager with 24-hour timeout
    let manager = SessionManager::with_idle_timeout(std::time::Duration::from_secs(24 * 3600));

    log::info!("State Management system initialized successfully");
    Ok(manager)
}
