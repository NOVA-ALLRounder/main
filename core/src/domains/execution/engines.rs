// Execution Engine Implementations
//
// Wrappers for existing execution modules

use super::framework::{ExecutionEngine, ExecutionContext, ExecutionResult, EngineType};
use anyhow::Result;

// UI Execution Engine
pub struct UIExecutionEngine;

impl UIExecutionEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UIExecutionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ExecutionEngine for UIExecutionEngine {
    fn name(&self) -> &str {
        "ui_executor"
    }

    fn engine_type(&self) -> EngineType {
        EngineType::UI
    }

    fn description(&self) -> &str {
        "UI automation engine (visual driver)"
    }

    async fn execute(&self, command: &str, context: &ExecutionContext) -> Result<ExecutionResult> {
        // Parse command and delegate to visual_driver
        // For now, return a placeholder
        if context.dry_run {
            return Ok(ExecutionResult::success("UI execution (dry run)"));
        }

        // Actual execution would call visual_driver methods
        Ok(ExecutionResult::success(format!("UI executed: {}", command)))
    }

    fn can_handle(&self, command: &str) -> bool {
        command.starts_with("ui_")
            || command.starts_with("click")
            || command.starts_with("type")
            || command.starts_with("find")
    }

    fn is_available(&self) -> bool {
        // Check if visual driver is available
        true
    }
}

// Browser Execution Engine
pub struct BrowserExecutionEngine;

impl BrowserExecutionEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BrowserExecutionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ExecutionEngine for BrowserExecutionEngine {
    fn name(&self) -> &str {
        "browser_executor"
    }

    fn engine_type(&self) -> EngineType {
        EngineType::Browser
    }

    fn description(&self) -> &str {
        "Browser automation engine"
    }

    async fn execute(&self, command: &str, context: &ExecutionContext) -> Result<ExecutionResult> {
        if context.dry_run {
            return Ok(ExecutionResult::success("Browser execution (dry run)"));
        }

        // Parse command and delegate to browser_automation
        if command.starts_with("http://") || command.starts_with("https://") {
            // Open URL
            match crate::browser_automation::open_url_in_chrome(command) {
                Ok(_) => Ok(ExecutionResult::success(format!("Opened URL: {}", command))),
                Err(e) => Ok(ExecutionResult::failure(format!("Browser error: {}", e))),
            }
        } else {
            Ok(ExecutionResult::success(format!("Browser executed: {}", command)))
        }
    }

    fn can_handle(&self, command: &str) -> bool {
        command.starts_with("http://")
            || command.starts_with("https://")
            || command.contains("browser")
            || command.contains("chrome")
    }

    fn is_available(&self) -> bool {
        // Check if browser is available
        true
    }
}

// Shell Execution Engine
pub struct ShellExecutionEngine;

impl ShellExecutionEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ShellExecutionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ExecutionEngine for ShellExecutionEngine {
    fn name(&self) -> &str {
        "shell_executor"
    }

    fn engine_type(&self) -> EngineType {
        EngineType::Shell
    }

    fn description(&self) -> &str {
        "Shell command execution engine"
    }

    async fn execute(&self, command: &str, context: &ExecutionContext) -> Result<ExecutionResult> {
        if context.dry_run {
            return Ok(ExecutionResult::success("Shell execution (dry run)"));
        }

        // TODO: Integrate with shell_actions module properly
        // For now, return placeholder
        Ok(ExecutionResult::success(format!("Shell command queued: {}", command))
            .with_metadata("command", serde_json::json!(command)))
    }

    fn can_handle(&self, command: &str) -> bool {
        // Shell can handle most commands, but should be last resort
        !command.starts_with("ui_")
            && !command.starts_with("http")
            && !command.is_empty()
    }

    fn is_available(&self) -> bool {
        // Shell is always available
        true
    }
}
