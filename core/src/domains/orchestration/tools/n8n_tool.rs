// N8n Tool (Phase 6 - Stub for now)

use super::tool_trait::{Tool, ToolParams, ToolResult};
use anyhow::Result;
use async_trait::async_trait;

pub struct N8nTool {
    // To be implemented in Phase 6
}

impl N8nTool {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Tool for N8nTool {
    fn name(&self) -> &str {
        "n8n"
    }

    fn description(&self) -> &str {
        "n8n workflow automation"
    }

    async fn execute(&self, _params: ToolParams) -> Result<ToolResult> {
        Ok(ToolResult::success("N8nTool not yet implemented"))
    }
}
