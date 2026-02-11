// n8n Workflow models

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct N8nWorkflow {
    pub name: String,
    pub nodes: Vec<N8nNode>,
    pub connections: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct N8nNode {
    pub name: String,
    pub r#type: String,
    pub parameters: serde_json::Value,
    pub position: [i32; 2],
}

impl N8nWorkflow {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            nodes: Vec::new(),
            connections: serde_json::json!({}),
        }
    }

    pub fn add_node(&mut self, node: N8nNode) {
        self.nodes.push(node);
    }
}
