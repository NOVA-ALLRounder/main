// Tool Policy (Phase 1 - Stub for now)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPolicy {
    pub allowed: Vec<String>,
    pub denied: Vec<String>,
    pub require_approval: Vec<String>,
}

impl Default for ToolPolicy {
    fn default() -> Self {
        Self {
            allowed: vec!["*".to_string()], // Allow all by default
            denied: Vec::new(),
            require_approval: Vec::new(),
        }
    }
}

pub struct ToolPolicyManager {
    // To be implemented in Phase 1.4
}

impl ToolPolicyManager {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for ToolPolicyManager {
    fn default() -> Self {
        Self::new()
    }
}
