use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ReplanStrategy {
    pub stop: bool,
    pub reason: &'static str,
    pub severity: &'static str,
    pub fix_hint: Option<&'static str>,
}

pub fn get_replan_strategy(failure_type: &str) -> ReplanStrategy {
    let strategies = build_strategies();
    strategies
        .get(failure_type)
        .cloned()
        .unwrap_or_else(|| ReplanStrategy {
            stop: false,
            reason: "Unknown failure - retry with safer steps",
            severity: "medium",
            fix_hint: Some("Simplify the action, add waits, and verify preconditions before acting."),
        })
}

fn build_strategies() -> HashMap<&'static str, ReplanStrategy> {
    let mut map = HashMap::new();
    map.insert(
        "permission_denied",
        ReplanStrategy {
            stop: true,
            reason: "Permission denied - requires manual intervention",
            severity: "critical",
            fix_hint: Some("Ask the user to grant OS accessibility/automation permissions."),
        },
    );
    map.insert(
        "timeout",
        ReplanStrategy {
            stop: false,
            reason: "Timeout - wait and retry",
            severity: "medium",
            fix_hint: Some("Add a WAIT step before retrying; verify UI is stable."),
        },
    );
    map.insert(
        "network_error",
        ReplanStrategy {
            stop: false,
            reason: "Network error - retry after reconnect",
            severity: "medium",
            fix_hint: Some("Check connectivity; reload page; retry after a short delay."),
        },
    );
    map.insert(
        "element_missing",
        ReplanStrategy {
            stop: false,
            reason: "Element missing - adjust navigation",
            severity: "medium",
            fix_hint: Some("Activate the correct app, scroll, or open the target view before clicking."),
        },
    );
    map.insert(
        "verify_fail",
        ReplanStrategy {
            stop: false,
            reason: "Verification failed - fix the detected issue",
            severity: "medium",
            fix_hint: Some("Use the verification error message to adjust the step; re-check UI state."),
        },
    );
    map.insert(
        "tests_fail",
        ReplanStrategy {
            stop: false,
            reason: "Tests failed - fix failing assertions",
            severity: "high",
            fix_hint: Some("Inspect test output and correct logic without changing expected behavior."),
        },
    );
    map.insert(
        "lint_fail",
        ReplanStrategy {
            stop: false,
            reason: "Lint failed - fix style issues",
            severity: "low",
            fix_hint: Some("Fix lint errors, remove unused imports, and format code."),
        },
    );
    map.insert(
        "build_fail",
        ReplanStrategy {
            stop: false,
            reason: "Build failed - fix imports/dependencies",
            severity: "high",
            fix_hint: Some("Resolve missing modules and ensure dependencies are installed."),
        },
    );
    map.insert(
        "execution_error",
        ReplanStrategy {
            stop: false,
            reason: "Execution error - retry with safer steps",
            severity: "medium",
            fix_hint: Some("Break the action into smaller steps and verify between them."),
        },
    );
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_denied_stops() {
        let strategy = get_replan_strategy("permission_denied");
        assert!(strategy.stop);
        assert_eq!(strategy.severity, "critical");
    }

    #[test]
    fn unknown_fallbacks() {
        let strategy = get_replan_strategy("unknown");
        assert!(!strategy.stop);
        assert!(strategy.reason.contains("Unknown"));
    }
}
