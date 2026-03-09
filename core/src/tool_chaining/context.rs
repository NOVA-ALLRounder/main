use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Result from a tool execution that can be passed to next tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_name: String,
    pub success: bool,
    pub output: Value,
    pub extracted_data: HashMap<String, String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ToolResult {
    pub fn success(tool_name: &str, output: Value) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            success: true,
            output,
            extracted_data: HashMap::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn failed(tool_name: &str, error: &str) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            success: false,
            output: serde_json::json!({ "error": error }),
            extracted_data: HashMap::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn with_data(mut self, key: &str, value: &str) -> Self {
        self.extracted_data
            .insert(key.to_string(), value.to_string());
        self
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.extracted_data.get(key)
    }
}

/// Shared context across tool chain execution
#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    pub variables: HashMap<String, String>,
    pub tool_results: Vec<ToolResult>,
    pub clipboard: Option<String>,
    pub current_app: Option<String>,
    pub last_screenshot: Option<String>,
}

impl ExecutionContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.variables.insert(key.to_string(), value.to_string());
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.variables.get(key)
    }

    pub fn last_result(&self) -> Option<&ToolResult> {
        self.tool_results.last()
    }

    pub fn add_result(&mut self, result: ToolResult) {
        for (k, v) in &result.extracted_data {
            self.variables.insert(k.clone(), v.clone());
        }
        self.tool_results.push(result);
    }

    pub fn substitute(&self, template: &str) -> String {
        let mut result = template.to_string();
        for (key, value) in &self.variables {
            result = result.replace(&format!("{{{{{}}}}}", key), value);
        }
        for (key, value) in &self.variables {
            result = result.replace(&format!("${}", key), value);
        }
        result
    }
}
