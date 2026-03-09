use super::*;

#[test]
fn business_evidence_accepts_meaningful_summary() {
    let plan = test_plan(IntentType::GenericTask);
    let logs = vec![
        "Start plan".to_string(),
        "Summary: Notes saved and shared".to_string(),
        "Verification passed".to_string(),
    ];

    let (ok, detail) = evaluate_business_evidence(&plan, &logs);
    assert!(ok, "expected evidence to pass, got: {}", detail);
}

#[test]
fn business_evidence_rejects_placeholder_summary() {
    let plan = test_plan(IntentType::GenericTask);
    let logs = vec!["Summary: need more details".to_string()];

    let (ok, detail) = evaluate_business_evidence(&plan, &logs);
    assert!(!ok);
    assert!(detail.contains("missing meaningful summary output"));
}

#[test]
fn business_evidence_rejects_blocking_signal() {
    let plan = test_plan(IntentType::FlightSearch);
    let logs = vec![
        "Summary: search flights Seoul -> Tokyo".to_string(),
        "Execution paused awaiting approval".to_string(),
    ];

    let (ok, detail) = evaluate_business_evidence(&plan, &logs);
    assert!(!ok);
    assert!(detail.contains("blocking signal present"));
}

#[test]
fn business_contract_rejects_missing_flight_slot_in_summary() {
    let mut plan = test_plan(IntentType::FlightSearch);
    plan.slots.insert("from".to_string(), "Seoul".to_string());
    plan.slots.insert("to".to_string(), "Tokyo".to_string());
    plan.slots
        .insert("date_start".to_string(), "2026-03-01".to_string());
    let logs = vec![
        "Summary: search flights Seoul -> unknown on 2026-03-01".to_string(),
        "Verification passed".to_string(),
    ];
    let (ok, detail) = evaluate_business_evidence(&plan, &logs);
    assert!(!ok);
    assert!(detail.contains("contract_missing_to=Tokyo"));
}

#[test]
fn business_contract_accepts_generic_execution_without_summary() {
    let plan = test_plan(IntentType::GenericTask);
    let logs = vec![
        "RUN_ATTEMPT|phase=execution_start|status=running|details=ok|ts=now".to_string(),
        "Step 1: Collect more details from user (Wait)".to_string(),
    ];
    let (ok, detail) = evaluate_business_evidence(&plan, &logs);
    assert!(ok, "expected ok but got {}", detail);
}
