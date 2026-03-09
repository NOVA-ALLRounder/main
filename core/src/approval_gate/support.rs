use super::*;

/// Decision result expected by execution_controller
#[derive(Debug, Clone)]
pub struct ApprovalDecision {
    pub status: String,
    pub risk_level: String,
    pub policy: String,
    pub message: String,
    pub requires_approval: bool,
}

/// Register decision - legacy API
pub fn register_decision(decision: &str, action: &str, plan: &crate::nl_automation::Plan) {
    let Some(status) = parse_user_decision(decision) else {
        println!(
            "⚠️ [ApprovalGate] Ignored unsupported decision '{}' for action '{}'",
            decision, action
        );
        return;
    };
    let ttl = decision_ttl_for(decision);
    let key = decision_key(&plan.plan_id, action);
    let entry = DecisionEntry {
        status,
        expires_at: Instant::now() + ttl,
    };
    if let Ok(mut registry) = DECISION_REGISTRY.lock() {
        cleanup_expired_decisions_locked(&mut registry);
        registry.insert(key.clone(), entry);
    }
    if let Err(e) = crate::db::upsert_approval_decision(
        &key,
        &plan.plan_id,
        action,
        status_to_storage(status),
        std::cmp::min(ttl.as_secs(), i64::MAX as u64) as i64,
    ) {
        println!(
            "⚠️ [ApprovalGate] Failed to persist decision for plan {}: {}",
            plan.plan_id, e
        );
    }
    println!(
        "📝 [ApprovalGate] Decision '{}' registered for plan {} action: {}",
        decision, plan.plan_id, action
    );
}

/// Preview approval - legacy API  
pub fn preview_approval(action: &str, plan: &crate::nl_automation::Plan) -> ApprovalDecision {
    if let Some(override_status) = get_registered_decision(&plan.plan_id, action) {
        let (status, risk, requires_approval, message) = match override_status {
            ApprovalStatus::Approved => (
                "approved".to_string(),
                "low".to_string(),
                false,
                "User approved this action".to_string(),
            ),
            ApprovalStatus::Denied => (
                "denied".to_string(),
                "high".to_string(),
                true,
                "User denied this action".to_string(),
            ),
            ApprovalStatus::Pending => (
                "pending".to_string(),
                "high".to_string(),
                true,
                "Approval decision is pending".to_string(),
            ),
            ApprovalStatus::Expired => (
                "pending".to_string(),
                "high".to_string(),
                true,
                "Approval decision expired".to_string(),
            ),
        };
        return ApprovalDecision {
            status,
            risk_level: risk,
            policy: "user_decision".to_string(),
            message: format!("{}: {}", message, action),
            requires_approval,
        };
    }

    // Try to parse action as JSON
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(action) {
        let action_type = parsed
            .get("action")
            .and_then(|a| a.as_str())
            .unwrap_or("unknown");
        let action_type_lc = action_type.trim().to_lowercase();

        if action_type == "shell" || action_type == "run_shell" {
            if let Some(cmd) = parsed.get("command").and_then(|c| c.as_str()) {
                let level = ApprovalGate::check_command(cmd);
                let (status, risk, requires_approval) = match level {
                    ApprovalLevel::Blocked => ("denied".to_string(), "critical".to_string(), true),
                    ApprovalLevel::RequireApproval => {
                        ("pending".to_string(), "high".to_string(), true)
                    }
                    ApprovalLevel::AutoApprove => {
                        ("approved".to_string(), "low".to_string(), false)
                    }
                };
                return ApprovalDecision {
                    status,
                    risk_level: risk,
                    policy: "default".to_string(),
                    message: format!("Shell command: {}", cmd),
                    requires_approval,
                };
            }
        }

        if SAFE_NON_SHELL_ACTIONS.contains(action_type_lc.as_str()) {
            return ApprovalDecision {
                status: "approved".to_string(),
                risk_level: "low".to_string(),
                policy: "default".to_string(),
                message: format!("Action: {}", action_type),
                requires_approval: false,
            };
        }

        return ApprovalDecision {
            status: "pending".to_string(),
            risk_level: "high".to_string(),
            policy: "default".to_string(),
            message: format!("Action requires explicit approval: {}", action_type),
            requires_approval: true,
        };
    }

    let trimmed = action.trim();
    let action_lc = trimmed.to_lowercase();
    if matches!(action_lc.as_str(), "done" | "continue" | "next" | "skip") {
        return ApprovalDecision {
            status: "approved".to_string(),
            risk_level: "low".to_string(),
            policy: "default".to_string(),
            message: format!("Action: {}", action),
            requires_approval: false,
        };
    }

    // Plain text action: gate by the same segmented command policy.
    let level = ApprovalGate::check_command(trimmed);
    let (status, risk, requires_approval) = match level {
        ApprovalLevel::Blocked => ("denied".to_string(), "critical".to_string(), true),
        ApprovalLevel::RequireApproval => ("pending".to_string(), "high".to_string(), true),
        ApprovalLevel::AutoApprove => ("approved".to_string(), "low".to_string(), false),
    };
    ApprovalDecision {
        status,
        risk_level: risk,
        policy: "default".to_string(),
        message: format!("Action: {}", action),
        requires_approval,
    }
}

