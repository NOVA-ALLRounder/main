use crate::nl_automation::{ExecutionResult, Plan, StepType};
use crate::approval_gate;
use crate::executor;
use crate::visual_driver::{SmartStep, UiAction, VisualDriver};
use crate::browser_automation;
use serde_json::Value;

pub async fn execute_plan(plan: &Plan) -> ExecutionResult {
    let mut logs = Vec::new();
    let mut manual_required = false;
    let mut approval_required = false;
    let mut blocked = false;

    logs.push(format!("Start plan {} ({})", plan.plan_id, plan.intent.as_str()));
    logs.push(summary_for_plan(plan));

    for (idx, step) in plan.steps.iter().enumerate() {
        logs.push(format!("Step {}: {} ({:?})", idx + 1, step.description, step.step_type));
        match step.step_type {
            StepType::Navigate => {
                if let Some(url) = step.data.get("url").and_then(|v| v.as_str()) {
                    if let Err(err) = executor::open_url(url) {
                        logs.push(format!("Failed to open url {}: {}", url, err));
                        return ExecutionResult { status: "error".to_string(), logs };
                    }
                } else {
                    logs.push("Navigate step missing url".to_string());
                }
            }
            StepType::Wait => {
                let seconds = step.data.get("seconds").and_then(|v| v.as_u64()).unwrap_or(1);
                tokio::time::sleep(tokio::time::Duration::from_secs(seconds)).await;
            }
            StepType::Fill | StepType::Select | StepType::Click => {
                if is_auto_step(&step.data) {
                    if let Some(action) = step.data.get("action").and_then(|v| v.as_str()) {
                        if action == "submit_search" {
                            let mut clicked = false;
                            for attempt in 0..2 {
                                match browser_automation::click_search_button() {
                                    Ok(true) => {
                                        logs.push("Clicked search button".to_string());
                                        clicked = true;
                                        break;
                                    }
                                    Ok(false) => {
                                        logs.push(format!("Search button not found (attempt {})", attempt + 1));
                                        if attempt == 0 {
                                            let _ = browser_automation::scroll_page(600);
                                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                                        }
                                    }
                                    Err(err) => {
                                        logs.push(format!("Search click failed: {}", err));
                                        if attempt == 0 {
                                            let _ = browser_automation::scroll_page(600);
                                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                                        }
                                    }
                                }
                            }
                            if !clicked {
                                if let Ok(ctx) = browser_automation::get_page_context() {
                                    logs.push(format!("Page context: {}", ctx));
                                }
                                manual_required = true;
                            }
                            continue;
                        }
                    }
                    if let Some(field) = step.data.get("field").and_then(|v| v.as_str()) {
                        let mut filled = false;
                        for attempt in 0..2 {
                            match try_browser_autofill(plan, field) {
                                Ok(true) => {
                                    logs.push(format!("Auto fill succeeded for {}", field));
                                    filled = true;
                                    break;
                                }
                                Ok(false) => {
                                    logs.push(format!("Auto fill skipped (no match) for {} (attempt {})", field, attempt + 1));
                                    if attempt == 0 {
                                        let _ = browser_automation::scroll_page(400);
                                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                                    }
                                }
                                Err(err) => {
                                    logs.push(format!("Auto fill failed: {} (attempt {})", err, attempt + 1));
                                    if attempt == 0 {
                                        let _ = browser_automation::scroll_page(400);
                                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                                    }
                                }
                            }
                        }
                        if !filled {
                            if let Ok(ctx) = browser_automation::get_page_context() {
                                logs.push(format!("Page context: {}", ctx));
                            }
                            manual_required = true;
                        }
                        continue;
                    }
                    if let Some(value) = step.data.get("value").and_then(|v| v.as_str()) {
                        let mut driver = VisualDriver::new();
                        driver.add_step(SmartStep::new(UiAction::Type(value.to_string()), "Type value"));
                        if let Err(err) = driver.execute(None).await {
                            logs.push(format!("Auto input failed: {}", err));
                            manual_required = true;
                        } else {
                            logs.push("Auto input attempted".to_string());
                        }
                    } else if let Some(query) = step.data.get("query").and_then(|v| v.as_str()) {
                        let mut driver = VisualDriver::new();
                        driver.add_step(SmartStep::new(UiAction::Type(query.to_string()), "Type query"));
                        if let Err(err) = driver.execute(None).await {
                            logs.push(format!("Auto input failed: {}", err));
                            manual_required = true;
                        } else {
                            logs.push("Auto input attempted".to_string());
                        }
                    } else {
                        manual_required = true;
                        logs.push(format!("Manual input required for step '{}'", step.description));
                    }
                } else {
                    manual_required = true;
                    logs.push(format!("Manual input required for step '{}'", step.description));
                }
            }
            StepType::Approve => {
                let action = step
                    .data
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("approve");
                let decision = approval_gate::evaluate_approval(action, plan);
                logs.push(format!(
                    "Approval check: {} (risk {}, policy {})",
                    decision.status, decision.risk_level, decision.policy
                ));
                if decision.status == "denied" {
                    logs.push("Execution blocked by policy".to_string());
                    blocked = true;
                    break;
                }
                if decision.requires_approval {
                    approval_required = true;
                    logs.push("Approval required before continuing".to_string());
                } else {
                    logs.push("Approval auto-granted".to_string());
                }
            }
            StepType::Extract => {
                if let Some(summary) = try_extract_summary(plan) {
                    logs.push(format!("Summary: {}", summary));
                } else {
                    logs.push("No summary extracted".to_string());
                }
            }
            StepType::Screenshot => {}
        }
    }

    if blocked {
        logs.push("Execution stopped due to approval policy".to_string());
        return ExecutionResult { status: "blocked".to_string(), logs };
    }
    if approval_required {
        logs.push("Execution paused awaiting approval".to_string());
        return ExecutionResult { status: "approval_required".to_string(), logs };
    }
    if manual_required {
        logs.push("Execution paused for manual input".to_string());
        return ExecutionResult { status: "manual_required".to_string(), logs };
    }

    ExecutionResult {
        status: "completed".to_string(),
        logs,
    }
}

