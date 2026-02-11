// Semantic Verification Check
//
// Wraps semantic_verification module in the unified framework

use super::framework::{VerificationCheck, VerificationContext, VerificationResult, Severity};
use crate::semantic_verification;
use anyhow::Result;
use std::path::PathBuf;

pub struct SemanticCheck;

impl SemanticCheck {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SemanticCheck {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl VerificationCheck for SemanticCheck {
    fn name(&self) -> &str {
        "semantic"
    }

    fn description(&self) -> &str {
        "Checks for semantic issues like TODOs, FIXMEs, and unimplemented code"
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
            semantic_verification::semantic_consistency(&workdir, max_files)
        })
        .await?;

        let message = if result.ok {
            "Semantic verification passed".to_string()
        } else {
            format!(
                "Found {} semantic issues: {}",
                result.issues.len(),
                result.reason
            )
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
