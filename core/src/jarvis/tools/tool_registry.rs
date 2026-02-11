// Tool Registry (Clawdbot pattern)
//
// DEPRECATED: This module is deprecated as of v0.2.0 and will be removed in v0.4.0
// Please migrate to SkillRegistry (jarvis::skills::SkillRegistry)
// See docs/MIGRATION_GUIDE.md for migration instructions

#![allow(deprecated)]

use super::tool_trait::{Tool, ToolParams, ToolResult};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

#[deprecated(
    since = "0.2.0",
    note = "Use SkillRegistry instead. See docs/MIGRATION_GUIDE.md"
)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        log::info!("Registering tool: {}", name);
        self.tools.insert(name, tool);
    }

    /// Execute a tool
    pub async fn execute(&self, tool_name: &str, params: ToolParams) -> Result<ToolResult> {
        let tool = self
            .tools
            .get(tool_name)
            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", tool_name))?;

        log::debug!(
            "Executing tool: {} with action: {}",
            tool_name,
            params.action
        );

        tool.execute(params).await
    }

    /// Get tool by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// List all registered tools
    pub fn list(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Get tool count
    pub fn count(&self) -> usize {
        self.tools.len()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct DummyTool;

    #[async_trait]
    impl Tool for DummyTool {
        fn name(&self) -> &str {
            "dummy"
        }

        fn description(&self) -> &str {
            "A dummy tool"
        }

        async fn execute(&self, _params: ToolParams) -> Result<ToolResult> {
            Ok(ToolResult::success("Dummy executed"))
        }
    }

    #[tokio::test]
    async fn test_tool_registry() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(DummyTool));

        assert_eq!(registry.count(), 1);
        assert!(registry.get("dummy").is_some());

        let result = registry
            .execute("dummy", ToolParams::new("test"))
            .await
            .unwrap();

        assert!(result.success);
    }
}
