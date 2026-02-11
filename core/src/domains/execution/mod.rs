// Execution Domain - Execution engines for UI, browser, and shell
//
// Provides trait-based execution system with registry

// New unified framework
pub mod framework;
pub mod engines;

// Re-export framework types
pub use framework::{
    ExecutionEngine, ExecutionContext, ExecutionRegistry, ExecutionResult, EngineType,
};

// Re-export engine implementations
pub use engines::{UIExecutionEngine, BrowserExecutionEngine, ShellExecutionEngine};

// Core execution modules (now local)
pub mod executor;
pub mod visual_driver;
pub mod browser_automation;
pub mod shell_actions;
pub mod execution_controller;

// Re-export for backward compatibility
pub use executor::*;
pub use visual_driver::*;
pub use browser_automation::*;
pub use shell_actions::*;
pub use execution_controller::*;

use anyhow::Result;
use std::sync::Arc;

/// Initialize execution system with standard engines
pub fn init() -> Result<ExecutionRegistry> {
    log::info!("Initializing Execution system...");

    let mut registry = ExecutionRegistry::new();

    // Register standard engines
    registry.register(Arc::new(UIExecutionEngine::new()));
    registry.register(Arc::new(BrowserExecutionEngine::new()));
    registry.register(Arc::new(ShellExecutionEngine::new()));

    log::info!("Execution system initialized with {} engines", 3);
    Ok(registry)
}
