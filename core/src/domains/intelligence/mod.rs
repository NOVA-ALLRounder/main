// Intelligence Domain - AI engines for pattern detection, vision, and context
//
// This module provides pattern detection and analysis capabilities

// Primary pattern detector (from JARVIS) - for user action sequences
pub use crate::jarvis::engines::pattern_detector::PatternDetector as ActionPatternDetector;
pub use crate::jarvis::engines::pattern_detector::{DetectedPattern as ActionPattern, UserAction, PatternType as ActionPatternType};

// Event pattern detector (legacy) - for system event analysis
// Re-export the old pattern_detector for backward compatibility
// TODO: Phase 2 - Merge event analysis capabilities into ActionPatternDetector
pub use crate::pattern_detector::{
    PatternDetector as EventPatternDetector,
    DetectedPattern as EventPattern,
    PatternType as EventPatternType,
    PatternConfig as EventPatternConfig,
};

// Re-export other intelligence engines from JARVIS
pub use crate::jarvis::engines::{
    context_engine::ContextEngine,
    vision_engine::VisionEngine,
    workflow_builder::WorkflowBuilder,
};

// AI/ML analysis and planning modules
pub mod analyzer;
pub mod architect;
pub mod intent_router;
pub mod llm_gateway;
pub mod nl_automation;
pub mod plan_builder;
pub mod recommendation;
pub mod quality_scorer;
pub mod judgment;

pub use analyzer::*;
pub use architect::*;
pub use intent_router::*;
pub use llm_gateway::*;
pub use nl_automation::*;
pub use plan_builder::*;
pub use recommendation::*;
pub use quality_scorer::*;
pub use judgment::*;

use anyhow::Result;

/// Initialize the intelligence system with both pattern detectors
pub async fn init() -> Result<()> {
    log::info!("Initializing Intelligence system...");

    // Initialize action pattern detector
    let _action_detector = ActionPatternDetector::new().await?;

    // Event pattern detector is initialized on-demand (old synchronous API)
    let _event_detector = EventPatternDetector::new();

    log::info!("Intelligence system initialized successfully");
    Ok(())
}

pub mod pattern_detector;
pub use pattern_detector::*;
