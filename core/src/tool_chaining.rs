//! Tool Chaining Engine - The REAL missing piece
//!
//! This enables complex multi-step scenarios like:
//! "Check calendar, then send summary to Slack"
//!
//! Core concept: Each tool outputs data that can be consumed by the next tool

mod bridge;
mod chain;
mod context;
mod scenario;

pub use bridge::CrossAppBridge;
pub use chain::{FailAction, ToolChain, ToolStep};
pub use context::{ExecutionContext, ToolResult};
pub use scenario::{calculate, execute_scenario, read_from_app, write_to_app};

#[cfg(test)]
#[path = "tool_chaining/tests.rs"]
mod tests;
