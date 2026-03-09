//! MCP - Model Context Protocol Client
//!
//! Standard protocol for LLM agents to interact with external services.
//! Inspired by Anthropic's MCP specification.
//!
//! Supports:
//! - Service discovery
//! - Tool invocation
//! - Resource access (files, databases)
//! - Prompt templates

mod client;
mod registry;
mod types;

pub use client::McpClient;
pub use registry::{call_mcp_tool, get_mcp_registry, init_mcp, McpRegistry};
pub use types::{McpPrompt, McpPromptArg, McpResource, McpServer, McpTool};

#[cfg(test)]
mod tests;
