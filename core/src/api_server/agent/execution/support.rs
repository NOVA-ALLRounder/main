use crate::api_server::inflight_agent_executions;

use super::types::ParsedResumeToken;

pub(super) struct AgentExecutionGuard {
    plan_id: String,
}

#[derive(Debug)]
struct AgentExecutionLockConflict {
    scope: String,
    active_plan_id: Option<String>,
}

impl Drop for AgentExecutionGuard {
    fn drop(&mut self) {
        if let Ok(mut set) = inflight_agent_executions().lock() {
            set.remove(&self.plan_id);
        }
    }
}

fn agent_execution_lock_scope() -> String {
    let configured = std::env::var("STEER_AGENT_EXECUTION_LOCK_SCOPE")
        .ok()
        .unwrap_or_else(|| "global".to_string());
    match configured.trim().to_lowercase().as_str() {
        "plan" | "per_plan" | "per-plan" => "plan".to_string(),
        _ => "global".to_string(),
    }
}

pub(super) fn acquire_agent_execution(
    plan_id: &str,
) -> Result<AgentExecutionGuard, (String, Option<String>)> {
    let scope = agent_execution_lock_scope();
    if let Ok(mut set) = inflight_agent_executions().lock() {
        if scope == "global" {
            if let Some(active) = set.iter().next() {
                let conflict = AgentExecutionLockConflict {
                    scope,
                    active_plan_id: Some(active.to_string()),
                };
                return Err((conflict.scope, conflict.active_plan_id));
            }
            set.insert(plan_id.to_string());
            return Ok(AgentExecutionGuard {
                plan_id: plan_id.to_string(),
            });
        }

        if set.contains(plan_id) {
            let conflict = AgentExecutionLockConflict {
                scope,
                active_plan_id: Some(plan_id.to_string()),
            };
            return Err((conflict.scope, conflict.active_plan_id));
        }
        set.insert(plan_id.to_string());
        return Ok(AgentExecutionGuard {
            plan_id: plan_id.to_string(),
        });
    }
    let conflict = AgentExecutionLockConflict {
        scope,
        active_plan_id: None,
    };
    Err((conflict.scope, conflict.active_plan_id))
}

pub(crate) fn parse_resume_token(raw: &str) -> Result<ParsedResumeToken, String> {
    let token = raw.trim();
    if token.is_empty() {
        return Err("resume_token is empty".to_string());
    }
    let parts: Vec<&str> = token.splitn(5, ':').collect();
    if parts.len() < 5 {
        return Err("resume_token format invalid".to_string());
    }
    if parts[0] != "resume" {
        return Err("resume_token prefix invalid".to_string());
    }
    let plan_id = parts[1].trim();
    if plan_id.is_empty() {
        return Err("resume_token plan_id is empty".to_string());
    }
    let step_index = parts[2]
        .trim()
        .parse::<usize>()
        .map_err(|_| "resume_token step index invalid".to_string())?;
    let reason = parts[3].trim();
    if reason.is_empty() {
        return Err("resume_token reason is empty".to_string());
    }
    Ok(ParsedResumeToken {
        plan_id: plan_id.to_string(),
        step_index,
        reason: reason.to_string(),
    })
}
