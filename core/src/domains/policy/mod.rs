// Policy Domain - Unified policy framework
//
// Provides trait-based policy system with precedence

// New unified framework
pub mod framework;
pub mod policies;

// Re-export framework types
pub use framework::{
    PolicyCheck, PolicyContext, PolicyDecision, PolicyPrecedence, PolicyRegistry, PolicyResult,
    PolicySummary,
};

// Re-export policy implementations
pub use policies::{
    SecurityPolicy, ToolPolicy, ApprovalPolicy, ReleasePolicy, ChatPolicy,
};

// Core policy modules (now local)
pub mod policy;
pub mod tool_policy;
pub mod approval_gate;
pub mod release_gate;
pub mod chat_gate;
pub mod send_policy;
pub mod tool_result_guard;

// Re-export for backward compatibility
pub use policy::*;
pub use tool_policy::*;
pub use approval_gate::*;
pub use release_gate::*;
pub use chat_gate::*;
pub use send_policy::*;
pub use tool_result_guard::*;

use anyhow::Result;
use std::sync::Arc;

/// Initialize policy system with standard policies
pub fn init(write_lock: bool) -> Result<PolicyRegistry> {
    log::info!("Initializing Policy system...");

    let mut registry = PolicyRegistry::new();

    // Register standard policies
    registry.register(Arc::new(SecurityPolicy::new(write_lock)));
    registry.register(Arc::new(ToolPolicy::new()));
    registry.register(Arc::new(ApprovalPolicy::default()));
    registry.register(Arc::new(ReleasePolicy::default()));
    registry.register(Arc::new(ChatPolicy::default()));

    log::info!("Policy system initialized with {} policies", 5);
    Ok(registry)
}
