use crate::db;
use crate::nl_automation::{ApprovalDecision, Plan};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
struct ApprovalPolicyStore {
    allow_once: HashMap<String, u32>,
}

lazy_static! {
    static ref POLICY_STORE: Mutex<ApprovalPolicyStore> = Mutex::new(ApprovalPolicyStore::default());
}

pub fn evaluate_approval(action: &str, plan: &Plan) -> ApprovalDecision {
    compute_decision(action, plan, true)
}

pub fn preview_approval(action: &str, plan: &Plan) -> ApprovalDecision {
    compute_decision(action, plan, false)
}

pub fn register_decision(decision: &str, action: &str, plan: &Plan) -> String {
    let key = policy_key(action, plan);
    let normalized = decision.trim().to_lowercase();
    if let Ok(mut store) = POLICY_STORE.lock() {
        match normalized.as_str() {
            "allow_once" | "allow-once" => {
                let entry = store.allow_once.entry(key).or_insert(0);
                *entry += 1;
                return "allow_once".to_string();
            }
            "allow_always" | "allow-always" => {
                let _ = db::upsert_approval_policy(&key, "allow_always");
                return "allow_always".to_string();
            }
            "deny" | "deny_always" | "deny-always" => {
                let _ = db::upsert_approval_policy(&key, "deny_always");
                return "deny_always".to_string();
            }
            "clear" => {
                store.allow_once.remove(&key);
                let _ = db::delete_approval_policy(&key);
                return "cleared".to_string();
            }
            _ => {}
        }
    }
    "none".to_string()
}

fn compute_decision(action: &str, plan: &Plan, consume_once: bool) -> ApprovalDecision {
    let lower = action.to_lowercase();
    let key = policy_key(action, plan);
    let risk_level = risk_level(&lower, plan);
    let mut policy = "none".to_string();
    if let Ok(Some(persisted)) = db::get_approval_policy_decision(&key) {
        if persisted == "deny_always" {
            policy = "deny_always".to_string();
            return ApprovalDecision {
                status: "denied".to_string(),
                requires_approval: false,
                message: "Action denied by policy".to_string(),
                risk_level,
                policy,
            };
        }
        if persisted == "allow_always" {
            policy = "allow_always".to_string();
            return ApprovalDecision {
                status: "approved".to_string(),
                requires_approval: false,
                message: "Action auto-approved (allow always)".to_string(),
                risk_level,
                policy,
            };
        }
    }
    if let Ok(mut store) = POLICY_STORE.lock() {
        if let Some(count) = store.allow_once.get_mut(&key) {
            if *count > 0 {
                policy = "allow_once".to_string();
                if consume_once {
                    *count -= 1;
                    if *count == 0 {
                        store.allow_once.remove(&key);
                    }
                }
                return ApprovalDecision {
                    status: "approved".to_string(),
                    requires_approval: false,
                    message: "Action auto-approved (allow once)".to_string(),
                    risk_level,
                    policy,
                };
            }
        }
    }

    let requires_approval = requires_approval(&risk_level);
    let status = if requires_approval { "pending" } else { "approved" };
    let message = if requires_approval {
        "Approval required before continuing".to_string()
    } else {
        "Action auto-approved".to_string()
    };

    ApprovalDecision {
        status: status.to_string(),
        requires_approval,
        message,
        risk_level,
        policy,
    }
}

fn requires_approval(risk_level: &str) -> bool {
    if risk_level == "high" {
        return true;
    }
    if risk_level == "medium" {
        let flag = std::env::var("STEER_APPROVAL_REQUIRE_MEDIUM")
            .ok()
            .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        return flag;
    }
    false
}

fn contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|kw| text.contains(kw))
}

fn risk_level(action: &str, plan: &Plan) -> String {
    if contains_any(
        action,
        &[
            "purchase",
            "pay",
            "checkout",
            "submit",
            "결제",
            "제출",
            "구매",
            "payment",
        ],
    ) {
        return "high".to_string();
    }
    if contains_any(action, &["login", "signup", "register", "삭제", "delete"]) {
        return "medium".to_string();
    }
    if plan.intent == crate::nl_automation::IntentType::FormFill {
        return "medium".to_string();
    }
    "low".to_string()
}

fn policy_key(action: &str, plan: &Plan) -> String {
    format!("{}::{}", plan.intent.as_str(), action.to_lowercase())
}
