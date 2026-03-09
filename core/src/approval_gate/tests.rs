use super::*;
use crate::nl_automation::{IntentType, Plan};
use serial_test::serial;

#[test]
#[serial]
fn test_blocked_commands() {
    assert_eq!(
        ApprovalGate::check_command("rm -rf /"),
        ApprovalLevel::Blocked
    );
    assert_eq!(
        ApprovalGate::check_command("rm -rf /*"),
        ApprovalLevel::Blocked
    );
}

#[test]
#[serial]
fn test_safe_commands() {
    assert_eq!(
        ApprovalGate::check_command("ls -la"),
        ApprovalLevel::AutoApprove
    );
    assert_eq!(
        ApprovalGate::check_command("pwd"),
        ApprovalLevel::AutoApprove
    );
    assert_eq!(
        ApprovalGate::check_command("cat file.txt"),
        ApprovalLevel::RequireApproval
    );
    assert_eq!(
        ApprovalGate::check_command("echo hello world"),
        ApprovalLevel::AutoApprove
    );
    assert_eq!(
        ApprovalGate::check_command("echo 'rm -rf /'"),
        ApprovalLevel::AutoApprove
    );
}

#[test]
#[serial]
fn test_approval_required() {
    assert_eq!(
        ApprovalGate::check_command("sudo apt install git"),
        ApprovalLevel::RequireApproval
    );
    assert_eq!(
        ApprovalGate::check_command("rm -rf mydir"),
        ApprovalLevel::RequireApproval
    );
}

#[test]
#[serial]
fn test_token_aware_classification_avoids_false_positive_contains() {
    assert_eq!(
        ApprovalGate::check_command("echo sudo is blocked"),
        ApprovalLevel::AutoApprove
    );
}

#[test]
#[serial]
fn test_package_install_requires_approval() {
    assert_eq!(
        ApprovalGate::check_command("npm install -g pnpm"),
        ApprovalLevel::RequireApproval
    );
    assert_eq!(
        ApprovalGate::check_command("cargo install ripgrep"),
        ApprovalLevel::RequireApproval
    );
}

#[test]
#[serial]
fn test_unknown_json_action_requires_approval() {
    reset_decisions();
    let plan = test_plan(&format!("plan-unknown-action-{}", uuid::Uuid::new_v4()));
    let action = r#"{"action":"open_app","name":"Safari"}"#;
    let decision = preview_approval(action, &plan);
    assert_eq!(decision.status, "pending");
    assert!(decision.requires_approval);
}

#[test]
#[serial]
fn test_json_action_key_is_canonicalized_for_decision_reuse() {
    reset_decisions();
    let plan = test_plan("plan-json-canonical-key");
    let action_a = r#"{"action":"open_app","name":"Safari","meta":{"b":2,"a":1},"args":[2,1]}"#;
    let action_b = r#"{ "name":"Safari","meta":{"a":1,"b":2},"args":[2,1],"action":"open_app" }"#;
    register_decision("approve", action_a, &plan);
    let decision = preview_approval(action_b, &plan);
    assert_eq!(decision.status, "approved");
    assert!(!decision.requires_approval);
    assert_eq!(decision.policy, "user_decision");
}

#[test]
#[serial]
fn test_safe_bin_with_shell_features_requires_approval() {
    assert_eq!(
        ApprovalGate::check_command("echo hello > /tmp/a.txt"),
        ApprovalLevel::RequireApproval
    );
    assert_eq!(
        ApprovalGate::check_command("cat file.txt | wc -l"),
        ApprovalLevel::RequireApproval
    );
    assert_eq!(
        ApprovalGate::check_command("echo hello | wc -c"),
        ApprovalLevel::AutoApprove
    );
    assert_eq!(
        ApprovalGate::check_command("echo hello && pwd"),
        ApprovalLevel::RequireApproval
    );
}

#[test]
#[serial]
fn test_user_approval_overrides_policy() {
    reset_decisions();
    let plan = test_plan(&format!("plan-approval-override-{}", uuid::Uuid::new_v4()));
    let action = r#"{"action":"shell","command":"sudo apt install git"}"#;

    let before = preview_approval(action, &plan);
    assert_eq!(before.status, "pending");
    assert!(before.requires_approval);

    register_decision("approve", action, &plan);
    let after = preview_approval(action, &plan);
    assert_eq!(after.status, "approved");
    assert!(!after.requires_approval);
    assert_eq!(after.policy, "user_decision");
}

#[test]
#[serial]
fn test_user_denial_overrides_policy() {
    reset_decisions();
    let plan = test_plan(&format!("plan-deny-override-{}", uuid::Uuid::new_v4()));
    let action = r#"{"action":"shell","command":"sudo apt install git"}"#;

    register_decision("deny", action, &plan);
    let decision = evaluate_approval(action, &plan);
    assert_eq!(decision.status, "denied");
    assert!(decision.requires_approval);
    assert_eq!(decision.policy, "user_decision");
}

