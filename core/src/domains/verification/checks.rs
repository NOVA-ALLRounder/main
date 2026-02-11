// Standard Verification Checks
//
// Implementations of common verification checks

use super::framework::{VerificationCheck, VerificationContext, VerificationResult, Severity};
use anyhow::Result;
use std::path::PathBuf;

// Performance Check
pub struct PerformanceCheck;

impl PerformanceCheck {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PerformanceCheck {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl VerificationCheck for PerformanceCheck {
    fn name(&self) -> &str {
        "performance"
    }

    fn description(&self) -> &str {
        "Checks performance metrics and potential bottlenecks"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    async fn verify(&self, context: &VerificationContext) -> Result<VerificationResult> {
        let workdir = context
            .workdir
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let max_files = context.max_files.unwrap_or(100);

        let result = tokio::task::spawn_blocking(move || {
            crate::performance_verification::performance_baseline(&workdir, max_files)
        })
        .await?;

        let message = if result.ok {
            "Performance check passed".to_string()
        } else {
            format!("Performance issues found: {}", result.reason)
        };

        Ok(VerificationResult {
            check_name: self.name().to_string(),
            passed: result.ok,
            severity: self.severity(),
            message,
            details: Some(serde_json::to_value(&result)?),
        })
    }

    fn enabled_by_default(&self) -> bool {
        false // Performance checks can be slow
    }
}

// Consistency Check
pub struct ConsistencyCheck;

impl ConsistencyCheck {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConsistencyCheck {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl VerificationCheck for ConsistencyCheck {
    fn name(&self) -> &str {
        "consistency"
    }

    fn description(&self) -> &str {
        "Checks codebase consistency (naming, structure, patterns)"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    async fn verify(&self, context: &VerificationContext) -> Result<VerificationResult> {
        let workdir_str = context
            .workdir
            .clone();

        let result = tokio::task::spawn_blocking(move || {
            crate::consistency_check::run_consistency_check(
                crate::consistency_check::ConsistencyCheckRequest { workdir: workdir_str }
            )
        })
        .await?;

        let message = if result.ok {
            "Consistency check passed".to_string()
        } else {
            format!("Consistency issues found: {}", result.summary)
        };

        Ok(VerificationResult {
            check_name: self.name().to_string(),
            passed: result.ok,
            severity: self.severity(),
            message,
            details: Some(serde_json::to_value(&result)?),
        })
    }
}

// Visual Verification Check
pub struct VisualCheck;

impl VisualCheck {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VisualCheck {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl VerificationCheck for VisualCheck {
    fn name(&self) -> &str {
        "visual"
    }

    fn description(&self) -> &str {
        "Visual UI verification using screenshots"
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    async fn verify(&self, _context: &VerificationContext) -> Result<VerificationResult> {
        // Visual verification requires specific setup
        // For now, return a placeholder
        Ok(VerificationResult {
            check_name: self.name().to_string(),
            passed: true,
            severity: self.severity(),
            message: "Visual verification skipped (requires manual setup)".to_string(),
            details: None,
        })
    }

    fn enabled_by_default(&self) -> bool {
        false // Visual checks require specific setup
    }
}
