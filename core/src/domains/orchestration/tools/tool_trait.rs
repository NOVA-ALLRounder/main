// Tool trait definition (Clawdbot pattern)

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name (e.g., "windows", "vision", "n8n")
    fn name(&self) -> &str;

    /// Tool description
    fn description(&self) -> &str;

    /// Execute the tool
    async fn execute(&self, params: ToolParams) -> Result<ToolResult>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParams {
    pub action: String,
    pub params: HashMap<String, serde_json::Value>,
}

impl ToolParams {
    pub fn new(action: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            params: HashMap::new(),
        }
    }

    pub fn with_param(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    pub fn get_string(&self, key: &str) -> Result<String> {
        self.params
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid parameter: {}", key))
    }

    pub fn get_i32(&self, key: &str) -> Result<i32> {
        self.params
            .get(key)
            .and_then(|v| v.as_i64())
            .map(|i| i as i32)
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid parameter: {}", key))
    }

    pub fn get_json(&self, key: &str) -> Result<serde_json::Value> {
        self.params
            .get(key)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Missing parameter: {}", key))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl ToolResult {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: None,
        }
    }

    pub fn success_with_data(message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: Some(data),
        }
    }

    pub fn json(data: serde_json::Value) -> Self {
        Self {
            success: true,
            message: "Success".to_string(),
            data: Some(data),
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self {
            success: true,
            message: text.into(),
            data: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
        }
    }
}
