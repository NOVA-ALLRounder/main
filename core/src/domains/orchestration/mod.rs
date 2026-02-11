// Orchestration Domain - Unified command processing and task routing
//
// This module provides the central orchestration system powered by JARVIS
//
// Components:
// - Orchestrator: Main command processor (JARVIS)
// - CommandParser: Natural language command parsing
// - Tools: Tool registry and implementations
// - Models: Command/Response types
//
// Integration Status: JARVIS is the authoritative implementation
// Future: This domain serves as the stable API for orchestration

// Core orchestration components
pub use crate::jarvis::orchestrator::JarvisOrchestrator as Orchestrator;
pub use crate::jarvis::command_parser::CommandParser;

// Tool system (fully integrated)
pub use crate::jarvis::tools;

// Models (command/response types)
pub use crate::jarvis::models;

// Error handling
pub use crate::jarvis::error_handler;

use anyhow::Result;

/// Initialize the orchestration system
pub async fn init() -> Result<Orchestrator> {
    log::info!("Initializing Orchestration system...");
    let orchestrator = Orchestrator::new().await?;
    log::info!("Orchestration system initialized successfully");
    Ok(orchestrator)
}
