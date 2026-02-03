use crate::executor::PlanStep;

pub fn build_replan_steps(failure_type: &str, failed_step: &PlanStep) -> Vec<PlanStep> {
    let mut steps = Vec::new();
    let fail = failure_type.to_lowercase();

    if fail.contains("timeout") {
        steps.push(PlanStep {
            description: "Wait briefly and retry".to_string(),
            action_type: "WAIT".to_string(),
            target: None,
            value: Some("2".to_string()),
            verification: "Action should be responsive".to_string(),
            pre_check: None,
        });
        steps.push(failed_step.clone());
        return steps;
    }

    if fail.contains("permission") {
        steps.push(PlanStep {
            description: "Prompt user for permission".to_string(),
            action_type: "WAIT".to_string(),
            target: None,
            value: Some("2".to_string()),
            verification: "Permissions granted".to_string(),
            pre_check: Some("System permission dialog visible".to_string()),
        });
        steps.push(failed_step.clone());
        return steps;
    }

    if fail.contains("not found") || fail.contains("missing") {
        // 1) Wait a moment for UI to settle
        steps.push(PlanStep {
            description: "Wait for UI to settle".to_string(),
            action_type: "WAIT".to_string(),
            target: None,
            value: Some("1".to_string()),
            verification: "UI stable".to_string(),
            pre_check: None,
        });

        // 2) Re-activate frontmost app to recover focus
        steps.push(PlanStep {
            description: "Activate frontmost app".to_string(),
            action_type: "ACTIVATE".to_string(),
            target: None,
            value: Some("frontmost".to_string()),
            verification: "App focused".to_string(),
            pre_check: None,
        });

        // 3) Scroll down to reveal hidden elements
        steps.push(PlanStep {
            description: "Scroll down to reveal hidden elements".to_string(),
            action_type: "SCROLL".to_string(),
            target: None,
            value: Some("down".to_string()),
            verification: "More content visible".to_string(),
            pre_check: None,
        });

        // 4) Retry the failed action once
        steps.push(failed_step.clone());

        // 5) If the failed step was a URL open, try reloading the URL
        if failed_step.action_type == "URL" {
            if let Some(value) = failed_step.value.clone() {
                steps.push(PlanStep {
                    description: "Reload target URL".to_string(),
                    action_type: "URL".to_string(),
                    target: None,
                    value: Some(value),
                    verification: failed_step.verification.clone(),
                    pre_check: failed_step.pre_check.clone(),
                });
            }
        } else if failed_step.action_type == "CLICK" {
            // 6) If click failed, try a short wait and re-click
            steps.push(PlanStep {
                description: "Retry click after short wait".to_string(),
                action_type: "WAIT".to_string(),
                target: None,
                value: Some("1".to_string()),
                verification: "Element available".to_string(),
                pre_check: None,
            });
            steps.push(failed_step.clone());
        }

        return steps;
    }

    if fail.contains("network") || fail.contains("connection") || fail.contains("dns") {
        steps.push(PlanStep {
            description: "Wait for network to recover".to_string(),
            action_type: "WAIT".to_string(),
            target: None,
            value: Some("3".to_string()),
            verification: "Network responsive".to_string(),
            pre_check: None,
        });
        if failed_step.action_type == "URL" {
            if let Some(value) = failed_step.value.clone() {
                steps.push(PlanStep {
                    description: "Retry opening URL".to_string(),
                    action_type: "URL".to_string(),
                    target: None,
                    value: Some(value),
                    verification: failed_step.verification.clone(),
                    pre_check: failed_step.pre_check.clone(),
                });
            }
        } else {
            steps.push(failed_step.clone());
        }
        return steps;
    }

    if fail.contains("execution") {
        steps.push(PlanStep {
            description: "Brief pause before retry".to_string(),
            action_type: "WAIT".to_string(),
            target: None,
            value: Some("1".to_string()),
            verification: "UI responsive".to_string(),
            pre_check: None,
        });
        steps.push(failed_step.clone());
        return steps;
    }

    steps.push(failed_step.clone());
    steps
}
