// Unified Verification Framework
//
// Provides a trait-based system for registering and running verification checks

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Unified verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub check_name: String,
    pub passed: bool,
    pub severity: Severity,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Context for verification checks
#[derive(Debug, Clone, Default)]
pub struct VerificationContext {
    pub workdir: Option<String>,
    pub max_files: Option<usize>,
    pub backend_port: Option<u16>,
    pub frontend_port: Option<u16>,
    pub timeout_secs: Option<u64>,
    pub custom_params: HashMap<String, String>,
}

/// Trait for all verification checks
#[async_trait::async_trait]
pub trait VerificationCheck: Send + Sync {
    /// Name of the verification check
    fn name(&self) -> &str;

    /// Description of what this check verifies
    fn description(&self) -> &str;

    /// Severity level if this check fails
    fn severity(&self) -> Severity {
        Severity::Error
    }

    /// Run the verification check
    async fn verify(&self, context: &VerificationContext) -> Result<VerificationResult>;

    /// Whether this check should run by default
    fn enabled_by_default(&self) -> bool {
        true
    }
}

/// Registry for verification checks
pub struct VerificationRegistry {
    checks: HashMap<String, Arc<dyn VerificationCheck>>,
}

impl VerificationRegistry {
    pub fn new() -> Self {
        Self {
            checks: HashMap::new(),
        }
    }

    /// Register a verification check
    pub fn register(&mut self, check: Arc<dyn VerificationCheck>) {
        self.checks.insert(check.name().to_string(), check);
    }

    /// Run all enabled checks
    pub async fn run_all(&self, context: &VerificationContext) -> Vec<VerificationResult> {
        let mut results = Vec::new();

        for check in self.checks.values() {
            if !check.enabled_by_default() {
                continue;
            }

            match check.verify(context).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    results.push(VerificationResult {
                        check_name: check.name().to_string(),
                        passed: false,
                        severity: check.severity(),
                        message: format!("Check failed: {}", e),
                        details: None,
                    });
                }
            }
        }

        results
    }

    /// Run specific checks by name
    pub async fn run_checks(
        &self,
        check_names: &[String],
        context: &VerificationContext,
    ) -> Vec<VerificationResult> {
        let mut results = Vec::new();

        for name in check_names {
            if let Some(check) = self.checks.get(name) {
                match check.verify(context).await {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        results.push(VerificationResult {
                            check_name: name.clone(),
                            passed: false,
                            severity: check.severity(),
                            message: format!("Check failed: {}", e),
                            details: None,
                        });
                    }
                }
            }
        }

        results
    }

    /// Get summary of all results
    pub fn summarize(results: &[VerificationResult]) -> VerificationSummary {
        let total = results.len();
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = total - passed;

        let critical = results
            .iter()
            .filter(|r| !r.passed && r.severity == Severity::Critical)
            .count();
        let errors = results
            .iter()
            .filter(|r| !r.passed && r.severity == Severity::Error)
            .count();
        let warnings = results
            .iter()
            .filter(|r| !r.passed && r.severity == Severity::Warning)
            .count();

        let overall_passed = critical == 0 && errors == 0;

        VerificationSummary {
            total,
            passed,
            failed,
            critical,
            errors,
            warnings,
            overall_passed,
        }
    }
}

impl Default for VerificationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub critical: usize,
    pub errors: usize,
    pub warnings: usize,
    pub overall_passed: bool,
}
