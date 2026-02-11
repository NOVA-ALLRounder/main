// Standard Policy Implementations
//
// Wraps existing policy modules in the unified framework

use super::framework::{PolicyCheck, PolicyContext, PolicyDecision, PolicyPrecedence};
use anyhow::Result;
use crate::schema::AgentAction;

// Security Policy
pub struct SecurityPolicy {
    write_lock: bool,
}

impl SecurityPolicy {
    pub fn new(write_lock: bool) -> Self {
        Self { write_lock }
    }

    pub fn with_lock() -> Self {
        Self { write_lock: true }
    }
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self::with_lock()
    }
}

#[async_trait::async_trait]
impl PolicyCheck for SecurityPolicy {
    fn name(&self) -> &str {
        "security"
    }

    fn description(&self) -> &str {
        "Security policy enforcement (write lock, action classification)"
    }

    fn precedence(&self) -> PolicyPrecedence {
        PolicyPrecedence::High
    }

    async fn evaluate(&self, context: &PolicyContext) -> Result<PolicyDecision> {
        // Use write_lock from context if provided, otherwise use instance setting
        let write_lock = context.write_lock || self.write_lock;

        // For now, check based on action_type if provided
        if let Some(action_type) = &context.action_type {
            let decision = match action_type.as_str() {
                "ui_snapshot" | "ui_find" | "system_search" => PolicyDecision::Allow,
                "ui_click" | "ui_type" | "keyboard_type" | "system_open" => {
                    if write_lock {
                        PolicyDecision::RequireApproval {
                            reason: "Write lock engaged".to_string(),
                        }
                    } else {
                        PolicyDecision::Allow
                    }
                }
                "shell_execution" | "terminate" => PolicyDecision::RequireApproval {
                    reason: "Critical action requires approval".to_string(),
                },
                _ => PolicyDecision::Allow,
            };
            Ok(decision)
        } else {
            // No action type, allow by default
            Ok(PolicyDecision::Allow)
        }
    }
}

// Tool Policy
pub struct ToolPolicy;

impl ToolPolicy {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ToolPolicy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl PolicyCheck for ToolPolicy {
    fn name(&self) -> &str {
        "tool"
    }

    fn description(&self) -> &str {
        "Tool usage policy (which tools are allowed)"
    }

    fn precedence(&self) -> PolicyPrecedence {
        PolicyPrecedence::Medium
    }

    async fn evaluate(&self, context: &PolicyContext) -> Result<PolicyDecision> {
        // Check if action is provided in custom_params
        if let Some(action_str) = context.custom_params.get("action") {
            // Parse action and check with tool_policy
            if let Ok(action) = serde_json::from_str::<AgentAction>(action_str) {
                let allowed = crate::tool_policy::is_action_allowed(&action);
                if allowed {
                    Ok(PolicyDecision::Allow)
                } else {
                    Ok(PolicyDecision::Deny {
                        reason: "Tool policy blocked this action".to_string(),
                    })
                }
            } else {
                Ok(PolicyDecision::Allow)
            }
        } else {
            Ok(PolicyDecision::Allow)
        }
    }
}

// Approval Policy
pub struct ApprovalPolicy {
    require_approval: bool,
}

impl ApprovalPolicy {
    pub fn new(require_approval: bool) -> Self {
        Self { require_approval }
    }

    pub fn always_require() -> Self {
        Self {
            require_approval: true,
        }
    }
}

impl Default for ApprovalPolicy {
    fn default() -> Self {
        Self::new(false)
    }
}

#[async_trait::async_trait]
impl PolicyCheck for ApprovalPolicy {
    fn name(&self) -> &str {
        "approval"
    }

    fn description(&self) -> &str {
        "Approval gate policy (manual approval required)"
    }

    fn precedence(&self) -> PolicyPrecedence {
        PolicyPrecedence::High
    }

    async fn evaluate(&self, _context: &PolicyContext) -> Result<PolicyDecision> {
        if self.require_approval {
            Ok(PolicyDecision::RequireApproval {
                reason: "Manual approval required by policy".to_string(),
            })
        } else {
            Ok(PolicyDecision::Allow)
        }
    }

    fn enabled_by_default(&self) -> bool {
        false // Approval gate is opt-in
    }
}

// Release Policy
pub struct ReleasePolicy {
    allow_release: bool,
}

impl ReleasePolicy {
    pub fn new(allow_release: bool) -> Self {
        Self { allow_release }
    }
}

impl Default for ReleasePolicy {
    fn default() -> Self {
        Self::new(false)
    }
}

#[async_trait::async_trait]
impl PolicyCheck for ReleasePolicy {
    fn name(&self) -> &str {
        "release"
    }

    fn description(&self) -> &str {
        "Release gate policy (production deployment checks)"
    }

    fn precedence(&self) -> PolicyPrecedence {
        PolicyPrecedence::Critical
    }

    async fn evaluate(&self, context: &PolicyContext) -> Result<PolicyDecision> {
        // Check if this is a release action
        if let Some(action_type) = &context.action_type {
            if action_type.contains("release") || action_type.contains("deploy") {
                if self.allow_release {
                    Ok(PolicyDecision::Allow)
                } else {
                    Ok(PolicyDecision::Deny {
                        reason: "Release gate not satisfied".to_string(),
                    })
                }
            } else {
                Ok(PolicyDecision::Allow)
            }
        } else {
            Ok(PolicyDecision::Allow)
        }
    }

    fn enabled_by_default(&self) -> bool {
        false // Release gate is opt-in
    }
}

// Chat Policy
pub struct ChatPolicy {
    filter_enabled: bool,
}

impl ChatPolicy {
    pub fn new(filter_enabled: bool) -> Self {
        Self { filter_enabled }
    }
}

impl Default for ChatPolicy {
    fn default() -> Self {
        Self::new(true)
    }
}

#[async_trait::async_trait]
impl PolicyCheck for ChatPolicy {
    fn name(&self) -> &str {
        "chat"
    }

    fn description(&self) -> &str {
        "Chat content filtering policy"
    }

    fn precedence(&self) -> PolicyPrecedence {
        PolicyPrecedence::Low
    }

    async fn evaluate(&self, context: &PolicyContext) -> Result<PolicyDecision> {
        if !self.filter_enabled {
            return Ok(PolicyDecision::Allow);
        }

        // Check for inappropriate content in target
        if let Some(target) = &context.target {
            let lower = target.to_lowercase();
            if lower.contains("secret") || lower.contains("password") || lower.contains("token") {
                return Ok(PolicyDecision::Deny {
                    reason: "Content may contain sensitive information".to_string(),
                });
            }
        }

        Ok(PolicyDecision::Allow)
    }
}
