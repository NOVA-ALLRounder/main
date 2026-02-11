// Skill metadata types - Eligibility checks, requirements, actions

use serde::{Deserialize, Serialize};

/// Skill metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub version: String,
    pub actions: Vec<String>, // Available actions (e.g., ["send", "list", "search"])
    pub requirements: SkillRequirements,
    pub tags: Vec<String>, // For categorization (e.g., ["messaging", "productivity"])
}

/// Skill requirements
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillRequirements {
    /// Required environment variables
    pub env_vars: Vec<String>,

    /// Required binaries in PATH
    pub required_bins: Vec<String>,

    /// At least one of these binaries (OR condition)
    pub any_bins: Vec<String>,

    /// Required platform (windows, macos, linux, None = all)
    pub platform: Option<String>,

    /// Required config keys
    pub config_keys: Vec<String>,
}

/// Eligibility check result
#[derive(Debug, Clone)]
pub struct EligibilityResult {
    pub eligible: bool,
    pub reason: Option<String>,
}

impl EligibilityResult {
    pub fn eligible() -> Self {
        Self {
            eligible: true,
            reason: None,
        }
    }

    pub fn not_eligible(reason: impl Into<String>) -> Self {
        Self {
            eligible: false,
            reason: Some(reason.into()),
        }
    }
}

/// Helper to check environment variable
pub fn check_env_var(var_name: &str) -> bool {
    std::env::var(var_name).is_ok()
}

/// Helper to check if binary exists in PATH
pub fn check_binary(bin_name: &str) -> bool {
    // Try to execute with --version to check existence
    // This works on Windows without external dependencies
    std::process::Command::new(bin_name)
        .arg("--version")
        .output()
        .is_ok()
}

/// Helper to check platform
pub fn check_platform(required: &str) -> bool {
    let current = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    };

    current == required
}

/// Check all requirements
pub fn check_requirements(requirements: &SkillRequirements) -> EligibilityResult {
    // Check platform
    if let Some(ref platform) = requirements.platform {
        if !check_platform(platform) {
            return EligibilityResult::not_eligible(format!(
                "Requires platform: {}",
                platform
            ));
        }
    }

    // Check required env vars
    for var in &requirements.env_vars {
        if !check_env_var(var) {
            return EligibilityResult::not_eligible(format!(
                "Missing environment variable: {}",
                var
            ));
        }
    }

    // Check required binaries
    for bin in &requirements.required_bins {
        if !check_binary(bin) {
            return EligibilityResult::not_eligible(format!(
                "Missing required binary: {}",
                bin
            ));
        }
    }

    // Check any_bins (at least one must exist)
    if !requirements.any_bins.is_empty() {
        let found = requirements.any_bins.iter().any(|bin| check_binary(bin));
        if !found {
            return EligibilityResult::not_eligible(format!(
                "Missing at least one of: {}",
                requirements.any_bins.join(", ")
            ));
        }
    }

    EligibilityResult::eligible()
}
