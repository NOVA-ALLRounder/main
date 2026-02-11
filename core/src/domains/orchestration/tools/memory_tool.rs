// Memory Tool (Phase 1.4 - Stub for now)

use super::tool_trait::{Tool, ToolParams, ToolResult};
use anyhow::Result;
use async_trait::async_trait;

pub struct MemoryTool {
    // To be implemented in Phase 1.4
}

impl MemoryTool {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str {
        "memory"
    }

    fn description(&self) -> &str {
        "Memory storage"
    }

    async fn execute(&self, _params: ToolParams) -> Result<ToolResult> {
        Ok(ToolResult::success("MemoryTool not yet implemented"))
    }
}
