// Verification Domain - Unified verification framework
//
// Provides trait-based verification system with registry

// New unified framework
pub mod framework;
pub mod runtime_check;
pub mod semantic_check;
pub mod checks;

// Re-export framework types
pub use framework::{
    VerificationCheck, VerificationContext, VerificationRegistry, VerificationResult,
    VerificationSummary, Severity,
};

// Re-export check implementations
pub use runtime_check::RuntimeCheck;
pub use semantic_check::SemanticCheck;
pub use checks::{PerformanceCheck, ConsistencyCheck, VisualCheck};

// Core verification modules (now local)
pub mod runtime_verification;
pub mod semantic_verification;
pub mod performance_verification;
pub mod visual_verification;
pub mod consistency_check;
pub mod verification_engine;

// Re-export for backward compatibility
pub use runtime_verification::*;
pub use semantic_verification::*;
pub use performance_verification::*;
pub use visual_verification::*;
pub use consistency_check::*;
pub use verification_engine::*;

// Cross-domain re-exports
pub use crate::static_checks;  // In infrastructure/project
pub use crate::judgment;  // In domains/intelligence

use anyhow::Result;
use std::sync::Arc;

/// Initialize verification system with standard checks
pub fn init() -> Result<VerificationRegistry> {
    log::info!("Initializing Verification system...");

    let mut registry = VerificationRegistry::new();

    // Register standard checks
    registry.register(Arc::new(SemanticCheck::new()));
    registry.register(Arc::new(ConsistencyCheck::new()));
    registry.register(Arc::new(PerformanceCheck::new()));
    registry.register(Arc::new(RuntimeCheck::new()));
    registry.register(Arc::new(VisualCheck::new()));

    log::info!("Verification system initialized with {} checks", 5);
    Ok(registry)
}
