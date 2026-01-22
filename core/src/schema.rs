use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum AgentAction {
    // Observe
    UiSnapshot { scope: Option<String> },
    UiFind { query: String },
    
    // Act
    UiClick { element_id: String, double_click: bool },
    KeyboardType { text: String, submit: bool },
    
    // System
    Terminate,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AgentCommand {
    pub id: String,
    #[serde(flatten)]
    pub action: AgentAction,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AgentResponse {
    pub request_id: String,
    pub status: String, // "success", "fail"
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}
