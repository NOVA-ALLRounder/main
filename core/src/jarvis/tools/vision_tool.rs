// Vision Tool (Phase 4 - Stub for now)

use super::tool_trait::{Tool, ToolParams, ToolResult};
use anyhow::Result;
use async_trait::async_trait;

pub struct VisionTool {
    // To be implemented in Phase 4
}

impl VisionTool {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Tool for VisionTool {
    fn name(&self) -> &str {
        "vision"
    }

    fn description(&self) -> &str {
        "Vision-based AI"
    }

    async fn execute(&self, _params: ToolParams) -> Result<ToolResult> {
        Ok(ToolResult::success("VisionTool not yet implemented"))
    }
}