/// Evaluate approval - legacy API (used by execution_controller)
pub fn evaluate_approval(action: &str, plan: &crate::nl_automation::Plan) -> ApprovalDecision {
    let preview = preview_approval(action, plan);
    apply_pending_ask_fallback(preview)
}

fn normalize_action(action: &str) -> String {
    action
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn canonicalize_json_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            let mut sorted = Map::new();
            for key in keys {
                if let Some(item) = map.get(&key) {
                    sorted.insert(key, canonicalize_json_value(item));
                }
            }
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_json_value).collect()),
        _ => value.clone(),
    }
}

fn action_fingerprint(action: &str) -> String {
    let normalized = normalize_action(action);
    let canonical = if let Ok(parsed) = serde_json::from_str::<Value>(&normalized) {
        let stable = canonicalize_json_value(&parsed);
        serde_json::to_string(&stable).unwrap_or(normalized)
    } else {
        normalized.to_lowercase()
    };
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn decision_key(plan_id: &str, action: &str) -> String {
    format!("{}::{}", plan_id, action_fingerprint(action))
}

fn legacy_decision_key(plan_id: &str, action: &str) -> String {
    format!("{}::{}", plan_id, normalize_action(action))
}

fn parse_user_decision(decision: &str) -> Option<ApprovalStatus> {
    let normalized = decision.trim().to_lowercase();
    if matches!(
        normalized.as_str(),
        "approve"
            | "approved"
            | "allow"
            | "allow-once"
            | "allow_once"
            | "allow-always"
            | "allow_always"
            | "yes"
            | "y"
    ) {
        return Some(ApprovalStatus::Approved);
    }
    if matches!(
        normalized.as_str(),
        "deny" | "denied" | "reject" | "rejected" | "no" | "n"
    ) {
        return Some(ApprovalStatus::Denied);
    }
    None
}

fn approval_ask_fallback_mode() -> String {
    std::env::var("STEER_APPROVAL_ASK_FALLBACK")
        .ok()
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "deny".to_string())
}

fn approval_ask_fallback_allow_once_permitted() -> bool {
    let in_test_context = crate::env_flag("STEER_TEST_MODE") || crate::env_flag("CI");
    in_test_context || crate::env_flag("STEER_APPROVAL_ALLOW_ONCE_NON_TEST")
}

fn apply_pending_ask_fallback(mut decision: ApprovalDecision) -> ApprovalDecision {
    if !(decision.requires_approval && decision.status == "pending") {
        return decision;
    }
    match approval_ask_fallback_mode().as_str() {
        "ask" | "pending" => decision,
        "allow" | "allow-once" | "allow_once" | "allow-once-only" => {
            if approval_ask_fallback_allow_once_permitted() {
                decision.status = "approved".to_string();
                decision.requires_approval = false;
                decision.policy = "ask_fallback_allow_once".to_string();
                decision.message = format!("{} [ask_fallback=allow-once]", decision.message);
                crate::diagnostic_events::emit(
                    "approval.ask_fallback",
                    serde_json::json!({
                        "mode": "allow_once",
                        "policy": decision.policy,
                        "status": decision.status,
                        "requires_approval": decision.requires_approval
                    }),
                );
                return decision;
            }
            decision.status = "denied".to_string();
            decision.requires_approval = true;
            decision.policy = "ask_fallback_allow_once_blocked_non_test".to_string();
            decision.message = format!(
                "{} [ask_fallback=allow-once blocked outside test mode]",
                decision.message
            );
            crate::diagnostic_events::emit(
                "approval.ask_fallback",
                serde_json::json!({
                    "mode": "allow_once",
                    "policy": decision.policy,
                    "status": decision.status,
                    "requires_approval": decision.requires_approval
                }),
            );
            decision
        }
        _ => {
            decision.status = "denied".to_string();
            decision.requires_approval = true;
            decision.policy = "ask_fallback_deny".to_string();
            decision.message = format!("{} [ask_fallback=deny]", decision.message);
            crate::diagnostic_events::emit(
                "approval.ask_fallback",
                serde_json::json!({
                    "mode": "deny",
                    "policy": decision.policy,
                    "status": decision.status,
                    "requires_approval": decision.requires_approval
                }),
            );
            decision
        }
    }
}

