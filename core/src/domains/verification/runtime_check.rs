// Runtime Verification Check
//
// Wraps runtime_verification module in the unified framework

use super::framework::{VerificationCheck, VerificationContext, VerificationResult, Severity};
use crate::runtime_verification::{self, RuntimeVerifyOptions};
use anyhow::Result;

pub struct RuntimeCheck {
    run_backend: bool,
    run_frontend: bool,
    run_e2e: bool,
    run_build_checks: bool,
}

impl RuntimeCheck {
    pub fn new() -> Self {
        Self {
            run_backend: true,
            run_frontend: true,
            run_e2e: false,
            run_build_checks: false,
        }
    }

    pub fn with_options(
        run_backend: bool,
        run_frontend: bool,
        run_e2e: bool,
        run_build_checks: bool,
    ) -> Self {
        Self {
            run_backend,
            run_frontend,
            run_e2e,
            run_build_checks,
        }
    }
}

impl Default for RuntimeCheck {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl VerificationCheck for RuntimeCheck {
    fn name(&self) -> &str {
        "runtime"
    }

    fn description(&self) -> &str {
        "Verifies backend and frontend runtime behavior"
    }

    fn severity(&self) -> Severity {
        Severity::Critical
    }

    async fn verify(&self, context: &VerificationContext) -> Result<VerificationResult> {
        let options = RuntimeVerifyOptions {
            workdir: context.workdir.clone(),
            run_backend: Some(self.run_backend),
            run_frontend: Some(self.run_frontend),
            run_e2e: Some(self.run_e2e),
            run_build_checks: Some(self.run_build_checks),
            backend_port: context.backend_port,
            frontend_port: context.frontend_port,
            backend_health_path: None,
        };

        let result = runtime_verification::run_runtime_verification(options).await;

        let passed = result.backend_health
            && result.frontend_health
            && result.e2e_passed.unwrap_or(true);

        let message = if passed {
            "Runtime verification passed".to_string()
        } else {
            format!("Runtime verification failed: {}", result.issues.join(", "))
        };

        Ok(VerificationResult {
            check_name: self.name().to_string(),
            passed,
            severity: self.severity(),
            message,
            details: Some(serde_json::to_value(&result)?),
        })
    }

    fn enabled_by_default(&self) -> bool {
        false // Runtime checks are expensive, enable explicitly
    }
}
