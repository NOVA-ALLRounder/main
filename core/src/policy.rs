// Minimal policy engine implementation
pub struct PolicyEngine;

impl PolicyEngine {
    pub fn check(&self, _action: &crate::schema::AgentAction) -> bool {
        // Default deny all for now, or implement logic based on SECURITY.md
        true 
    }
}