fn decision_ttl() -> std::time::Duration {
    let ttl_seconds = std::env::var("STEER_APPROVAL_DECISION_TTL_SECONDS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(600);
    Duration::from_secs(ttl_seconds)
}

fn decision_ttl_for(decision: &str) -> std::time::Duration {
    let normalized = decision.trim().to_lowercase();
    if matches!(normalized.as_str(), "allow-always" | "allow_always") {
        let ttl_seconds = std::env::var("STEER_APPROVAL_ALLOW_ALWAYS_TTL_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60 * 60 * 24 * 30);
        return Duration::from_secs(ttl_seconds);
    }
    decision_ttl()
}

fn cleanup_expired_decisions_locked(registry: &mut HashMap<String, DecisionEntry>) {
    let now = Instant::now();
    registry.retain(|_, entry| entry.expires_at > now);
}

fn get_registered_decision(plan_id: &str, action: &str) -> Option<ApprovalStatus> {
    let key = decision_key(plan_id, action);
    let legacy_key = legacy_decision_key(plan_id, action);
    let mut keys = vec![key.clone()];
    if legacy_key != key {
        keys.push(legacy_key.clone());
    }

    if let Ok(mut registry) = DECISION_REGISTRY.lock() {
        cleanup_expired_decisions_locked(&mut registry);
        for candidate in &keys {
            if let Some(entry) = registry.get(candidate).cloned() {
                if candidate != &key {
                    registry.insert(key.clone(), entry.clone());
                }
                return Some(entry.status);
            }
        }
    }

    for candidate in &keys {
        let Some(stored) = crate::db::get_active_approval_decision(candidate)
            .ok()
            .flatten()
        else {
            continue;
        };
        let Some(status) = parse_stored_status(&stored.status) else {
            continue;
        };
        let expires_at = instant_from_expiry(&stored.expires_at)
            .unwrap_or_else(|| Instant::now() + decision_ttl());
        if let Ok(mut registry) = DECISION_REGISTRY.lock() {
            registry.insert(key.clone(), DecisionEntry { status, expires_at });
            if candidate != &key {
                registry.insert(candidate.clone(), DecisionEntry { status, expires_at });
            }
        }
        if candidate != &key {
            let ttl_seconds = chrono::DateTime::parse_from_rfc3339(&stored.expires_at)
                .ok()
                .map(|expiry| {
                    let now = chrono::Utc::now();
                    let expiry_utc = expiry.with_timezone(&chrono::Utc);
                    (expiry_utc - now).num_seconds().max(1)
                })
                .unwrap_or_else(|| decision_ttl().as_secs().min(i64::MAX as u64) as i64);
            let _ = crate::db::upsert_approval_decision(
                &key,
                plan_id,
                action,
                status_to_storage(status),
                ttl_seconds,
            );
        }
        return Some(status);
    }

    None
}

fn status_to_storage(status: ApprovalStatus) -> &'static str {
    match status {
        ApprovalStatus::Pending => "pending",
        ApprovalStatus::Approved => "approved",
        ApprovalStatus::Denied => "denied",
        ApprovalStatus::Expired => "expired",
    }
}

fn parse_stored_status(value: &str) -> Option<ApprovalStatus> {
    match value.trim().to_lowercase().as_str() {
        "pending" => Some(ApprovalStatus::Pending),
        "approved" => Some(ApprovalStatus::Approved),
        "denied" => Some(ApprovalStatus::Denied),
        "expired" => Some(ApprovalStatus::Expired),
        _ => None,
    }
}

fn instant_from_expiry(expires_at: &str) -> Option<Instant> {
    let parsed = chrono::DateTime::parse_from_rfc3339(expires_at).ok()?;
    let expiry_utc = parsed.with_timezone(&chrono::Utc);
    let now_utc = chrono::Utc::now();
    let remaining = (expiry_utc - now_utc).to_std().ok()?;
    Some(Instant::now() + remaining)
}