#[test]
#[serial]
fn test_evaluate_approval_ask_fallback_deny() {
    reset_decisions();
    std::env::set_var("STEER_APPROVAL_ASK_FALLBACK", "deny");
    let plan = test_plan(&format!("plan-fallback-deny-{}", uuid::Uuid::new_v4()));
    let action = r#"{"action":"shell","command":"sudo apt install git"}"#;
    let decision = evaluate_approval(action, &plan);
    assert_eq!(decision.status, "denied");
    assert!(decision.requires_approval);
    assert_eq!(decision.policy, "ask_fallback_deny");
    std::env::remove_var("STEER_APPROVAL_ASK_FALLBACK");
}

#[test]
#[serial]
fn test_evaluate_approval_ask_fallback_allow_once() {
    reset_decisions();
    std::env::set_var("STEER_APPROVAL_ASK_FALLBACK", "allow-once");
    std::env::set_var("STEER_TEST_MODE", "1");
    let plan = test_plan(&format!("plan-fallback-allow-{}", uuid::Uuid::new_v4()));
    let action = r#"{"action":"shell","command":"sudo apt install git"}"#;
    let decision = evaluate_approval(action, &plan);
    assert_eq!(decision.status, "approved");
    assert!(!decision.requires_approval);
    assert_eq!(decision.policy, "ask_fallback_allow_once");
    std::env::remove_var("STEER_TEST_MODE");
    std::env::remove_var("STEER_APPROVAL_ASK_FALLBACK");
}

#[test]
#[serial]
fn test_evaluate_approval_allow_once_blocked_outside_test() {
    reset_decisions();
    std::env::set_var("STEER_APPROVAL_ASK_FALLBACK", "allow-once");
    std::env::remove_var("STEER_TEST_MODE");
    std::env::remove_var("CI");
    std::env::remove_var("STEER_APPROVAL_ALLOW_ONCE_NON_TEST");
    let plan = test_plan(&format!(
        "plan-fallback-allow-blocked-{}",
        uuid::Uuid::new_v4()
    ));
    let action = r#"{"action":"shell","command":"sudo apt install git"}"#;
    let decision = evaluate_approval(action, &plan);
    assert_eq!(decision.status, "denied");
    assert!(decision.requires_approval);
    assert_eq!(decision.policy, "ask_fallback_allow_once_blocked_non_test");
    std::env::remove_var("STEER_APPROVAL_ASK_FALLBACK");
}

#[test]
#[serial]
fn test_evaluate_approval_allow_once_non_test_override() {
    reset_decisions();
    std::env::set_var("STEER_APPROVAL_ASK_FALLBACK", "allow-once");
    std::env::remove_var("STEER_TEST_MODE");
    std::env::remove_var("CI");
    std::env::set_var("STEER_APPROVAL_ALLOW_ONCE_NON_TEST", "1");
    let plan = test_plan(&format!(
        "plan-fallback-allow-non-test-{}",
        uuid::Uuid::new_v4()
    ));
    let action = r#"{"action":"shell","command":"sudo apt install git"}"#;
    let decision = evaluate_approval(action, &plan);
    assert_eq!(decision.status, "approved");
    assert!(!decision.requires_approval);
    assert_eq!(decision.policy, "ask_fallback_allow_once");
    std::env::remove_var("STEER_APPROVAL_ALLOW_ONCE_NON_TEST");
    std::env::remove_var("STEER_APPROVAL_ASK_FALLBACK");
}

#[test]
#[serial]
fn test_evaluate_approval_default_fallback_is_deny() {
    reset_decisions();
    std::env::remove_var("STEER_APPROVAL_ASK_FALLBACK");
    let plan = test_plan(&format!("plan-fallback-default-{}", uuid::Uuid::new_v4()));
    let action = r#"{"action":"shell","command":"sudo apt install git"}"#;
    let decision = evaluate_approval(action, &plan);
    assert_eq!(decision.status, "denied");
    assert!(decision.requires_approval);
    assert_eq!(decision.policy, "ask_fallback_deny");
}

#[test]
#[serial]
fn test_decision_persists_via_db() {
    if let Err(e) = crate::db::init() {
        eprintln!("skip: db init unavailable for persistence test: {}", e);
        return;
    }
    reset_decisions();
    let plan = test_plan(&format!("plan-db-persist-{}", uuid::Uuid::new_v4()));
    let action = r#"{"action":"shell","command":"sudo apt install git"}"#;

    register_decision("approve", action, &plan);
    reset_decisions();

    let after = preview_approval(action, &plan);
    assert_eq!(after.status, "approved");
    assert!(!after.requires_approval);
    assert_eq!(after.policy, "user_decision");
}

fn test_plan(plan_id: &str) -> Plan {
    Plan {
        plan_id: plan_id.to_string(),
        intent: IntentType::GenericTask,
        slots: std::collections::HashMap::new(),
        steps: Vec::new(),
    }
}

fn reset_decisions() {
    if let Ok(mut registry) = DECISION_REGISTRY.lock() {
        registry.clear();
    }
}