fn try_browser_autofill(plan: &Plan, field: &str) -> anyhow::Result<bool> {
    match plan.intent {
        crate::nl_automation::IntentType::FlightSearch => {
            if !matches!(field, "from" | "to" | "date_start" | "date_end") {
                return Ok(false);
            }
            let from = plan.slots.get("from").map(|v| v.as_str()).unwrap_or("");
            let to = plan.slots.get("to").map(|v| v.as_str()).unwrap_or("");
            let date_start = plan.slots.get("date_start").map(|v| v.as_str()).unwrap_or("");
            let date_end = plan.slots.get("date_end").map(|v| v.as_str());
            browser_automation::fill_flight_fields(from, to, date_start, date_end)
        }
        crate::nl_automation::IntentType::ShoppingCompare => {
            let query = plan.slots.get("product_name").map(|v| v.as_str()).unwrap_or("");
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

fn try_extract_summary(plan: &Plan) -> Option<String> {
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

fn is_auto_step(data: &Value) -> bool {
    data.get("auto").and_then(|v| v.as_bool()).unwrap_or(false)
}

fn summary_for_plan(plan: &Plan) -> String {
    let slots = &plan.slots;
    match plan.intent {
        crate::nl_automation::IntentType::FlightSearch => {
            let from = slots.get("from").cloned().unwrap_or_else(|| "unknown".to_string());
            let to = slots.get("to").cloned().unwrap_or_else(|| "unknown".to_string());
            let date = slots.get("date_start").cloned().unwrap_or_else(|| "unknown".to_string());
            let budget = slots.get("budget_max").cloned().unwrap_or_else(|| "no budget".to_string());
            format!("Summary: search flights {} → {} on {} (budget {})", from, to, date, budget)
        }
        crate::nl_automation::IntentType::ShoppingCompare => {
            let product = slots.get("product_name").cloned().unwrap_or_else(|| "unknown".to_string());
            let max_price = slots.get("price_max").cloned().unwrap_or_else(|| "no max".to_string());
            format!("Summary: compare prices for {} (max {})", product, max_price)
        }
        crate::nl_automation::IntentType::FormFill => {
            let purpose = slots.get("form_purpose").cloned().unwrap_or_else(|| "unknown".to_string());
            format!("Summary: fill form for {}", purpose)
        }
        crate::nl_automation::IntentType::GenericTask => "Summary: need more details".to_string(),
    }
}
