use super::*;
use seeding::{seed_execution_memory, seed_request_memory, seed_request_memory_with_scope};
use support::AiDigestMockGuard;

mod chat;
mod gates;
mod policy;

use chat::evaluate_chat_case;
use gates::{evaluate_business_contract_case, evaluate_recommendation_case};
use policy::{evaluate_execution_memory_policy_case, evaluate_request_memory_policy_case};

pub(super) async fn run_scenario(scenario: &LaunchEvalScenario) -> LaunchEvalCaseResult {
    match scenario {
        LaunchEvalScenario::Chat {
            id,
            description,
            request,
            ai_digest_mock,
            expect,
        } => {
            let digest_guard = if let Some(mock) = ai_digest_mock {
                match AiDigestMockGuard::start(mock).await {
                    Ok(guard) => Some(guard),
                    Err(error) => {
                        return LaunchEvalCaseResult {
                            id: id.clone(),
                            kind: "chat".to_string(),
                            description: description.clone(),
                            passed: false,
                            errors: vec![error.to_string()],
                            notes: Vec::new(),
                            command: None,
                            response_preview: None,
                            readiness: None,
                            admission: None,
                        };
                    }
                }
            } else {
                None
            };
            let _digest_guard = digest_guard;
            evaluate_chat_case(id, "chat", description, request, expect).await
        }
        LaunchEvalScenario::RequestMemoryReuse {
            id,
            description,
            seed,
            request,
            expect,
        } => {
            if let Err(error) = seed_request_memory(seed, request.memory_scope().as_deref()) {
                return LaunchEvalCaseResult {
                    id: id.clone(),
                    kind: "request_memory_reuse".to_string(),
                    description: description.clone(),
                    passed: false,
                    errors: vec![error.to_string()],
                    notes: Vec::new(),
                    command: None,
                    response_preview: None,
                    readiness: None,
                    admission: None,
                };
            }
            evaluate_chat_case(id, "request_memory_reuse", description, request, expect).await
        }
        LaunchEvalScenario::ExecutionMemoryReuse {
            id,
            description,
            seed,
            request,
            expect,
        } => {
            if let Err(error) = seed_execution_memory(seed, request.memory_scope().as_deref()) {
                return LaunchEvalCaseResult {
                    id: id.clone(),
                    kind: "execution_memory_reuse".to_string(),
                    description: description.clone(),
                    passed: false,
                    errors: vec![error.to_string()],
                    notes: Vec::new(),
                    command: None,
                    response_preview: None,
                    readiness: None,
                    admission: None,
                };
            }
            evaluate_chat_case(id, "execution_memory_reuse", description, request, expect).await
        }
        LaunchEvalScenario::RequestMemoryPolicy {
            id,
            description,
            seed,
            request,
            mode,
            feedback,
            suppress,
            expect_cached,
            expect_command,
            expect_response_contains,
        } => evaluate_request_memory_policy_case(
            id,
            description,
            seed,
            request,
            mode,
            feedback.as_deref(),
            *suppress,
            *expect_cached,
            expect_command.as_deref(),
            expect_response_contains,
        ),
        LaunchEvalScenario::ExecutionMemoryPolicy {
            id,
            description,
            seed,
            request,
            lookup_params,
            feedback,
            suppress,
            expect_cached,
            expect_response_contains,
        } => evaluate_execution_memory_policy_case(
            id,
            description,
            seed,
            request,
            lookup_params.as_ref(),
            feedback.as_deref(),
            *suppress,
            *expect_cached,
            expect_response_contains,
        ),
        LaunchEvalScenario::BusinessContract {
            id,
            description,
            plan,
            logs,
            expect_ok,
            expect_detail_contains,
            expect_assertions,
        } => evaluate_business_contract_case(
            id,
            description,
            plan,
            logs,
            *expect_ok,
            expect_detail_contains,
            expect_assertions,
        ),
        LaunchEvalScenario::MemoryScopeIsolation {
            id,
            description,
            seed,
            request,
            expect,
        } => {
            if let Err(error) = seed_request_memory_with_scope(seed) {
                return LaunchEvalCaseResult {
                    id: id.clone(),
                    kind: "memory_scope_isolation".to_string(),
                    description: description.clone(),
                    passed: false,
                    errors: vec![error.to_string()],
                    notes: Vec::new(),
                    command: None,
                    response_preview: None,
                    readiness: None,
                    admission: None,
                };
            }
            evaluate_chat_case(id, "memory_scope_isolation", description, request, expect).await
        }
        LaunchEvalScenario::RecommendationGate {
            id,
            description,
            stage,
            proposal,
            expect_ready,
            expect_reasons_contains,
        } => evaluate_recommendation_case(
            id,
            description,
            stage,
            proposal,
            *expect_ready,
            expect_reasons_contains,
        ),
    }
}
