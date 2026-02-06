// JARVIS - AI Assistant System
// Inspired by Clawdbot architecture patterns

pub mod autonomous_agent;
pub mod channels;  // Phase 1: Multi-channel support
pub mod command_parser;
pub mod computer_use;
pub mod config_manager;  // Phase 3: TOML configuration with hot-reload
pub mod engines;
pub mod error_handler;
pub mod errors;
pub mod event_bus;
pub mod intent_classifier;  // Phase 4: Intent classification
pub mod models;
pub mod observers;
pub mod orchestrator;
pub mod parallel_executor;  // Phase 5: Parallel execution engine
pub mod patterns;
pub mod performance;
pub mod plan_builder;  // Phase 4: Multi-step planning
pub mod privacy;
pub mod proactive_assistant;
pub mod session_manager;
pub mod skills;  // Phase 2: Skill system
pub mod tools;   // Deprecated - migrating to skills

#[cfg(test)]
pub mod tests;

// Re-exports
pub use command_parser::CommandParser;
pub use errors::{JarvisError, JarvisResult};
pub use event_bus::EventBus;
pub use orchestrator::JarvisOrchestrator;
pub use privacy::PrivacyMode;
pub use session_manager::SessionManager;

use anyhow::Result;

/// Initialize the JARVIS system
pub async fn init() -> Result<JarvisOrchestrator> {
    log::info!("Initializing JARVIS system...");

    let orchestrator = JarvisOrchestrator::new().await?;

    log::info!("JARVIS system initialized successfully");
    Ok(orchestrator)
}
