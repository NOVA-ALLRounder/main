use crate::nl_automation::{Plan, VerificationResult, StepType};

pub fn verify_plan(plan: &Plan) -> VerificationResult {
    let mut issues = Vec::new();
    if plan.steps.is_empty() {
        issues.push("Plan has no steps".to_string());
    }

    let has_extract = plan.steps.iter().any(|s| matches!(s.step_type, StepType::Extract));
    if !has_extract {
        issues.push("No extract step found for result verification".to_string());
    }

    if matches!(plan.intent, crate::nl_automation::IntentType::FlightSearch) {
        if plan.slots.get("from").map(|v| v.is_empty()).unwrap_or(true) {
            issues.push("Missing flight origin (from)".to_string());
        }
        if plan.slots.get("to").map(|v| v.is_empty()).unwrap_or(true) {
            issues.push("Missing flight destination (to)".to_string());
        }
        if plan.slots.get("date_start").map(|v| v.is_empty()).unwrap_or(true) {
            issues.push("Missing flight start date".to_string());
        }
    }

    if matches!(plan.intent, crate::nl_automation::IntentType::ShoppingCompare) {
        if plan
            .slots
            .get("product_name")
            .map(|v| v.is_empty())
            .unwrap_or(true)
        {
            issues.push("Missing product name".to_string());
        }
    }

    if matches!(plan.intent, crate::nl_automation::IntentType::FormFill) {
        if plan
            .slots
            .get("form_purpose")
            .map(|v| v.is_empty())
            .unwrap_or(true)
        {
            issues.push("Missing form purpose".to_string());
        }
    }

    VerificationResult {
        ok: issues.is_empty(),
        issues,
    }
}
