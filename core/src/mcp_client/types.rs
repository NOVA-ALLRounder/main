use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    #[serde(default)]
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPrompt {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Option<Vec<McpPromptArg>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptArg {
    pub name: String,
    pub description: Option<String>,
    pub required: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct JsonRpcRequest {
    pub(super) jsonrpc: String,
    pub(super) id: u64,
    pub(super) method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct JsonRpcResponse {
    pub(super) jsonrpc: String,
    pub(super) id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct JsonRpcError {
    pub(super) code: i32,
    pub(super) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) data: Option<Value>,
}
