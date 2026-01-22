use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "action", content = "payload")]
pub enum AgentAction {
    // Observe
    #[serde(rename = "ui.snapshot")]
    UiSnapshot { scope: Option<String> },
    #[serde(rename = "ui.find")]
    UiFind { query: String },
    
    // Act
    #[serde(rename = "ui.click")]
    UiClick { element_id: String, double_click: bool },
    #[serde(rename = "keyboard.type")]
    KeyboardType { text: String, submit: bool },
    
    // System
    #[serde(rename = "system.terminate")]
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
