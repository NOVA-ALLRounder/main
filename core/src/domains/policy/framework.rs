// Unified Policy Framework
//
// Provides a trait-based system for policy enforcement with precedence

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Policy decision result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PolicyDecision {
    Allow,
    Deny { reason: String },
    RequireApproval { reason: String },
}

impl PolicyDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, PolicyDecision::Allow)
    }

    pub fn is_denied(&self) -> bool {
        matches!(self, PolicyDecision::Deny { .. })
    }

    pub fn requires_approval(&self) -> bool {
        matches!(self, PolicyDecision::RequireApproval { .. })
    }
}

/// Policy precedence level (higher = more restrictive wins)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PolicyPrecedence {
    Low = 1,        // Informational policies
    Medium = 2,     // Standard policies
    High = 3,       // Security policies
    Critical = 4,   // Override everything
}

/// Context for policy evaluation
#[derive(Debug, Clone, Default)]
pub struct PolicyContext {
    pub action_type: Option<String>,
    pub target: Option<String>,
    pub user: Option<String>,
    pub write_lock: bool,
    pub custom_params: HashMap<String, String>,
}

/// Trait for all policy checks
#[async_trait::async_trait]
pub trait PolicyCheck: Send + Sync {
    /// Name of the policy
    fn name(&self) -> &str;

    /// Description of what this policy enforces
    fn description(&self) -> &str;

    /// Precedence level (higher = more restrictive)
    fn precedence(&self) -> PolicyPrecedence {
        PolicyPrecedence::Medium
    }

    /// Evaluate the policy
    async fn evaluate(&self, context: &PolicyContext) -> Result<PolicyDecision>;

    /// Whether this policy is enabled by default
    fn enabled_by_default(&self) -> bool {
        true
    }
}

/// Policy evaluation result
#[derive(Debug, Clone)]
pub struct PolicyResult {
    pub policy_name: String,
    pub decision: PolicyDecision,
    pub precedence: PolicyPrecedence,
}

/// Registry for policy checks with precedence handling
pub struct PolicyRegistry {
    policies: HashMap<String, Arc<dyn PolicyCheck>>,
}

impl PolicyRegistry {
    pub fn new() -> Self {
        Self {
            policies: HashMap::new(),
        }
    }

    /// Register a policy check
    pub fn register(&mut self, policy: Arc<dyn PolicyCheck>) {
        self.policies.insert(policy.name().to_string(), policy);
    }

    /// Evaluate all enabled policies and return the most restrictive decision
    pub async fn evaluate_all(&self, context: &PolicyContext) -> PolicyDecision {
        let mut results = Vec::new();

        for policy in self.policies.values() {
            if !policy.enabled_by_default() {
                continue;
            }

            match policy.evaluate(context).await {
                Ok(decision) => {
                    results.push(PolicyResult {
                        policy_name: policy.name().to_string(),
                        decision,
                        precedence: policy.precedence(),
                    });
                }
                Err(e) => {
                    // On error, deny by default for safety
                    results.push(PolicyResult {
                        policy_name: policy.name().to_string(),
                        decision: PolicyDecision::Deny {
                            reason: format!("Policy evaluation error: {}", e),
                        },
                        precedence: policy.precedence(),
                    });
                }
            }
        }

        Self::apply_precedence(results)
    }

    /// Evaluate specific policies by name
    pub async fn evaluate_policies(
        &self,
        policy_names: &[String],
        context: &PolicyContext,
    ) -> PolicyDecision {
        let mut results = Vec::new();

        for name in policy_names {
            if let Some(policy) = self.policies.get(name) {
                match policy.evaluate(context).await {
                    Ok(decision) => {
                        results.push(PolicyResult {
                            policy_name: name.clone(),
                            decision,
                            precedence: policy.precedence(),
                        });
                    }
                    Err(e) => {
                        results.push(PolicyResult {
                            policy_name: name.clone(),
                            decision: PolicyDecision::Deny {
                                reason: format!("Policy evaluation error: {}", e),
                            },
                            precedence: policy.precedence(),
                        });
                    }
                }
            }
        }

        Self::apply_precedence(results)
    }

    /// Apply precedence rules to determine final decision
    /// Rules:
    /// 1. Any Deny from higher precedence policy = Deny
    /// 2. Any RequireApproval = RequireApproval (unless overridden by higher Deny)
    /// 3. All Allow = Allow
    fn apply_precedence(results: Vec<PolicyResult>) -> PolicyDecision {
        if results.is_empty() {
            return PolicyDecision::Allow; // No policies = allow by default
        }

        // Sort by precedence (highest first)
        let mut sorted = results;
        sorted.sort_by(|a, b| b.precedence.cmp(&a.precedence));

        // Find highest precedence Deny
        for result in &sorted {
            if let PolicyDecision::Deny { reason } = &result.decision {
                return PolicyDecision::Deny {
                    reason: format!("{}: {}", result.policy_name, reason),
                };
            }
        }

        // Find any RequireApproval
        for result in &sorted {
            if let PolicyDecision::RequireApproval { reason } = &result.decision {
                return PolicyDecision::RequireApproval {
                    reason: format!("{}: {}", result.policy_name, reason),
                };
            }
        }

        // All policies allowed
        PolicyDecision::Allow
    }

    /// Get summary of all policy decisions
    pub fn summarize(results: &[PolicyResult]) -> PolicySummary {
        let total = results.len();
        let allowed = results
            .iter()
            .filter(|r| r.decision.is_allowed())
            .count();
        let denied = results
            .iter()
            .filter(|r| r.decision.is_denied())
            .count();
        let approval_required = results
            .iter()
            .filter(|r| r.decision.requires_approval())
            .count();

        PolicySummary {
            total,
            allowed,
            denied,
            approval_required,
        }
    }
}

impl Default for PolicyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySummary {
    pub total: usize,
    pub allowed: usize,
    pub denied: usize,
    pub approval_required: usize,
}
