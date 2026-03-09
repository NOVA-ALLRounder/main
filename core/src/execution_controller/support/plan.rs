use super::*;

pub(crate) fn build_resume_token(
    plan: &Plan,
    resume_from: Option<usize>,
    reason: &str,
) -> Option<String> {
    let next_step = resume_from?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Some(format!(
        "resume:{}:{}:{}:{}",
        plan.plan_id, next_step, reason, ts
    ))
}

pub(crate) fn push_run_attempt(logs: &mut Vec<String>, phase: &str, status: &str, details: &str) {
    let ts = chrono::Utc::now().to_rfc3339();
    logs.push(format!(
        "RUN_ATTEMPT|phase={}|status={}|details={}|ts={}",
        phase, status, details, ts
    ));
    let payload = json!({
        "type": "run.attempt",
        "phase": phase,
        "status": status,
        "details": details,
        "ts": ts
    });
    logs.push(format!("RUN_ATTEMPT_JSON|{}", payload));
    crate::diagnostic_events::emit(
        "run.attempt",
        json!({
            "phase": phase,
            "status": status,
            "details": details
        }),
    );
}

pub(crate) fn try_browser_autofill(plan: &Plan, field: &str) -> anyhow::Result<bool> {
    match plan.intent {
        crate::nl_automation::IntentType::FlightSearch => {
            if !matches!(field, "from" | "to" | "date_start" | "date_end") {
                return Ok(false);
            }
            let from = plan.slots.get("from").map(|v| v.as_str()).unwrap_or("");
            let to = plan.slots.get("to").map(|v| v.as_str()).unwrap_or("");
            let date_start = plan
                .slots
                .get("date_start")
                .map(|v| v.as_str())
                .unwrap_or("");
            let date_end = plan.slots.get("date_end").map(|v| v.as_str());
            browser_automation::fill_flight_fields(from, to, date_start, date_end)
        }
        crate::nl_automation::IntentType::ShoppingCompare => {
            let query = plan
                .slots
                .get("product_name")
                .map(|v| v.as_str())
                .unwrap_or("");
            browser_automation::fill_search_query(query)
        }
        crate::nl_automation::IntentType::FormFill => {
            if field != "form_profile" {
                return Ok(false);
            }
            let name = std::env::var("STEER_PROFILE_NAME").ok();
            let email = std::env::var("STEER_PROFILE_EMAIL").ok();
            let phone = std::env::var("STEER_PROFILE_PHONE").ok();
            let address = std::env::var("STEER_PROFILE_ADDRESS").ok();
            browser_automation::autofill_form(
                name.as_deref(),
                email.as_deref(),
                phone.as_deref(),
                address.as_deref(),
            )
        }
        crate::nl_automation::IntentType::GenericTask => Ok(false),
    }
}

pub(crate) fn try_extract_summary(plan: &Plan) -> Option<String> {
    match plan.intent {
        crate::nl_automation::IntentType::FlightSearch => {
            browser_automation::extract_flight_summary().ok()
        }
        crate::nl_automation::IntentType::ShoppingCompare => {
            browser_automation::extract_shopping_summary().ok()
        }
        _ => None,
    }
}

pub(crate) fn is_auto_step(data: &Value) -> bool {
    data.get("auto").and_then(|v| v.as_bool()).unwrap_or(false)
}

pub(crate) fn summary_for_plan(plan: &Plan) -> String {
    let slots = &plan.slots;
    match plan.intent {
        crate::nl_automation::IntentType::FlightSearch => {
            let from = slots
                .get("from")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            let to = slots
                .get("to")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            let date = slots
                .get("date_start")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            let budget = slots
                .get("budget_max")
                .cloned()
                .unwrap_or_else(|| "no budget".to_string());
            format!(
                "Summary: search flights {} → {} on {} (budget {})",
                from, to, date, budget
            )
        }
        crate::nl_automation::IntentType::ShoppingCompare => {
            let product = slots
                .get("product_name")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            let max_price = slots
                .get("price_max")
                .cloned()
                .unwrap_or_else(|| "no max".to_string());
            format!(
                "Summary: compare prices for {} (max {})",
                product, max_price
            )
        }
        crate::nl_automation::IntentType::FormFill => {
            let purpose = slots
                .get("form_purpose")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            format!("Summary: fill form for {}", purpose)
        }
        crate::nl_automation::IntentType::GenericTask => "Summary: need more details".to_string(),
    }
}
