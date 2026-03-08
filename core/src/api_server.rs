use axum::{
    extract::{Path, Query, State},
    http::{HeaderValue, Method, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower_http::cors::{Any, CorsLayer};

use crate::permission_manager::PermissionManager;
use crate::{
    ai_digest, approval_gate, chat_sanitize, collector_pipeline, consistency_check,
    context_pruning, db, execution_controller, feedback_collector, integrations, intent_router,
    judgment, launch_eval, llm_gateway, monitor, nl_store, pattern_detector,
    performance_verification, plan_builder, project_scanner, quality_scorer,
    recommendation_executor, release_gate, release_readiness, runtime_verification,
    semantic_verification, slot_filler, tool_result_guard, verification_engine,
    visual_verification, workflow_intake,
};
use std::cmp::Ordering as CmpOrdering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use sysinfo::System;

#[derive(Clone)]
pub struct AppState {
    pub llm_client: Option<std::sync::Arc<dyn llm_gateway::LLMClient>>,
    pub current_goal: Arc<Mutex<Option<String>>>,
}

static INFLIGHT_AGENT_EXECUTIONS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static TELEGRAM_LISTENER_STARTED: OnceLock<AtomicBool> = OnceLock::new();
static API_SERVER_STARTED_AT: OnceLock<String> = OnceLock::new();
static INFLIGHT_PROVISION_OPS: OnceLock<Mutex<HashSet<i64>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelegramListenerStartOutcome {
    Started,
    AlreadyRunning,
}

fn inflight_agent_executions() -> &'static Mutex<HashSet<String>> {
    INFLIGHT_AGENT_EXECUTIONS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn telegram_listener_started_flag() -> &'static AtomicBool {
    TELEGRAM_LISTENER_STARTED.get_or_init(|| AtomicBool::new(false))
}

pub fn try_spawn_telegram_listener(
    llm: std::sync::Arc<dyn llm_gateway::LLMClient>,
) -> Result<TelegramListenerStartOutcome, &'static str> {
    let started_flag = telegram_listener_started_flag();
    if started_flag
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(TelegramListenerStartOutcome::AlreadyRunning);
    }

    let bot = match crate::telegram::TelegramBot::from_env(llm, None) {
        Some(v) => v,
        None => {
            started_flag.store(false, Ordering::SeqCst);
            return Err("missing_telegram_token");
        }
    };

    tokio::spawn(async move {
        std::sync::Arc::new(bot).start_polling().await;
        telegram_listener_started_flag().store(false, Ordering::SeqCst);
    });

    Ok(TelegramListenerStartOutcome::Started)
}

fn is_truthy_env_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn has_nonempty_env(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

fn default_server_workdir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn allow_operational_workdir_override() -> bool {
    std::env::var("ALLVIA_API_ALLOW_WORKDIR_OVERRIDE")
        .ok()
        .map(|value| is_truthy_env_value(&value))
        .unwrap_or(false)
}

fn allow_operational_path_override() -> bool {
    allow_operational_workdir_override()
        || std::env::var("ALLVIA_API_ALLOW_PATH_OVERRIDE")
            .ok()
            .map(|value| is_truthy_env_value(&value))
            .unwrap_or(false)
}

fn resolve_operational_workdir(requested: Option<&str>) -> PathBuf {
    if allow_operational_workdir_override() {
        if let Some(raw) = requested.map(str::trim).filter(|value| !value.is_empty()) {
            return PathBuf::from(raw);
        }
    }
    default_server_workdir()
}

fn resolve_operational_file_override(requested: Option<&str>) -> Option<PathBuf> {
    if allow_operational_path_override() {
        if let Some(raw) = requested.map(str::trim).filter(|value| !value.is_empty()) {
            return Some(PathBuf::from(raw));
        }
    }
    None
}

fn requested_operational_override(requested: Option<&str>) -> bool {
    requested
        .map(str::trim)
        .map(|value| !value.is_empty())
        .unwrap_or(false)
}

fn default_launch_eval_config_path() -> PathBuf {
    default_server_workdir().join("configs/launch_eval.yaml")
}

fn has_release_baseline_override(payload: &release_gate::ReleaseBaselineRequest) -> bool {
    payload.workdir.is_some()
        || payload.max_files.is_some()
        || payload.consistency.is_some()
        || payload.semantic.is_some()
        || payload.performance.is_some()
        || payload.quality.is_some()
        || payload.perf_regression_pct.is_some()
        || payload.quality_drop.is_some()
        || payload.launch_error_rate_pct.is_some()
        || payload.launch_low_confidence_rate_pct.is_some()
        || payload.launch_cache_hit_rate_drop_pct.is_some()
        || payload.recommendation_approval_rate_min.is_some()
        || payload.launch_eval_config_path.is_some()
        || payload.launch_eval_report_path.is_some()
        || payload.launch_eval_snapshot_output_path.is_some()
        || payload.refresh_launch_eval_candidates.is_some()
        || payload.launch_eval_candidate_limit.is_some()
}

fn default_release_baseline_request() -> release_gate::ReleaseBaselineRequest {
    release_gate::ReleaseBaselineRequest {
        workdir: None,
        max_files: None,
        consistency: None,
        semantic: None,
        performance: None,
        quality: None,
        perf_regression_pct: None,
        quality_drop: None,
        launch_error_rate_pct: None,
        launch_low_confidence_rate_pct: None,
        launch_cache_hit_rate_drop_pct: None,
        recommendation_approval_rate_min: None,
        launch_eval_config_path: None,
        launch_eval_report_path: None,
        refresh_launch_eval_candidates: Some(true),
        launch_eval_candidate_limit: Some(20),
        launch_eval_snapshot_output_path: None,
    }
}

fn telegram_polling_requested() -> bool {
    match std::env::var("STEER_TELEGRAM_POLLING") {
        Ok(value) => is_truthy_env_value(&value),
        Err(_) => has_nonempty_env("TELEGRAM_BOT_TOKEN"),
    }
}

fn inflight_provision_ops() -> &'static Mutex<HashSet<i64>> {
    INFLIGHT_PROVISION_OPS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn spawn_workflow_provision_recovery_loop(
    llm_client: Option<std::sync::Arc<dyn llm_gateway::LLMClient>>,
) {
    tokio::spawn(async move {
        // Startup kick to recover pending ops from previous process first.
        let _ = db::reconcile_workflow_provision_ops(50);
        loop {
            for status in ["requested", "provisioning"] {
                match db::list_workflow_provision_ops(25, Some(status), None) {
                    Ok(ops) => {
                        for op in ops {
                            let claim_token = match op
                                .claim_token
                                .as_deref()
                                .map(str::trim)
                                .filter(|v| v.starts_with("provisioning:"))
                            {
                                Some(v) => v.to_string(),
                                None => {
                                    let _ = db::mark_workflow_provision_failed(
                                        op.id,
                                        "workflow provisioning op missing valid claim token",
                                    );
                                    continue;
                                }
                            };

                            let should_spawn =
                                if let Ok(mut guard) = inflight_provision_ops().lock() {
                                    guard.insert(op.id)
                                } else {
                                    false
                                };
                            if !should_spawn {
                                continue;
                            }

                            let llm_for_op = llm_client.clone();
                            tokio::spawn(async move {
                                let preclaim = recommendation_executor::PreclaimedProvisioning {
                                    claim_token: Some(claim_token),
                                    provision_op_id: op.id,
                                    force_recreate: false,
                                };
                                if let Err(error) =
                                    recommendation_executor::execute_approved_recommendation_with_preclaim(
                                        op.recommendation_id,
                                        llm_for_op,
                                        preclaim,
                                    )
                                    .await
                                {
                                    eprintln!(
                                        "⚠️ Provision recovery failed: op_id={} recommendation_id={} status={} error={}",
                                        op.id, op.recommendation_id, status, error
                                    );
                                }
                                if let Ok(mut guard) = inflight_provision_ops().lock() {
                                    guard.remove(&op.id);
                                }
                            });
                        }
                    }
                    Err(error) => {
                        eprintln!(
                            "⚠️ Failed to list workflow provision ops (status={}): {}",
                            status, error
                        );
                    }
                }
            }

            let _ = db::reconcile_workflow_provision_ops(50);
            tokio::time::sleep(std::time::Duration::from_secs(8)).await;
        }
    });
}

struct AgentExecutionGuard {
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

fn acquire_agent_execution(
    plan_id: &str,
) -> Result<AgentExecutionGuard, AgentExecutionLockConflict> {
    let scope = agent_execution_lock_scope();
    if let Ok(mut set) = inflight_agent_executions().lock() {
        if scope == "global" {
            if let Some(active) = set.iter().next() {
                return Err(AgentExecutionLockConflict {
                    scope,
                    active_plan_id: Some(active.to_string()),
                });
            }
            set.insert(plan_id.to_string());
            return Ok(AgentExecutionGuard {
                plan_id: plan_id.to_string(),
            });
        }

        if set.contains(plan_id) {
            return Err(AgentExecutionLockConflict {
                scope,
                active_plan_id: Some(plan_id.to_string()),
            });
        }
        set.insert(plan_id.to_string());
        return Ok(AgentExecutionGuard {
            plan_id: plan_id.to_string(),
        });
    }
    Err(AgentExecutionLockConflict {
        scope,
        active_plan_id: None,
    })
}

// Request/Response types
#[derive(Deserialize, Clone)]
pub struct ChatRequest {
    pub message: String,
    pub channel: Option<String>,
    pub chat_type: Option<String>,
    pub sender: Option<String>,
    pub mentioned: Option<bool>,
}

#[derive(Deserialize)]
pub struct FeedbackRequest {
    pub goal: String,
    pub feedback: String,
    pub history_summary: Option<String>,
}

#[derive(Serialize)]
pub struct FeedbackResponse {
    pub action: String,
    pub new_goal: Option<String>,
    pub message: String,
}

#[derive(Deserialize)]
pub struct ChatFeedbackRequest {
    pub request_text: String,
    pub response_text: String,
    pub command: Option<String>,
    pub sentiment: String,
    pub channel: Option<String>,
    pub chat_type: Option<String>,
    pub sender: Option<String>,
}

#[derive(Serialize)]
pub struct ChatFeedbackResponse {
    pub ok: bool,
    pub request_memory_updated: bool,
    pub execution_memory_updated: bool,
    pub reuse_suppressed: bool,
}

#[derive(Deserialize)]
pub struct RecommendationFeedbackRequest {
    pub feedback: String,
    pub actor: Option<String>,
}

#[derive(Deserialize)]
pub struct RecommendationReviewActionRequest {
    pub actor: Option<String>,
    pub note: Option<String>,
}

#[derive(Serialize)]
pub struct RecommendationFeedbackResponse {
    pub ok: bool,
    pub sentiment: String,
    pub status: String,
    pub message: String,
    pub suppressed_similar: bool,
}

#[derive(Deserialize)]
pub struct MemoryRecordsQuery {
    pub limit: Option<i64>,
    pub include_suppressed: Option<bool>,
}

#[derive(Deserialize)]
pub struct RecommendationReviewEventsQuery {
    pub limit: Option<i64>,
}

#[derive(Serialize)]
pub struct MemoryRecordsResponse {
    pub metrics: crate::db::MemoryOpsMetrics,
    pub request_memory: Vec<crate::db::RequestMemoryRecord>,
    pub execution_memory: Vec<crate::db::ExecutionMemoryRecord>,
    pub recent_admin_events: Vec<crate::db::MemoryAdminEventRecord>,
}

#[derive(Deserialize)]
pub struct LaunchOpsQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct LaunchEvalCandidatesQuery {
    pub limit: Option<usize>,
    pub provenance: Option<String>,
    pub workdir: Option<String>,
}

#[derive(Deserialize)]
pub struct LaunchEvalSnapshotRequest {
    pub limit: Option<usize>,
    pub output_path: Option<String>,
    pub workdir: Option<String>,
    pub provenance: Option<String>,
}

#[derive(Deserialize)]
pub struct LaunchEvalSnapshotInfoQuery {
    pub output_path: Option<String>,
    pub workdir: Option<String>,
    pub provenance: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReleaseReadinessRequest {
    pub workdir: Option<String>,
    pub config_path: Option<String>,
    pub candidate_limit: Option<usize>,
    pub snapshot_output_path: Option<String>,
    pub save_baseline: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ReleaseReadinessHistoryQuery {
    pub workdir: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct HttpE2ERequest {
    pub workdir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HttpE2EHistoryQuery {
    pub workdir: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct LaunchOpsResponse {
    pub chat_metrics: crate::db::LaunchOpsMetrics,
    pub memory_metrics: crate::db::MemoryOpsMetrics,
    pub nl_run_metrics: crate::db::NLRunMetrics,
    pub exec_approval_metrics: crate::db::ExecApprovalMetrics,
    pub recommendation_metrics: RecommendationMetricsResponse,
    pub recommendation_review_metrics: crate::db::RecommendationReviewMetrics,
    pub recent_events: Vec<crate::db::LaunchOpsEventRecord>,
}

#[derive(Deserialize)]
pub struct RequestMemoryAdminActionRequest {
    pub request_text: String,
    pub memory_scope: Option<String>,
    pub reason: Option<String>,
    pub actor: Option<String>,
}

#[derive(Deserialize)]
pub struct ExecutionMemoryAdminActionRequest {
    pub intent_command: String,
    pub params_key: String,
    pub memory_scope: Option<String>,
    pub reason: Option<String>,
    pub actor: Option<String>,
}

#[derive(Serialize)]
pub struct MemoryAdminActionResponse {
    pub ok: bool,
    pub kind: String,
    pub action: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct AgentIntentRequest {
    pub text: String,
}

#[derive(Serialize)]
pub struct AgentIntentResponse {
    pub session_id: String,
    pub intent: String,
    pub confidence: f32,
    pub slots: HashMap<String, String>,
    pub missing_slots: Vec<String>,
    pub follow_up: Option<String>,
}

#[derive(Deserialize)]
pub struct AgentPlanRequest {
    pub session_id: String,
    pub slots: Option<HashMap<String, String>>,
}

#[derive(Serialize)]
pub struct AgentPlanResponse {
    pub plan_id: String,
    pub intent: String,
    pub steps: Vec<crate::nl_automation::PlanStep>,
    pub missing_slots: Vec<String>,
}

#[derive(Deserialize)]
pub struct AgentExecuteRequest {
    pub plan_id: String,
    pub profile: Option<AgentExecutionProfile>,
    #[serde(default)]
    pub resume_from: Option<usize>,
    #[serde(default)]
    pub resume_token: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum AgentExecutionProfile {
    #[default]
    Strict,
    Test,
    Fast,
}

impl AgentExecutionProfile {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Test => "test",
            Self::Fast => "fast",
        }
    }

    fn execution_options(&self) -> crate::execution_controller::ExecutionOptions {
        match self {
            Self::Strict => crate::execution_controller::ExecutionOptions::strict(),
            Self::Test => crate::execution_controller::ExecutionOptions::test(),
            Self::Fast => crate::execution_controller::ExecutionOptions::fast(),
        }
    }

    fn default_auto_replan_enabled(&self) -> bool {
        matches!(self, Self::Test)
    }
}

#[derive(Serialize)]
pub struct AgentExecuteResponse {
    pub status: String,
    pub logs: Vec<String>,
    pub approval: Option<crate::nl_automation::ApprovalContext>,
    #[serde(default)]
    pub manual_steps: Vec<String>,
    pub resume_from: Option<usize>,
    #[serde(default)]
    pub resume_token: Option<String>,
    pub run_id: Option<String>,
    #[serde(default)]
    pub planner_complete: bool,
    #[serde(default)]
    pub execution_complete: bool,
    #[serde(default)]
    pub business_complete: bool,
    #[serde(default)]
    pub completion_score: Option<AgentCompletionScore>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub collision_policy: Option<String>,
    #[serde(default)]
    pub stage_dod: Vec<AgentStageDodCheck>,
}

#[derive(Serialize, Clone)]
pub struct AgentCompletionScore {
    pub score: u8,
    pub label: String,
    pub pass: bool,
    pub reasons: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct AgentStageDodCheck {
    pub stage: String,
    pub key: String,
    pub expected: String,
    pub actual: String,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

#[derive(Debug, Clone)]
struct ParsedResumeToken {
    plan_id: String,
    step_index: usize,
    reason: String,
}

fn parse_resume_token(raw: &str) -> Result<ParsedResumeToken, String> {
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

#[derive(Deserialize)]
pub struct AgentVerifyRequest {
    pub plan_id: String,
}

#[derive(Serialize)]
pub struct AgentVerifyResponse {
    pub ok: bool,
    pub issues: Vec<String>,
}

#[derive(Deserialize)]
pub struct AgentApproveRequest {
    pub plan_id: String,
    pub action: String,
    pub decision: Option<String>,
}

#[derive(Serialize)]
pub struct AgentApproveResponse {
    pub status: String,
    pub requires_approval: bool,
    pub message: String,
    pub risk_level: String,
    pub policy: String,
}

#[derive(Serialize)]
pub struct AgentPreflightCheckItem {
    pub key: String,
    pub label: String,
    pub ok: bool,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub message: String,
}

#[derive(Serialize)]
pub struct AgentPreflightResponse {
    pub ok: bool,
    pub checks: Vec<AgentPreflightCheckItem>,
    pub active_app: Option<String>,
    pub checked_at: String,
}

#[derive(Deserialize)]
pub struct AgentPreflightFixRequest {
    pub action: String,
    pub run_id: Option<String>,
    pub stage_name: Option<String>,
    pub assertion_key: Option<String>,
}

#[derive(Serialize)]
pub struct AgentPreflightFixResponse {
    pub ok: bool,
    pub action: String,
    pub message: String,
    pub active_app: Option<String>,
    pub fixed_at: String,
    pub recorded: bool,
    pub run_id: Option<String>,
    pub stage_name: Option<String>,
}

#[derive(Deserialize)]
pub struct AgentRecoveryEventRequest {
    pub run_id: String,
    pub action_key: String,
    pub status: String,
    pub details: Option<String>,
    pub stage_name: Option<String>,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

#[derive(Serialize)]
pub struct AgentRecoveryEventResponse {
    pub ok: bool,
    pub recorded: bool,
    pub run_id: String,
    pub stage_name: String,
    pub action_key: String,
    pub status: String,
    pub recorded_at: String,
    pub reason: Option<String>,
}

#[derive(Deserialize)]
pub struct ExecApprovalQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct ApprovalPolicyQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct NLRunMetricsQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct ApprovalPolicyRequest {
    pub policy_key: String,
    pub decision: String,
}

#[derive(Serialize)]
pub struct ApprovalPolicyResponse {
    pub policy_key: String,
    pub decision: String,
    pub updated_at: String,
}

#[derive(Deserialize, Default)]
pub struct ExecApprovalResolve {
    pub resolved_by: Option<String>,
    pub decision: Option<String>,
}

#[derive(Deserialize)]
pub struct RoutineRunsQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct ExecAllowlistRequest {
    pub pattern: String,
    pub cwd: Option<String>,
}

#[derive(Deserialize)]
pub struct ExecAllowlistQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct ExecResultsQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct VerificationRunsQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct NLRunQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct TaskRunsQuery {
    pub limit: Option<i64>,
    pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct CollectorHandoffReceiptsQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct WorkflowProvisionOpsQuery {
    pub limit: Option<i64>,
    pub status: Option<String>,
    pub recommendation_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct ProjectScanQuery {
    pub max_files: Option<usize>,
    pub workdir: Option<String>,
}

#[derive(Serialize)]
pub struct ProjectScanResponse {
    pub project_type: String,
    pub files: Vec<String>,
    pub key_files: std::collections::HashMap<String, String>,
}

#[derive(Serialize)]
pub struct RuntimeDbPathsResponse {
    pub core_db_path: Option<String>,
    pub collector_db_path: String,
    pub mismatch: bool,
    pub allow_mismatch: bool,
}

#[derive(Serialize)]
pub struct RuntimeInfoResponse {
    pub service: String,
    pub version: String,
    pub profile: String,
    pub pid: u32,
    pub api_port: u16,
    pub allow_no_key: bool,
    pub started_at: String,
    pub binary_path: Option<String>,
    pub current_dir: Option<String>,
}

#[derive(Serialize)]
pub struct LockMetricsResponse {
    pub acquired: u64,
    pub bypassed: u64,
    pub blocked: u64,
    pub stale_recovered: u64,
    pub rejected: u64,
}

#[derive(Deserialize)]
pub struct RuntimeVerifyRequest {
    pub workdir: Option<String>,
    pub run_backend: Option<bool>,
    pub run_frontend: Option<bool>,
    pub run_e2e: Option<bool>,
    pub run_build_checks: Option<bool>,
    pub backend_port: Option<u16>,
    pub frontend_port: Option<u16>,
    pub backend_health_path: Option<String>,
}

#[derive(Deserialize)]
pub struct QualityScoreRequest {
    pub runtime: Option<runtime_verification::RuntimeVerifyResult>,
    pub runtime_options: Option<RuntimeVerifyRequest>,
    pub code_review: Option<quality_scorer::CodeReviewInput>,
    pub goal: Option<String>,
    pub use_llm: Option<bool>,
}

#[derive(Serialize)]
pub struct QualityScoreResponse {
    pub created_at: String,
    pub score: quality_scorer::QualityScore,
}

#[derive(Deserialize)]
pub struct SemanticVerifyRequest {
    pub workdir: Option<String>,
    pub max_files: Option<usize>,
}

#[derive(Deserialize)]
pub struct PerformanceVerifyRequest {
    pub workdir: Option<String>,
    pub max_files: Option<usize>,
}

#[derive(Serialize)]
pub struct ChatRouteMeta {
    pub route_kind: String,
    pub outcome: String,
    pub confidence: Option<f64>,
    pub note: Option<String>,
    pub memory_scope: Option<String>,
    pub freshness_bypassed: bool,
    pub intent_memory_hit: bool,
    pub request_memory_hit: bool,
    pub execution_memory_hit: bool,
    pub deterministic_used: bool,
    pub llm_used: bool,
    pub ai_digest_used: bool,
    pub local_only: bool,
}

#[derive(Serialize)]
pub struct ChatResponse {
    pub response: String,
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_meta: Option<ChatRouteMeta>,
}

#[derive(Deserialize)]
struct AiDigestRunRequest {
    text: Option<String>,
    scope_marker: Option<String>,
}

#[derive(Serialize)]
struct AiDigestRunResponse {
    ok: bool,
    scope_marker: String,
    notion_url: Option<String>,
    webhook_url: String,
    status_code: u16,
    response_text: Option<String>,
}

#[derive(Deserialize)]
struct NewsSummaryRequest {
    title: Option<String>,
    link: Option<String>,
    source: Option<String>,
    #[serde(alias = "pubDate")]
    pub_date: Option<String>,
    description: Option<String>,
    scope_marker: Option<String>,
    telegram_chat_id: Option<String>,
    article_count: Option<usize>,
    topic: Option<String>,
}

#[derive(Serialize)]
struct NewsSummaryResponse {
    ok: bool,
    summary: Value,
}

#[derive(Serialize)]
pub struct SystemStatus {
    pub cpu_usage: f32,
    pub memory_used: u64,
    pub memory_total: u64,
}

#[derive(Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct RecommendationItem {
    pub id: i64,
    pub status: String, // [NEW] Status field
    pub title: String,
    pub summary: String,
    pub confidence: f64,
    pub category: String,
    pub business_score: f64,
    pub evidence: Vec<String>, // [NEW] Explainability field
    pub last_error: Option<String>,
    pub workflow_id: Option<String>,
    pub workflow_url: Option<String>,
    pub snoozed_until: Option<String>,
    pub approval_ready: bool,
    pub approval_reasons: Vec<String>,
}

#[derive(Serialize)]
pub struct QualityMetrics {
    pub total: u32,
    pub success: u32,
    pub rate: f64,
}

/// Start the HTTP API server for desktop GUI
// Middleware for API Key Authentication
async fn auth_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    if req.method() == Method::OPTIONS {
        return Ok(next.run(req).await);
    }

    let api_key = std::env::var("STEER_API_KEY").unwrap_or_default();
    let request_path = req.uri().path().to_string();
    let request_method = req.method().to_string();

    // Require explicit opt-in for no-key local development mode.
    if api_key.is_empty() {
        let allow_no_key = crate::env_flag("STEER_API_ALLOW_NO_KEY");
        if allow_no_key {
            // RELAXED AUTH FOR LOCAL DEMO: Always allow if allow_no_key is set.
            // The previous strict Host/Origin checks were blocking valid local requests from Tauri/n8n.
            return Ok(next.run(req).await);

            /*
            let host_header = req
                .headers()
                .get("host")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("")
                .to_lowercase();
            // ... (rest of the checks commented out)
            */
        } else {
            crate::diagnostic_events::emit(
                "api.auth.denied",
                serde_json::json!({
                    "reason": "no_key_mode_not_allowed",
                    "path": request_path,
                    "method": request_method
                }),
            );
        }
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Check Authorization header first, then fallback to X-API-Key.
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string());
    let x_api_key = req
        .headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.trim().to_string());

    match auth_header.or(x_api_key) {
        Some(key) if key == api_key => Ok(next.run(req).await),
        _ => {
            crate::diagnostic_events::emit(
                "api.auth.denied",
                serde_json::json!({
                    "reason": "api_key_mismatch_or_missing",
                    "path": request_path,
                    "method": request_method
                }),
            );
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

fn build_api_cors_layer() -> CorsLayer {
    let allowed_origins = [
        "http://localhost:5173"
            .parse::<HeaderValue>()
            .expect("Invalid CORS origin"),
        "http://localhost:5174"
            .parse::<HeaderValue>()
            .expect("Invalid CORS origin"),
        "http://localhost:5680"
            .parse::<HeaderValue>()
            .expect("Invalid CORS origin"),
        "tauri://localhost"
            .parse::<HeaderValue>()
            .expect("Invalid CORS origin"),
        "http://127.0.0.1:5173"
            .parse::<HeaderValue>()
            .expect("Invalid CORS origin"),
        "http://127.0.0.1:5174"
            .parse::<HeaderValue>()
            .expect("Invalid CORS origin"),
    ];

    CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods(Any)
        .allow_headers(Any)
}

pub fn build_api_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(root_handler))
        .route("/api/health", get(health_check))
        // Open endpoints (Status, Logs)
        .route("/api/status", get(get_system_status))
        .route("/api/logs", get(get_recent_logs))
        .route("/api/system/health", get(get_system_health))
        // Protected Endpoints (Chat, Execute, Plan, Verify)
        .route("/events", post(ingest_events))
        .route("/api/chat", post(handle_chat))
        .route("/api/chat/feedback", post(handle_chat_feedback))
        .route("/api/automation/ai-digest", post(run_ai_digest_handler))
        .route("/api/llm/news-summary", post(run_news_summary_handler))
        .route("/api/recommendations", get(list_recommendations))
        .route(
            "/api/recommendations/:id/approve",
            post(approve_recommendation),
        )
        .route(
            "/api/recommendations/:id/reject",
            post(reject_recommendation),
        )
        .route(
            "/api/recommendations/:id/feedback",
            post(submit_recommendation_feedback),
        )
        .route("/api/recommendations/:id/later", post(later_recommendation))
        .route(
            "/api/recommendations/:id/restore",
            post(restore_recommendation),
        )
        .route("/api/exec-approvals", get(list_exec_approvals))
        .route(
            "/api/exec-approvals/:id/approve",
            post(approve_exec_approval),
        )
        .route("/api/exec-approvals/:id/reject", post(reject_exec_approval))
        .route(
            "/api/exec-allowlist",
            get(list_exec_allowlist).post(add_exec_allowlist),
        )
        .route(
            "/api/exec-allowlist/:id",
            axum::routing::delete(remove_exec_allowlist),
        )
        .route("/api/exec-results", get(list_exec_results))
        .route("/api/project/scan", get(scan_project_handler))
        .route(
            "/api/verify/runtime",
            post(run_runtime_verification_handler),
        )
        .route("/api/verify/visual", post(run_visual_verification_handler))
        .route(
            "/api/verify/semantic",
            post(run_semantic_verification_handler),
        )
        .route(
            "/api/verify/performance",
            post(run_performance_verification_handler),
        )
        .route(
            "/api/verify/consistency",
            post(run_consistency_verification_handler),
        )
        .route("/api/verify/runs", get(list_verification_runs))
        .route("/api/judgment", post(run_judgment_handler))
        .route("/api/release/baseline", post(set_release_baseline_handler))
        .route("/api/release/gate", post(run_release_gate_handler))
        .route(
            "/api/release/readiness",
            get(get_latest_release_readiness_handler).post(run_release_readiness_handler),
        )
        .route(
            "/api/release/readiness/history",
            get(get_release_readiness_history_handler),
        )
        .route("/api/http-e2e/latest", get(get_latest_http_e2e_handler))
        .route("/api/http-e2e/history", get(get_http_e2e_history_handler))
        .route("/api/http-e2e/run", post(run_http_e2e_handler))
        .route(
            "/api/exec-results/guard",
            post(run_exec_results_guard_handler),
        )
        .route("/api/quality/score", post(score_quality_handler))
        .route("/api/quality/latest", get(latest_quality_handler))
        .route("/api/patterns/analyze", post(analyze_patterns))
        .route("/api/quality", get(get_quality_metrics))
        .route("/api/launch/ops", get(get_launch_ops_handler))
        .route(
            "/api/launch/eval-candidates",
            get(get_launch_eval_candidates_handler),
        )
        .route(
            "/api/launch/eval-candidates/snapshot",
            get(get_launch_eval_candidate_snapshot_info_handler)
                .post(write_launch_eval_candidate_snapshot_handler),
        )
        .route(
            "/api/recommendations/metrics",
            get(get_recommendation_metrics),
        )
        .route(
            "/api/recommendations/review-events",
            get(list_recommendation_review_events_handler),
        )
        .route("/api/memory/records", get(list_memory_records_handler))
        .route(
            "/api/memory/request/suppress",
            post(suppress_request_memory_handler),
        )
        .route(
            "/api/memory/request/restore",
            post(restore_request_memory_handler),
        )
        .route(
            "/api/memory/request/delete",
            post(delete_request_memory_handler),
        )
        .route(
            "/api/memory/execution/suppress",
            post(suppress_execution_memory_handler),
        )
        .route(
            "/api/memory/execution/restore",
            post(restore_execution_memory_handler),
        )
        .route(
            "/api/memory/execution/delete",
            post(delete_execution_memory_handler),
        )
        .route(
            "/api/routines",
            get(list_routines).post(create_routine_handler),
        )
        .route(
            "/api/collector/handoff/ingest",
            post(ingest_collector_handoff_handler),
        )
        .route(
            "/api/collector/handoff/receipts",
            get(list_collector_handoff_receipts_handler),
        )
        .route("/api/system/db-paths", get(runtime_db_paths_handler))
        .route("/api/system/runtime-info", get(runtime_info_handler))
        .route("/api/system/lock-metrics", get(lock_metrics_handler))
        .route(
            "/api/workflow/provision-ops",
            get(list_workflow_provision_ops_handler),
        )
        .route(
            "/api/routines/:id",
            axum::routing::patch(toggle_routine_handler),
        )
        .route("/api/routine-runs", get(list_routine_runs))
        .route("/api/agent/intent", post(agent_intent_handler))
        .route("/api/agent/plan", post(agent_plan_handler))
        .route("/api/agent/execute", post(agent_execute_handler))
        .route("/api/agent/verify", post(agent_verify_handler))
        .route("/api/agent/approve", post(agent_approve_handler))
        .route("/api/agent/preflight", get(agent_preflight_handler))
        .route(
            "/api/agent/preflight/fix",
            post(agent_preflight_fix_handler),
        )
        .route(
            "/api/agent/recovery-event",
            post(agent_recovery_event_handler),
        )
        .route("/api/agent/nl-runs", get(list_nl_runs_handler))
        .route("/api/agent/nl-metrics", get(nl_run_metrics_handler))
        .route("/api/agent/task-runs", get(list_task_runs_handler))
        .route("/api/agent/task-runs/:run_id", get(get_task_run_handler))
        .route(
            "/api/agent/task-runs/:run_id/stages",
            get(list_task_stage_runs_handler),
        )
        .route(
            "/api/agent/task-runs/:run_id/assertions",
            get(list_task_stage_assertions_handler),
        )
        .route(
            "/api/agent/task-runs/:run_id/artifacts",
            get(list_task_run_artifacts_handler),
        )
        .route(
            "/api/agent/approval-policies",
            get(list_nl_approval_policies).post(set_nl_approval_policy),
        )
        .route(
            "/api/agent/approval-policies/:key",
            axum::routing::delete(remove_nl_approval_policy),
        )
        .route("/api/agent/goal", post(execute_goal_handler))
        .route("/api/agent/goal/run", post(run_goal_sync_handler))
        .route("/api/agent/goal/current", get(get_current_goal))
        .route("/api/agent/feedback", post(handle_feedback))
        .route("/api/context/selection", get(get_selection_context))
        // Session Management (Clawdbot-ported)
        .route("/api/sessions", get(list_sessions_handler))
        .route(
            "/api/sessions/:id",
            get(get_session_handler).delete(delete_session_handler),
        )
        .route("/api/sessions/:id/resume", post(resume_session_handler))
        .layer(axum::middleware::from_fn(auth_middleware))
        .layer(build_api_cors_layer())
        .with_state(state)
}

/// Start the HTTP API server for desktop GUI
pub async fn start_api_server(
    llm_client: Option<std::sync::Arc<dyn llm_gateway::LLMClient>>,
) -> anyhow::Result<()> {
    API_SERVER_STARTED_AT.get_or_init(|| chrono::Utc::now().to_rfc3339());

    let telegram_polling_explicit = std::env::var("STEER_TELEGRAM_POLLING").ok();
    if telegram_polling_requested() {
        if let Some(llm) = llm_client.clone() {
            match try_spawn_telegram_listener(llm) {
                Ok(TelegramListenerStartOutcome::Started) => {
                    if telegram_polling_explicit.is_some() {
                        println!("🤖 Telegram polling enabled (STEER_TELEGRAM_POLLING=1).");
                    } else {
                        println!(
                            "🤖 Telegram polling auto-enabled (Telegram credentials detected)."
                        );
                    }
                }
                Ok(TelegramListenerStartOutcome::AlreadyRunning) => {
                    println!("ℹ️ Telegram listener already running.");
                }
                Err("missing_telegram_token") => {
                    println!(
                        "⚠️  STEER_TELEGRAM_POLLING=1 but TELEGRAM_BOT_TOKEN is missing; listener not started."
                    );
                }
                Err(_) => {}
            }
        } else {
            println!("⚠️  STEER_TELEGRAM_POLLING=1 but LLM is unavailable; listener not started.");
        }
    }

    match db::mark_orphaned_inflight_task_runs_failed() {
        Ok(recovered) if recovered > 0 => {
            println!(
                "♻️ Recovered {} orphaned in-flight task run(s) from previous core process.",
                recovered
            );
        }
        Ok(_) => {}
        Err(e) => {
            println!("⚠️ Failed to recover orphaned task runs: {}", e);
        }
    }

    let state = AppState {
        llm_client,
        current_goal: Arc::new(Mutex::new(None)),
    };

    // Recover/resume in-flight workflow provisioning across core restarts.
    spawn_workflow_provision_recovery_loop(state.llm_client.clone());
    let app = build_api_router(state);

    let port = std::env::var("STEER_API_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(5680);
    println!("🌐 Desktop API server running on http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to bind port {}: {}", port, e))?;

    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;
    Ok(())
}

async fn root_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "online",
        "service": "AllvIa Core API",
        "version": "v0.1.0",
        "ui_url": "http://localhost:5174",
        "docs": "/api/health"
    }))
}

async fn health_check() -> &'static str {
    "ok"
}

async fn get_system_status() -> Json<SystemStatus> {
    let mut sys = System::new_all();
    sys.refresh_cpu(); // First refresh just gathers data
    tokio::time::sleep(std::time::Duration::from_millis(200)).await; // Non-blocking wait for CPU delta
    sys.refresh_cpu(); // Second refresh calculates usage
    sys.refresh_memory();

    let cpu_usage = sys.global_cpu_info().cpu_usage();
    let memory_used = sys.used_memory() as f32 / 1024.0 / 1024.0; // MB
    let memory_total = sys.total_memory() as f32 / 1024.0 / 1024.0; // MB

    Json(SystemStatus {
        cpu_usage,
        memory_used: memory_used as u64,
        memory_total: memory_total as u64,
    })
}

fn truncate_log_message(raw: &str, max_chars: usize) -> String {
    if raw.chars().count() <= max_chars {
        return raw.to_string();
    }
    let mut out = String::new();
    for ch in raw.chars().take(max_chars.saturating_sub(1)) {
        out.push(ch);
    }
    out.push('…');
    out
}

async fn get_recent_logs() -> Json<Vec<LogEntry>> {
    let mut logs: Vec<LogEntry> = Vec::new();

    if let Ok(task_runs) = crate::db::list_task_runs(40, None) {
        for run in task_runs {
            let level = match run.status.as_str() {
                "failed" | "blocked" | "error" => "ERROR",
                "manual_required" | "approval_required" => "WARN",
                _ => "INFO",
            };
            let summary = run.summary.unwrap_or_else(|| "-".to_string());
            let message = format!(
                "task_run status={} intent={} run_id={} summary={}",
                run.status,
                run.intent,
                run.run_id,
                truncate_log_message(&summary, 180)
            );
            logs.push(LogEntry {
                timestamp: run.created_at,
                level: level.to_string(),
                message,
            });
        }
    }

    if let Ok(verification_runs) = crate::db::list_verification_runs(20) {
        for verification in verification_runs {
            logs.push(LogEntry {
                timestamp: verification.created_at,
                level: if verification.ok { "INFO" } else { "WARN" }.to_string(),
                message: truncate_log_message(
                    &format!(
                        "verification kind={} ok={} summary={}",
                        verification.kind, verification.ok, verification.summary
                    ),
                    200,
                ),
            });
        }
    }

    logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    logs.truncate(80);
    Json(logs)
}

async fn get_system_health() -> Json<crate::dependency_check::SystemHealth> {
    let health = crate::dependency_check::SystemHealth::check_all();
    Json(health)
}

fn run_osascript_inline(script: &str) -> Result<String, String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(out)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn open_system_settings_url(url: &str) -> Result<(), String> {
    Command::new("open")
        .arg(url)
        .status()
        .map_err(|e| e.to_string())
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(format!("open command failed for {}", url))
            }
        })
}

fn reveal_path_in_finder(path: &std::path::Path) -> Result<(), String> {
    Command::new("open")
        .arg("-R")
        .arg(path)
        .status()
        .map_err(|e| e.to_string())
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err("open -R failed".to_string())
            }
        })
}

fn escape_applescript_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn parse_primary_mail_recipient(raw: &str) -> Option<String> {
    let is_email_char =
        |ch: char| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '%' | '+' | '-' | '@');

    let scrub_segment = |segment: &str| -> Option<String> {
        let trimmed = segment
            .trim()
            .trim_matches('<')
            .trim_matches('>')
            .trim_matches('"')
            .trim_matches('\'');
        if trimmed.is_empty() {
            return None;
        }

        let mut cleaned = String::new();
        let mut started = false;
        for ch in trimmed.chars() {
            if is_email_char(ch) {
                cleaned.push(ch);
                started = true;
            } else if started {
                break;
            }
        }

        let normalized = cleaned
            .trim_end_matches(['.', ',', ';', ':', ')', '('])
            .to_ascii_lowercase();
        if normalized.contains('@') && normalized.contains('.') {
            Some(normalized)
        } else {
            None
        }
    };

    raw.split(|ch: char| ch.is_whitespace() || ch == ',' || ch == ';')
        .filter_map(scrub_segment)
        .next()
}

fn resolve_mail_recipient_for_recovery(run_id: Option<&str>) -> Option<String> {
    if let Some(id) = run_id {
        if let Ok(Some(run)) = db::get_task_run(id) {
            for candidate in crate::semantic_contract::extract_expected_recipients(&run.prompt) {
                if let Some(parsed) = parse_primary_mail_recipient(&candidate) {
                    return Some(parsed);
                }
            }
        }
    }
    std::env::var("STEER_DEFAULT_MAIL_TO")
        .ok()
        .and_then(|v| parse_primary_mail_recipient(&v))
}

fn mail_fill_default_recipient(recipient: &str) -> Result<String, String> {
    let recipient_escaped = escape_applescript_string(recipient);
    let script = format!(
        "tell application \"Mail\"\n\
            activate\n\
            if (count of outgoing messages) = 0 then return \"NO_OUTGOING\"\n\
            set _msg to (last outgoing message)\n\
            set _hasRecipient to false\n\
            try\n\
                if (count of to recipients of _msg) > 0 then\n\
                    set _first to address of first to recipient of _msg as text\n\
                    if _first is not \"\" then set _hasRecipient to true\n\
                end if\n\
            end try\n\
            if _hasRecipient is false then\n\
                make new to recipient at end of to recipients of _msg with properties {{address:\"{}\"}}\n\
            end if\n\
            set visible of _msg to true\n\
            set _draftId to \"\"\n\
            try\n\
                set _draftId to id of _msg as text\n\
            end try\n\
            return \"OK|\" & _draftId\n\
        end tell",
        recipient_escaped
    );
    run_osascript_inline(&script)
}

fn mail_cleanup_outgoing_windows() -> Result<String, String> {
    let script = "tell application \"Mail\"\n\
        activate\n\
        set _count to (count of outgoing messages)\n\
        if _count = 0 then return \"NO_OUTGOING|0\"\n\
        repeat with _msg in outgoing messages\n\
            try\n\
                set visible of _msg to false\n\
            end try\n\
        end repeat\n\
        return \"OK|\" & (_count as text)\n\
    end tell";
    run_osascript_inline(script)
}

fn textedit_save_front_document() -> Result<String, String> {
    let script = "tell application \"TextEdit\"\n\
        activate\n\
        if (count of documents) = 0 then return \"NO_DOCUMENT\"\n\
        set _doc to front document\n\
        save _doc\n\
        set _docId to \"\"\n\
        try\n\
            set _docId to id of _doc as text\n\
        end try\n\
        return \"OK|\" & _docId\n\
    end tell";
    run_osascript_inline(script)
}

fn env_truthy_default(name: &str, default_value: bool) -> bool {
    match std::env::var(name) {
        Ok(raw) => matches!(raw.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"),
        Err(_) => default_value,
    }
}

fn preflight_focus_mode() -> String {
    std::env::var("STEER_PREFLIGHT_FOCUS_MODE")
        .ok()
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "passive".to_string())
}

fn preflight_accessibility_snapshot_probe() -> Result<String, String> {
    // Keep preflight probe lightweight and stable: avoid native AX snapshot
    // path here because it can crash in some runtime setups.
    let script = r#"
tell application "System Events"
    set frontProc to first application process whose frontmost is true
    set appName to name of frontProc
    set winName to ""
    try
        if (count of windows of frontProc) > 0 then
            set winName to name of window 1 of frontProc
        end if
    end try
    if winName is missing value then set winName to ""
    return appName & " :: " & winName
end tell
"#;
    run_osascript_inline(script)
}

async fn agent_preflight_handler() -> Json<AgentPreflightResponse> {
    let mut checks: Vec<AgentPreflightCheckItem> = Vec::new();
    let mut all_ok = true;
    let mut active_app: Option<String> = None;

    let accessibility = run_osascript_inline(
        "tell application \"System Events\" to return name of first application process",
    );
    match accessibility {
        Ok(name) => checks.push(AgentPreflightCheckItem {
            key: "accessibility".to_string(),
            label: "Accessibility".to_string(),
            ok: true,
            expected: None,
            actual: Some(name),
            message: "Accessibility permission available".to_string(),
        }),
        Err(err) => {
            all_ok = false;
            checks.push(AgentPreflightCheckItem {
                key: "accessibility".to_string(),
                label: "Accessibility".to_string(),
                ok: false,
                expected: None,
                actual: None,
                message: format!("Accessibility unavailable: {}", err),
            });
        }
    }

    if env_truthy_default("STEER_PREFLIGHT_AX_SNAPSHOT", true) {
        match preflight_accessibility_snapshot_probe() {
            Ok(actual) => {
                checks.push(AgentPreflightCheckItem {
                    key: "accessibility_snapshot".to_string(),
                    label: "Accessibility Snapshot".to_string(),
                    ok: true,
                    expected: Some("focused app + focused window".to_string()),
                    actual: Some(actual),
                    message: "Accessibility snapshot ready (osascript probe)".to_string(),
                });
            }
            Err(err) => {
                all_ok = false;
                checks.push(AgentPreflightCheckItem {
                    key: "accessibility_snapshot".to_string(),
                    label: "Accessibility Snapshot".to_string(),
                    ok: false,
                    expected: Some("focused app + focused window".to_string()),
                    actual: None,
                    message: format!("Accessibility snapshot blocked: {}", err),
                });
            }
        }
    } else {
        checks.push(AgentPreflightCheckItem {
            key: "accessibility_snapshot".to_string(),
            label: "Accessibility Snapshot".to_string(),
            ok: true,
            expected: Some("focused app + focused window".to_string()),
            actual: Some("skipped".to_string()),
            message: "Snapshot check disabled by env".to_string(),
        });
    }

    if env_truthy_default("STEER_PREFLIGHT_SCREEN_CAPTURE", true) {
        let mut is_granted = PermissionManager::check_screen_recording();
        let shot_path = format!("/tmp/steer_agent_preflight_{}.png", std::process::id());

        if is_granted {
            // Permission is known to be granted natively.
            // We can optionally do a quick shot to be absolutely sure the binary can capture.
            let _ = Command::new("screencapture")
                .args(["-x", shot_path.as_str()])
                .status();
            let _ = fs::remove_file(&shot_path);

            checks.push(AgentPreflightCheckItem {
                key: "screen_capture".to_string(),
                label: "Screen Capture".to_string(),
                ok: true,
                expected: None,
                actual: Some("ok".to_string()),
                message: "Screen capture permission available".to_string(),
            });
        } else {
            // Attempt to request screen recording permission as a fallback
            let requested = PermissionManager::request_screen_recording();
            if requested {
                // Retry native check after requesting permission
                is_granted = PermissionManager::check_screen_recording();
                if is_granted {
                    let _ = Command::new("screencapture")
                        .args(["-x", shot_path.as_str()])
                        .status();
                    let _ = fs::remove_file(&shot_path);

                    checks.push(AgentPreflightCheckItem {
                        key: "screen_capture".to_string(),
                        label: "Screen Capture".to_string(),
                        ok: true,
                        expected: None,
                        actual: Some("ok (after request)".to_string()),
                        message: "Screen capture permission granted after request".to_string(),
                    });
                } else {
                    all_ok = false;
                    let exe_hint = std::env::current_exe()
                        .ok()
                        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
                    checks.push(AgentPreflightCheckItem {
                        key: "screen_capture".to_string(),
                        label: "Screen Capture".to_string(),
                        ok: false,
                        expected: None,
                        actual: exe_hint,
                        message: "화면 캡처 불가: 코어 프로세스(local_os_agent)에 '화면 기록' 권한이 필요합니다. 설정에서 코어 바이너리를 추가(+), 해당 프로세스를 재시작하세요.".to_string(),
                    });
                }
            } else {
                all_ok = false;
                let exe_hint = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
                checks.push(AgentPreflightCheckItem {
                    key: "screen_capture".to_string(),
                    label: "Screen Capture".to_string(),
                    ok: false,
                    expected: None,
                    actual: exe_hint,
                    message: "화면 캡처 불가: 코어 프로세스(local_os_agent)에 '화면 기록' 권한이 필요합니다. 설정에서 코어 바이너리를 추가(+), 해당 프로세스를 재시작하세요.".to_string(),
                });
            }
        }
    } else {
        let skip_allowed = env_truthy_default("STEER_TEST_MODE", false)
            || env_truthy_default("STEER_ALLOW_SCREEN_CAPTURE_SKIP", false);
        if !skip_allowed {
            all_ok = false;
            checks.push(AgentPreflightCheckItem {
                key: "screen_capture".to_string(),
                label: "Screen Capture".to_string(),
                ok: false,
                expected: Some("STEER_PREFLIGHT_SCREEN_CAPTURE=1 (default)".to_string()),
                actual: Some("disabled_by_env".to_string()),
                message:
                    "화면 캡처 체크가 비활성화되어 있어 실행 신뢰성을 보장할 수 없습니다. STEER_PREFLIGHT_SCREEN_CAPTURE=1로 복구하거나 테스트 모드에서만 skip 하세요."
                        .to_string(),
            });
        } else {
            checks.push(AgentPreflightCheckItem {
                key: "screen_capture".to_string(),
                label: "Screen Capture".to_string(),
                ok: true,
                expected: None,
                actual: Some("skipped".to_string()),
                message: "Screen capture check skipped by env (test/allowlist mode only)."
                    .to_string(),
            });
        }
    }

    if env_truthy_default("STEER_PREFLIGHT_FOCUS_HANDOFF", true) {
        let focus_mode = preflight_focus_mode();
        let front_res = run_osascript_inline(
            "tell application \"System Events\" to return name of first application process whose frontmost is true",
        );
        let (focus_ok, focus_actual, focus_msg) = if focus_mode == "active" {
            let activate_res = run_osascript_inline("tell application \"Finder\" to activate");
            match (activate_res, front_res) {
                (Ok(_), Ok(front)) => {
                    active_app = Some(front.clone());
                    if front == "Finder" {
                        (
                            true,
                            Some(front),
                            "Focus handoff ready (active mode, frontmost=Finder)".to_string(),
                        )
                    } else {
                        (
                            false,
                            Some(front.clone()),
                            format!("Focus handoff blocked (active mode, frontmost={})", front),
                        )
                    }
                }
                (_, Err(err)) => (false, None, format!("Focus handoff check failed: {}", err)),
                (Err(err), _) => (
                    false,
                    None,
                    format!("Focus handoff activate failed: {}", err),
                ),
            }
        } else {
            match front_res {
                Ok(front) => {
                    active_app = Some(front.clone());
                    if front == "Finder" {
                        (
                            true,
                            Some(front),
                            "Focus handoff ready (passive mode, frontmost=Finder)".to_string(),
                        )
                    } else {
                        (
                            true,
                            Some(front.clone()),
                            format!(
                                "Focus handoff passive check only (frontmost={}; recommended=Finder)",
                                front
                            ),
                        )
                    }
                }
                Err(err) => (false, None, format!("Focus handoff check failed: {}", err)),
            }
        };
        if !focus_ok {
            all_ok = false;
        }
        checks.push(AgentPreflightCheckItem {
            key: "focus_handoff".to_string(),
            label: "Focus Handoff".to_string(),
            ok: focus_ok,
            expected: Some(if focus_mode == "active" {
                "Finder (required)".to_string()
            } else {
                "Finder (recommended)".to_string()
            }),
            actual: focus_actual,
            message: focus_msg,
        });
    } else {
        checks.push(AgentPreflightCheckItem {
            key: "focus_handoff".to_string(),
            label: "Focus Handoff".to_string(),
            ok: true,
            expected: Some("Finder".to_string()),
            actual: Some("skipped".to_string()),
            message: "Focus handoff check disabled by env".to_string(),
        });
    }

    if active_app.is_none() {
        if let Ok(front) = run_osascript_inline(
            "tell application \"System Events\" to return name of first application process whose frontmost is true",
        ) {
            active_app = Some(front);
        }
    }

    Json(AgentPreflightResponse {
        ok: all_ok,
        checks,
        active_app,
        checked_at: chrono::Utc::now().to_rfc3339(),
    })
}

async fn agent_preflight_fix_handler(
    Json(payload): Json<AgentPreflightFixRequest>,
) -> impl IntoResponse {
    let action = payload.action.trim().to_string();
    if action.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "missing_action" })),
        )
            .into_response();
    }

    let run_id = payload
        .run_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string());
    let stage_name = payload
        .stage_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("recovery")
        .to_string();
    let assertion_key = payload
        .assertion_key
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .unwrap_or_else(|| format!("recovery.preflight.{}", action.replace(' ', "_")));

    let fix_result: Result<String, String> = match action.as_str() {
        "activate_finder" => run_osascript_inline("tell application \"Finder\" to activate")
            .map(|_| "Finder를 전면으로 전환했습니다. 다시 점검을 실행하세요.".to_string()),
        "activate_mail" => run_osascript_inline("tell application \"Mail\" to activate")
            .map(|_| "Mail을 전면으로 전환했습니다.".to_string()),
        "activate_notes" => run_osascript_inline("tell application \"Notes\" to activate")
            .map(|_| "Notes를 전면으로 전환했습니다.".to_string()),
        "activate_textedit" => run_osascript_inline("tell application \"TextEdit\" to activate")
            .map(|_| "TextEdit를 전면으로 전환했습니다.".to_string()),
        "prepare_isolated_mode" => run_osascript_inline(
            "tell application \"Finder\" to activate\n\
             delay 0.1\n\
             tell application \"System Events\" to keystroke \"h\" using {command down, option down}\n\
             delay 0.1\n\
             tell application \"Finder\" to activate",
        )
        .map(|_| {
            "격리 실행 모드를 준비했습니다(다른 앱 숨김 + Finder 전면). 실행 중 키보드/마우스 입력을 피하세요."
                .to_string()
        }),
        "open_accessibility_settings" => open_system_settings_url(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        )
        .map(|_| "접근성 권한 설정 화면을 열었습니다.".to_string()),
        "open_screen_capture_settings" => open_system_settings_url(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
        )
        .map(|_| "화면 기록 권한 설정 화면을 열었습니다.".to_string()),
        "request_screen_capture_access" => {
            let ok = PermissionManager::request_screen_recording();
            if ok {
                Ok("화면 기록 권한을 요청했습니다(프롬프트가 떴으면 허용 후 재시작).".to_string())
            } else {
                Ok("화면 기록 권한이 아직 없습니다. 설정 화면에서 코어 바이너리를 추가(+)한 뒤 재시작하세요.".to_string())
            }
        }
        "request_accessibility_access" => {
            let ok = PermissionManager::request_accessibility();
            if ok {
                Ok("접근성 권한을 요청했습니다(프롬프트가 떴으면 허용 후 재시작).".to_string())
            } else {
                Ok("접근성 권한이 아직 없습니다. 설정 화면에서 코어 바이너리를 추가(+)한 뒤 재시작하세요.".to_string())
            }
        }
        "reveal_core_binary" => std::env::current_exe()
            .map_err(|e| e.to_string())
            .and_then(|p| reveal_path_in_finder(&p).map(|_| p))
            .map(|p| format!("코어 바이너리를 Finder에서 표시했습니다: {}", p.to_string_lossy())),
        "open_input_monitoring_settings" => open_system_settings_url(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
        )
        .map(|_| "입력 모니터링 권한 설정 화면을 열었습니다.".to_string()),
        "mail_fill_default_recipient" => resolve_mail_recipient_for_recovery(run_id.as_deref())
            .ok_or_else(|| {
                "수신자 후보를 찾지 못했습니다(run prompt/STEER_DEFAULT_MAIL_TO 확인 필요)"
                    .to_string()
            })
            .and_then(|recipient| {
                mail_fill_default_recipient(&recipient).map(|result| {
                    if result.starts_with("NO_OUTGOING") {
                        "Mail의 outgoing message가 없어 수신자를 채우지 못했습니다.".to_string()
                    } else {
                        format!("Mail 수신자를 기본값({})으로 보강했습니다 ({})", recipient, result)
                    }
                })
            }),
        "mail_cleanup_outgoing_windows" => mail_cleanup_outgoing_windows().map(|result| {
            if result.starts_with("NO_OUTGOING") {
                "Mail outgoing 초안이 없어 정리할 항목이 없습니다.".to_string()
            } else {
                format!("Mail outgoing 초안 창을 정리했습니다 ({})", result)
            }
        }),
        "textedit_save_front_document" => textedit_save_front_document().map(|result| {
            if result.starts_with("NO_DOCUMENT") {
                "TextEdit 문서가 없어 저장하지 못했습니다.".to_string()
            } else {
                format!("TextEdit front document 저장을 실행했습니다 ({})", result)
            }
        }),
        _ => Err(format!("unsupported_action: {}", action)),
    };

    let active_app = run_osascript_inline(
        "tell application \"System Events\" to return name of first application process whose frontmost is true",
    )
    .ok();

    let persist_for_fix = |status: &str, details: &str| -> bool {
        let Some(id) = run_id.as_deref() else {
            return false;
        };
        persist_recovery_event(
            id,
            &stage_name,
            &assertion_key,
            status,
            Some("completed"),
            Some(status),
            Some(details),
        )
        .unwrap_or(false)
    };

    match fix_result {
        Ok(message) => {
            let evidence = if let Some(front) = active_app.as_deref() {
                format!("{} (front={})", message, front)
            } else {
                message.clone()
            };
            let recorded = persist_for_fix("completed", &evidence);
            (
                StatusCode::OK,
                Json(json!(AgentPreflightFixResponse {
                    ok: true,
                    action,
                    message,
                    active_app,
                    fixed_at: chrono::Utc::now().to_rfc3339(),
                    recorded,
                    run_id,
                    stage_name: Some(stage_name),
                })),
            )
                .into_response()
        }
        Err(err) => {
            let evidence = if let Some(front) = active_app.as_deref() {
                format!("{} (front={})", err, front)
            } else {
                err.clone()
            };
            let recorded = persist_for_fix("failed", &evidence);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "ok": false,
                    "action": action,
                    "error": err,
                    "active_app": active_app,
                    "fixed_at": chrono::Utc::now().to_rfc3339(),
                    "recorded": recorded,
                    "run_id": run_id,
                    "stage_name": stage_name,
                })),
            )
                .into_response()
        }
    }
}

fn persist_recovery_event(
    run_id: &str,
    stage_name: &str,
    action_key: &str,
    status: &str,
    expected: Option<&str>,
    actual: Option<&str>,
    details: Option<&str>,
) -> Result<bool, String> {
    let run_exists = db::get_task_run(run_id)
        .map_err(|e| e.to_string())?
        .is_some();
    if !run_exists {
        return Ok(false);
    }

    let clean_stage = if stage_name.trim().is_empty() {
        "recovery"
    } else {
        stage_name.trim()
    };
    let clean_action = if action_key.trim().is_empty() {
        "recovery.event"
    } else {
        action_key.trim()
    };
    let clean_status = if status.trim().is_empty() {
        "completed"
    } else {
        status.trim()
    };
    let expected_value = expected
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("completed");
    let actual_value = actual
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(clean_status);
    let passed = matches!(
        clean_status.to_lowercase().as_str(),
        "completed" | "success" | "ok"
    );
    let stage_order = 5;

    db::record_task_stage_run(run_id, clean_stage, stage_order, clean_status, details)
        .map_err(|e| e.to_string())?;
    db::record_task_stage_assertion(
        run_id,
        clean_stage,
        clean_action,
        expected_value,
        actual_value,
        passed,
        details,
    )
    .map_err(|e| e.to_string())?;
    Ok(true)
}

async fn agent_recovery_event_handler(
    Json(payload): Json<AgentRecoveryEventRequest>,
) -> impl IntoResponse {
    let run_id = payload.run_id.trim().to_string();
    let action_key = payload.action_key.trim().to_string();
    let status = payload.status.trim().to_string();
    if run_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "missing_run_id" })),
        )
            .into_response();
    }
    if action_key.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "missing_action_key" })),
        )
            .into_response();
    }
    if status.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "missing_status" })),
        )
            .into_response();
    }

    let stage_name = payload
        .stage_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("recovery")
        .to_string();
    match persist_recovery_event(
        &run_id,
        &stage_name,
        &action_key,
        &status,
        payload.expected.as_deref(),
        payload.actual.as_deref(),
        payload.details.as_deref(),
    ) {
        Ok(true) => (
            StatusCode::OK,
            Json(json!(AgentRecoveryEventResponse {
                ok: true,
                recorded: true,
                run_id,
                stage_name,
                action_key,
                status,
                recorded_at: chrono::Utc::now().to_rfc3339(),
                reason: None,
            })),
        )
            .into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!(AgentRecoveryEventResponse {
                ok: false,
                recorded: false,
                run_id,
                stage_name,
                action_key,
                status,
                recorded_at: chrono::Utc::now().to_rfc3339(),
                reason: Some("run_not_found".to_string()),
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "record_recovery_failed", "details": err })),
        )
            .into_response(),
    }
}

async fn scan_project_handler(Query(query): Query<ProjectScanQuery>) -> Json<ProjectScanResponse> {
    let workdir = query.workdir.unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    });
    let scanner = project_scanner::ProjectScanner::new(&workdir);
    let result = scanner.scan(query.max_files);
    let project_type = scanner.get_project_type();

    Json(ProjectScanResponse {
        project_type: project_type.as_str().to_string(),
        files: result.files,
        key_files: result.key_files,
    })
}

async fn run_runtime_verification_handler(
    Json(payload): Json<RuntimeVerifyRequest>,
) -> Json<runtime_verification::RuntimeVerifyResult> {
    let options = runtime_verification::RuntimeVerifyOptions {
        workdir: payload.workdir,
        run_backend: payload.run_backend,
        run_frontend: payload.run_frontend,
        run_e2e: payload.run_e2e,
        run_build_checks: payload.run_build_checks,
        backend_port: payload.backend_port,
        frontend_port: payload.frontend_port,
        backend_health_path: payload.backend_health_path,
    };
    let result = runtime_verification::run_runtime_verification(options).await;
    let summary = if result.issues.is_empty() {
        "Runtime verification passed".to_string()
    } else {
        format!("Runtime verification issues: {}", result.issues.len())
    };
    log_verification_run(
        "runtime",
        result.issues.is_empty(),
        &summary,
        Some(
            json!({ "issues": result.issues, "backend_health": result.backend_health, "frontend_health": result.frontend_health }),
        ),
    );
    Json(result)
}

async fn run_visual_verification_handler(
    State(state): State<AppState>,
    Json(payload): Json<visual_verification::VisualVerifyRequest>,
) -> Json<visual_verification::VisualVerifyResult> {
    let Some(llm) = &state.llm_client else {
        return Json(visual_verification::VisualVerifyResult {
            ok: false,
            verdicts: vec![],
        });
    };
    match visual_verification::verify_screen(llm.as_ref(), payload).await {
        Ok(result) => {
            let summary = if result.ok {
                "Visual verification passed"
            } else {
                "Visual verification failed"
            };
            let details = json!({
                "verdicts": result.verdicts.iter().map(|v| json!({ "prompt": v.prompt, "ok": v.ok })).collect::<Vec<_>>()
            });
            log_verification_run("visual", result.ok, summary, Some(details));
            Json(result)
        }
        Err(_) => Json(visual_verification::VisualVerifyResult {
            ok: false,
            verdicts: vec![],
        }),
    }
}

async fn run_semantic_verification_handler(
    Json(payload): Json<SemanticVerifyRequest>,
) -> Json<semantic_verification::SemanticVerificationResult> {
    let workdir = payload.workdir.unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    });
    let max_files = payload.max_files.unwrap_or(200);
    let result =
        semantic_verification::semantic_consistency(std::path::Path::new(&workdir), max_files);
    let details = json!({
        "issues": result.issues.iter().take(10).map(|i| json!({"file": i.file, "severity": i.severity, "reason": i.reason})).collect::<Vec<_>>()
    });
    log_verification_run("semantic", result.ok, &result.reason, Some(details));
    Json(result)
}

async fn run_performance_verification_handler(
    Json(payload): Json<PerformanceVerifyRequest>,
) -> Json<performance_verification::PerformanceVerificationResult> {
    let workdir = payload.workdir.unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    });
    let max_files = payload.max_files.unwrap_or(300);
    let result =
        performance_verification::performance_baseline(std::path::Path::new(&workdir), max_files);
    let details = json!({
        "metrics": result.metrics.iter().map(|m| json!({"name": m.name, "value": m.value, "threshold": m.threshold, "ok": m.ok})).collect::<Vec<_>>()
    });
    log_verification_run("performance", result.ok, &result.reason, Some(details));
    Json(result)
}

async fn run_consistency_verification_handler(
    Json(payload): Json<consistency_check::ConsistencyCheckRequest>,
) -> Json<consistency_check::ConsistencyCheckResult> {
    let result = consistency_check::run_consistency_check(payload);
    let details = json!({
        "summary": result.summary,
        "issues": result.issues.iter().take(10).map(|i| json!({"path": i.path, "source": i.source})).collect::<Vec<_>>()
    });
    log_verification_run("consistency", result.ok, &result.summary, Some(details));
    Json(result)
}

async fn run_judgment_handler(
    Json(payload): Json<judgment::JudgmentRequest>,
) -> Json<judgment::JudgmentResponse> {
    let result = judgment::evaluate_judgment(payload);
    Json(result)
}

async fn set_release_baseline_handler(
    Json(payload): Json<release_gate::ReleaseBaselineRequest>,
) -> Result<Json<release_gate::ReleaseBaseline>, StatusCode> {
    if has_release_baseline_override(&payload) && !allow_operational_path_override() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let request = if allow_operational_path_override() && has_release_baseline_override(&payload) {
        payload
    } else {
        default_release_baseline_request()
    };
    let baseline = release_gate::build_baseline(request);
    release_gate::save_baseline(&baseline);
    Ok(Json(baseline))
}

async fn run_release_gate_handler(
    Json(payload): Json<release_gate::ReleaseGateRequest>,
) -> Json<release_gate::ReleaseGateResult> {
    let result = release_gate::run_release_gate(payload);
    let summary = if result.ok {
        "Release gate passed"
    } else {
        "Release gate failed"
    };
    let details = json!({
        "regressions": result.regressions.iter().take(10).cloned().collect::<Vec<_>>(),
        "warnings": result.warnings.iter().take(10).cloned().collect::<Vec<_>>()
    });
    log_verification_run("release_gate", result.ok, summary, Some(details));
    Json(result)
}

async fn run_release_readiness_handler(
    Json(payload): Json<ReleaseReadinessRequest>,
) -> Result<Json<release_readiness::ReleaseReadinessReport>, StatusCode> {
    if payload.save_baseline.unwrap_or(false) && !allow_operational_path_override() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if requested_operational_override(payload.config_path.as_deref())
        && !allow_operational_path_override()
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if requested_operational_override(payload.snapshot_output_path.as_deref())
        && !allow_operational_path_override()
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let workdir = resolve_operational_workdir(payload.workdir.as_deref());
    let config_path = resolve_operational_file_override(payload.config_path.as_deref())
        .unwrap_or_else(default_launch_eval_config_path);
    let snapshot_output_path =
        resolve_operational_file_override(payload.snapshot_output_path.as_deref())
            .map(|value| value.display().to_string());
    let report = release_readiness::run_release_readiness(
        &workdir,
        &config_path,
        payload.candidate_limit.unwrap_or(20).clamp(1, 50),
        snapshot_output_path.as_deref(),
        payload.save_baseline.unwrap_or(false) && allow_operational_path_override(),
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ok = report.ready_for_launch;
    let summary = if ok {
        "Release readiness passed"
    } else {
        "Release readiness found issues"
    };
    let details = json!({
        "candidate_snapshot_scenarios": report.candidate_snapshot.scenario_count,
        "launch_eval_passed": report.launch_eval.passed,
        "launch_eval_total": report.launch_eval.total,
        "http_e2e_passed": report.http_e2e.as_ref().map(|value| value.passed),
        "http_e2e_total": report.http_e2e.as_ref().map(|value| value.total),
        "http_e2e_ok": report.http_e2e.as_ref().map(|value| value.ok),
        "release_gate_ok": report.release_gate.ok,
        "status": report.status,
        "ready_for_launch": report.ready_for_launch,
        "baseline_saved": report.baseline_saved,
        "report_json_path": report.report_json_path,
        "report_markdown_path": report.report_markdown_path,
        "blockers": report.blockers.iter().take(10).cloned().collect::<Vec<_>>(),
        "regressions": report.release_gate.regressions.iter().take(10).cloned().collect::<Vec<_>>(),
        "warnings": report.advisories.iter().take(10).cloned().collect::<Vec<_>>()
    });
    log_verification_run("release_readiness", ok, summary, Some(details));

    Ok(Json(report))
}

async fn get_latest_release_readiness_handler(
    Query(query): Query<ReleaseReadinessHistoryQuery>,
) -> Result<Json<Option<release_readiness::ReleaseReadinessReport>>, StatusCode> {
    let workdir = resolve_operational_workdir(query.workdir.as_deref());
    let report = release_readiness::load_latest_release_readiness_report(&workdir)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(report))
}

async fn get_release_readiness_history_handler(
    Query(query): Query<ReleaseReadinessHistoryQuery>,
) -> Result<Json<Vec<release_readiness::ReleaseReadinessHistoryEntry>>, StatusCode> {
    let workdir = resolve_operational_workdir(query.workdir.as_deref());
    let limit = query.limit.unwrap_or(10).clamp(1, 50);
    let history = release_readiness::list_release_readiness_history(&workdir, limit)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(history))
}

async fn run_http_e2e_handler(
    Json(payload): Json<HttpE2ERequest>,
) -> Result<Json<crate::http_e2e::HttpE2EReport>, StatusCode> {
    let workdir = resolve_operational_workdir(payload.workdir.as_deref());
    let report = crate::http_e2e::run_http_e2e(&workdir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let summary = if report.ok {
        "HTTP E2E passed"
    } else {
        "HTTP E2E found failures"
    };
    let details = json!({
        "passed": report.passed,
        "total": report.total,
        "report_json_path": report.report_json_path,
        "report_markdown_path": report.report_markdown_path,
        "failed_steps": report
            .steps
            .iter()
            .filter(|step| !step.ok)
            .map(|step| step.name.clone())
            .take(10)
            .collect::<Vec<_>>()
    });
    log_verification_run("http_e2e", report.ok, summary, Some(details));

    Ok(Json(report))
}

async fn get_latest_http_e2e_handler(
    Query(query): Query<HttpE2ERequest>,
) -> Result<Json<Option<crate::http_e2e::HttpE2EReport>>, StatusCode> {
    let workdir = resolve_operational_workdir(query.workdir.as_deref());
    let report = crate::http_e2e::load_latest_http_e2e_report(&workdir)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(report))
}

async fn get_http_e2e_history_handler(
    Query(query): Query<HttpE2EHistoryQuery>,
) -> Result<Json<Vec<crate::http_e2e::HttpE2EHistoryEntry>>, StatusCode> {
    let workdir = resolve_operational_workdir(query.workdir.as_deref());
    let limit = query.limit.unwrap_or(10).clamp(1, 50);
    let history = crate::http_e2e::list_http_e2e_history(&workdir, limit)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(history))
}

async fn run_exec_results_guard_handler(
    Json(payload): Json<tool_result_guard::ToolResultGuardRequest>,
) -> Json<tool_result_guard::ToolResultGuardResult> {
    let result = tool_result_guard::guard_exec_results(payload);
    Json(result)
}

async fn score_quality_handler(
    State(state): State<AppState>,
    Json(payload): Json<QualityScoreRequest>,
) -> Json<QualityScoreResponse> {
    let runtime = if let Some(rt) = payload.runtime {
        rt
    } else if let Some(opts) = payload.runtime_options {
        let options = runtime_verification::RuntimeVerifyOptions {
            workdir: opts.workdir,
            run_backend: opts.run_backend,
            run_frontend: opts.run_frontend,
            run_e2e: opts.run_e2e,
            run_build_checks: opts.run_build_checks,
            backend_port: opts.backend_port,
            frontend_port: opts.frontend_port,
            backend_health_path: opts.backend_health_path,
        };
        runtime_verification::run_runtime_verification(options).await
    } else {
        runtime_verification::RuntimeVerifyResult {
            backend_started: false,
            backend_health: false,
            backend_build_ok: None,
            frontend_started: false,
            frontend_health: false,
            frontend_build_ok: None,
            e2e_passed: None,
            issues: vec!["No runtime verification provided".to_string()],
            logs: Vec::new(),
        }
    };

    let use_llm = payload.use_llm.unwrap_or(false);
    let score = if use_llm {
        if let Some(llm) = &state.llm_client {
            match quality_scorer::score_quality_with_llm(
                llm.as_ref(),
                payload.goal.as_deref(),
                Some(&runtime),
                payload.code_review.as_ref(),
            )
            .await
            {
                Ok(score) => score,
                Err(_) => {
                    quality_scorer::score_quality(Some(&runtime), payload.code_review.as_ref())
                }
            }
        } else {
            quality_scorer::score_quality(Some(&runtime), payload.code_review.as_ref())
        }
    } else {
        quality_scorer::score_quality(Some(&runtime), payload.code_review.as_ref())
    };
    let _ = db::insert_quality_score(&score);
    let created_at = chrono::Utc::now().to_rfc3339();
    Json(QualityScoreResponse { created_at, score })
}

async fn latest_quality_handler() -> Json<Option<QualityScoreResponse>> {
    match db::get_latest_quality_score() {
        Ok(Some(record)) => {
            let score = quality_scorer::QualityScore {
                overall: record.overall,
                breakdown: record
                    .breakdown
                    .as_object()
                    .map(|map| {
                        map.iter()
                            .filter_map(|(k, v)| v.as_f64().map(|val| (k.clone(), val)))
                            .collect()
                    })
                    .unwrap_or_default(),
                issues: record.issues,
                strengths: record.strengths,
                recommendation: record.recommendation,
                summary: record.summary,
            };
            Json(Some(QualityScoreResponse {
                created_at: record.created_at,
                score,
            }))
        }
        _ => Json(None),
    }
}

// Ingest Handler (Replaces Python main.py)
async fn ingest_events(Json(payload): Json<serde_json::Value>) -> Json<serde_json::Value> {
    // 1. Normalize
    let events: Vec<crate::schema::EventEnvelope> = if let Some(arr) = payload.as_array() {
        arr.iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect()
    } else if let Ok(single) = serde_json::from_value(payload.clone()) {
        vec![single]
    } else {
        return Json(serde_json::json!({ "error": "Invalid Event Format", "count": 0 }));
    };

    let count = events.len();

    // 2. Process & Insert
    // In a real high-perf scenario, we would push to a channel (EventBus).
    // For now, direct DB insert is fast enough for direct migration.

    // Initialize Privacy Guard (Salt should come from env in prod)
    let salt = std::env::var("PRIVACY_SALT").unwrap_or_else(|_| "default_salt".to_string());
    let guard = crate::privacy::PrivacyGuard::new(salt);

    let mut success = 0;
    for event in events {
        // [Privacy] Apply masking
        if let Some(masked_event) = guard.apply(event) {
            if let Err(e) = db::insert_event_v2(&masked_event) {
                eprintln!("Ingest Error: {}", e);
            } else {
                success += 1;
            }
        } else {
            // Dropped by privacy rules (e.g. deny list)
            println!("Event dropped by PrivacyGuard");
        }
    }

    Json(serde_json::json!({
        "status": "queued",
        "received": count,
        "processed": success
    }))
}

async fn run_ai_digest_handler(Json(req): Json<AiDigestRunRequest>) -> impl IntoResponse {
    let text = ai_digest::normalize_request_text(req.text.as_deref());
    match ai_digest::trigger_program_webhook(&text, req.scope_marker).await {
        Ok(result) => (
            StatusCode::OK,
            Json(AiDigestRunResponse {
                ok: true,
                scope_marker: result.scope_marker,
                notion_url: result.notion_url,
                webhook_url: result.webhook_url,
                status_code: result.status_code,
                response_text: result.response_text,
            }),
        )
            .into_response(),
        Err(err) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "ok": false,
                "error": err.to_string(),
            })),
        )
            .into_response(),
    }
}

async fn run_news_summary_handler(
    State(state): State<AppState>,
    Json(req): Json<NewsSummaryRequest>,
) -> impl IntoResponse {
    let Some(llm) = state.llm_client.clone() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "ok": false,
                "error": "llm_client_not_available",
            })),
        )
            .into_response();
    };

    let clean = |v: Option<&str>, max_len: usize| -> String {
        v.unwrap_or("")
            .trim()
            .chars()
            .take(max_len)
            .collect::<String>()
    };

    let title = clean(req.title.as_deref(), 220);
    let link = clean(req.link.as_deref(), 500);
    let source = clean(req.source.as_deref(), 120);
    let pub_date = clean(req.pub_date.as_deref(), 120);
    let description = clean(req.description.as_deref(), 700);
    let scope_marker = clean(req.scope_marker.as_deref(), 120);
    let telegram_chat_id = clean(req.telegram_chat_id.as_deref(), 80);
    let topic = {
        let t = clean(req.topic.as_deref(), 80);
        if t.is_empty() {
            "뉴스".to_string()
        } else {
            t
        }
    };
    let article_count = req.article_count.unwrap_or(5).clamp(1, 10);
    let transparency_note = "원문 접근 제한: RSS title/description 기반 요약";

    let prompt = format!(
        "당신은 뉴스 요약기입니다. 입력(title/link/source/pubDate/description)만 사용해 요약하세요.\n\
JSON 객체 하나만 출력하고 코드블록/마크다운/설명문을 추가하지 마세요.\n\
summary_bullets는 한국어 완결 문장 3~6개로 작성하세요.\n\
transparency_note에는 반드시 \"{}\"를 포함하세요.\n\n\
입력:\n\
- title: {}\n\
- link: {}\n\
- source: {}\n\
- pubDate: {}\n\
- description: {}\n\n\
출력 스키마:\n\
{{\n\
  \"title\": \"string\",\n\
  \"link\": \"string\",\n\
  \"source\": \"string\",\n\
  \"pubDate\": \"string\",\n\
  \"summary_bullets\": [\"string\", \"string\", \"string\"],\n\
  \"why_it_matters\": \"string\",\n\
  \"keywords\": [\"string\", \"string\", \"string\"],\n\
  \"transparency_note\": \"string\",\n\
  \"scope_marker\": \"string\",\n\
  \"telegram_chat_id\": \"string\",\n\
  \"article_count\": {}\n\
}}",
        transparency_note, title, link, source, pub_date, description, article_count
    );

    let messages = vec![
        json!({
            "role": "system",
            "content": "Return only one valid JSON object. No markdown."
        }),
        json!({
            "role": "user",
            "content": prompt
        }),
    ];

    let raw = match llm.chat_completion(messages).await {
        Ok(v) => v,
        Err(err) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "ok": false,
                    "error": format!("llm_chat_completion_failed: {}", err),
                })),
            )
                .into_response();
        }
    };

    let parsed = match llm_gateway::recover_json(&raw) {
        Some(v) => v,
        None => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "ok": false,
                    "error": "llm_output_not_json",
                    "raw_preview": truncate_log_message(&raw, 300),
                })),
            )
                .into_response();
        }
    };

    let get_str = |k: &str| -> Option<String> {
        parsed
            .get(k)
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };

    let mut bullets: Vec<String> = parsed
        .get("summary_bullets")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .take(6)
                .map(|s| s.chars().take(190).collect::<String>())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if bullets.len() < 3 {
        let fallback_title = if title.is_empty() {
            "주요 뉴스".to_string()
        } else {
            title.clone()
        };
        let fallback_desc = if description.is_empty() {
            "RSS 본문이 짧아 제목 중심으로 요약했습니다.".to_string()
        } else {
            description.chars().take(170).collect::<String>()
        };
        bullets = vec![
            format!("핵심: {}", fallback_title),
            format!("내용: {}", fallback_desc),
            format!(
                "영향: {} 맥락의 후속 의사결정에 참고할 가치가 있습니다.",
                topic
            ),
        ];
    }

    let mut keywords: Vec<String> = parsed
        .get("keywords")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .take(3)
                .map(|s| s.chars().take(30).collect::<String>())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    while keywords.len() < 3 {
        let filler = match keywords.len() {
            0 => topic.clone(),
            1 => "트렌드".to_string(),
            _ => "요약".to_string(),
        };
        keywords.push(filler);
    }

    let summary = json!({
        "title": get_str("title").unwrap_or_else(|| title.clone()),
        "link": get_str("link").unwrap_or_else(|| link.clone()),
        "source": get_str("source").unwrap_or_else(|| source.clone()),
        "pubDate": get_str("pubDate").unwrap_or_else(|| pub_date.clone()),
        "summary_bullets": bullets,
        "why_it_matters": get_str("why_it_matters").unwrap_or_else(|| format!("이 이슈는 {} 맥락의 실행 우선순위와 전략에 영향을 줄 수 있습니다.", topic)),
        "keywords": keywords,
        "transparency_note": transparency_note,
        "scope_marker": if scope_marker.is_empty() { get_str("scope_marker").unwrap_or_default() } else { scope_marker.clone() },
        "telegram_chat_id": if telegram_chat_id.is_empty() { get_str("telegram_chat_id").unwrap_or_default() } else { telegram_chat_id.clone() },
        "article_count": article_count,
    });

    (
        StatusCode::OK,
        Json(NewsSummaryResponse { ok: true, summary }),
    )
        .into_response()
}

fn request_memory_min_confidence() -> f64 {
    std::env::var("ALLVIA_REQUEST_MEMORY_MIN_CONFIDENCE")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(|v| v.clamp(0.0, 1.0))
        .unwrap_or(0.8)
}

fn normalize_chat_scope_part(value: Option<&str>) -> Option<String> {
    let normalized = value
        .unwrap_or_default()
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn build_chat_memory_scope(
    channel: Option<&str>,
    chat_type: Option<&str>,
    sender: Option<&str>,
) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(channel) = normalize_chat_scope_part(channel) {
        parts.push(format!("channel_{}", channel));
    }
    if let Some(chat_type) = normalize_chat_scope_part(chat_type) {
        parts.push(format!("type_{}", chat_type));
    }
    if let Some(sender) = normalize_chat_scope_part(sender) {
        parts.push(format!("sender_{}", sender));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("__"))
    }
}

fn chat_request_memory_scope(req: &ChatRequest) -> Option<String> {
    build_chat_memory_scope(
        req.channel.as_deref(),
        req.chat_type.as_deref(),
        req.sender.as_deref(),
    )
}

fn chat_feedback_memory_scope(req: &ChatFeedbackRequest) -> Option<String> {
    build_chat_memory_scope(
        req.channel.as_deref(),
        req.chat_type.as_deref(),
        req.sender.as_deref(),
    )
}

fn request_memory_response_mode(command: &str) -> &'static str {
    match command {
        "help" | "help_local" | "greeting_local" => "reusable_response",
        "calendar_today" | "calendar_week" | "gmail_list" => "ttl_response_signature",
        "system_status" => "ttl_response_exact",
        _ => "intent_only",
    }
}

fn request_memory_signature_cache_supported(command: &str) -> bool {
    matches!(command, "calendar_today" | "calendar_week" | "gmail_list")
}

fn request_memory_response_is_successful(response: &str) -> bool {
    let trimmed = response.trim();
    if trimmed.is_empty() {
        return false;
    }

    !trimmed.starts_with("❌")
        && !trimmed.starts_with("⚠️")
        && !trimmed.starts_with("⛔")
        && !trimmed.starts_with("🤔")
        && !trimmed.starts_with("❓")
}

fn request_memory_should_store(command: &str, confidence: f64, response: &str) -> bool {
    !command.trim().is_empty()
        && !command.eq_ignore_ascii_case("unknown")
        && confidence >= request_memory_min_confidence()
        && request_memory_response_is_successful(response)
}

fn request_memory_response_cache_ttl_secs(command: &str, response_mode: &str) -> Option<i64> {
    let env_key = match command {
        "gmail_list" => Some("ALLVIA_RESPONSE_CACHE_TTL_GMAIL_LIST"),
        "calendar_today" => Some("ALLVIA_RESPONSE_CACHE_TTL_CALENDAR_TODAY"),
        "calendar_week" => Some("ALLVIA_RESPONSE_CACHE_TTL_CALENDAR_WEEK"),
        "system_status" => Some("ALLVIA_RESPONSE_CACHE_TTL_SYSTEM_STATUS"),
        _ => None,
    };

    if response_mode == "reusable_response" {
        return env_key
            .and_then(|key| std::env::var(key).ok())
            .and_then(|value| value.trim().parse::<i64>().ok())
            .or(Some(24 * 60 * 60));
    }

    if !matches!(
        response_mode,
        "ttl_response_signature" | "ttl_response_exact"
    ) {
        return None;
    }

    env_key
        .and_then(|key| std::env::var(key).ok())
        .and_then(|value| value.trim().parse::<i64>().ok())
        .or_else(|| match command {
            "gmail_list" => Some(20),
            "calendar_today" => Some(30),
            "calendar_week" => Some(120),
            "system_status" => Some(10),
            _ => None,
        })
        .filter(|ttl| *ttl > 0)
}

fn request_memory_response_mode_supports_signature(response_mode: &str) -> bool {
    matches!(response_mode, "ttl_response_signature")
}

fn command_supports_live_refresh(command: &str) -> bool {
    matches!(
        command,
        "gmail_list" | "calendar_today" | "calendar_week" | "system_status"
    )
}

fn request_prefers_fresh_data(message: &str, command: &str) -> bool {
    if !command_supports_live_refresh(command) {
        return false;
    }

    let normalized = crate::request_memory::normalize_request_text(message);
    if normalized.is_empty() {
        return false;
    }

    let tokens = normalized.split_whitespace().collect::<HashSet<_>>();
    let token_hit = [
        "새로고침",
        "refresh",
        "realtime",
        "실시간",
        "최신으로",
        "지금",
        "방금",
    ]
    .iter()
    .any(|token| tokens.contains(token));
    let phrase_hit = ["real time", "latest status", "up to date"]
        .iter()
        .any(|phrase| normalized.contains(phrase));

    token_hit || phrase_hit || (tokens.contains("다시") && tokens.contains("확인"))
}

fn repeat_request_fresh_window_secs() -> Option<i64> {
    std::env::var("ALLVIA_REPEAT_REQUEST_FRESH_WINDOW_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .or(Some(15))
        .filter(|window| *window > 0)
}

fn timestamp_is_within_window(timestamp: &str, window_secs: i64) -> bool {
    let recorded_at = match chrono::DateTime::parse_from_rfc3339(timestamp) {
        Ok(value) => value.with_timezone(&chrono::Utc),
        Err(_) => return false,
    };
    let age_secs = chrono::Utc::now()
        .signed_duration_since(recorded_at)
        .num_seconds();
    age_secs >= 0 && age_secs <= window_secs
}

fn chat_memory_source_allows_repeat_bypass(source: &str) -> bool {
    source.starts_with("api.chat.")
}

fn request_memory_recent_chat_reask(
    record: &db::RequestMemoryRecord,
    command: &str,
    window_secs: i64,
) -> bool {
    record.intent_command.as_deref() == Some(command)
        && chat_memory_source_allows_repeat_bypass(&record.source)
        && feedback_allows_response_reuse(
            record.positive_feedback_count,
            record.negative_feedback_count,
        )
        && timestamp_is_within_window(&record.last_used_at, window_secs)
}

fn execution_memory_recent_chat_reask(
    record: &db::ExecutionMemoryRecord,
    command: &str,
    window_secs: i64,
) -> bool {
    record.intent_command == command
        && chat_memory_source_allows_repeat_bypass(&record.source)
        && feedback_allows_response_reuse(
            record.positive_feedback_count,
            record.negative_feedback_count,
        )
        && timestamp_is_within_window(&record.last_used_at, window_secs)
}

fn request_recently_reasked(memory_scope: Option<&str>, message: &str, command: &str) -> bool {
    if !command_supports_live_refresh(command) {
        return false;
    }
    let Some(window_secs) = repeat_request_fresh_window_secs() else {
        return false;
    };

    if let Some(record) = db::get_request_memory_scoped(memory_scope, message)
        .ok()
        .flatten()
    {
        if request_memory_recent_chat_reask(&record, command, window_secs) {
            return true;
        }
    }

    if !request_memory_signature_cache_supported(command) {
        return false;
    }

    let input_signature = crate::request_memory::build_request_signature(message);
    if input_signature.is_empty() {
        return false;
    }

    db::get_request_memory_by_signature_scoped(memory_scope, &input_signature, &[command])
        .ok()
        .flatten()
        .is_some_and(|record| request_memory_recent_chat_reask(&record, command, window_secs))
}

fn feedback_allows_response_reuse(
    positive_feedback_count: i64,
    negative_feedback_count: i64,
) -> bool {
    negative_feedback_count == 0 || positive_feedback_count > negative_feedback_count
}

fn request_memory_static_intent(command: &str) -> Value {
    json!({
        "command": command,
        "params": {},
        "confidence": 1.0
    })
}

fn persist_chat_response_memory(
    memory_scope: Option<&str>,
    message: &str,
    command: &str,
    response: &str,
    source: &str,
    confidence: f64,
) {
    if let Err(e) = db::insert_chat_message("assistant", response) {
        eprintln!("Failed to save assistant chat: {}", e);
    }

    if request_memory_should_store(command, confidence, response) {
        let intent = request_memory_static_intent(command);
        let _ = db::upsert_request_memory_scoped(
            memory_scope,
            message,
            Some(&intent),
            Some(response),
            request_memory_response_mode(command),
            source,
            confidence,
        );
    }
}

fn chat_response_memory_source(intent: &Value) -> &'static str {
    match intent.get("source").and_then(|value| value.as_str()) {
        Some("deterministic") => "api.chat.deterministic",
        _ => "api.chat.llm",
    }
}

fn request_memory_response_fresh(
    record: &db::RequestMemoryRecord,
    command: &str,
) -> Option<String> {
    if record.intent_command.as_deref() != Some(command) {
        return None;
    }
    if !feedback_allows_response_reuse(
        record.positive_feedback_count,
        record.negative_feedback_count,
    ) {
        return None;
    }

    let response = record
        .response_text
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();

    let ttl_secs = request_memory_response_cache_ttl_secs(command, &record.response_mode)?;
    if ttl_secs <= 0 {
        return None;
    }

    if record.response_mode == "reusable_response" {
        return Some(response);
    }

    let updated_at = chrono::DateTime::parse_from_rfc3339(&record.updated_at).ok()?;
    let age_secs = chrono::Utc::now()
        .signed_duration_since(updated_at.with_timezone(&chrono::Utc))
        .num_seconds();
    if age_secs < 0 || age_secs > ttl_secs {
        return None;
    }

    Some(response)
}

pub(crate) fn load_cached_request_response(
    memory_scope: Option<&str>,
    message: &str,
    command: &str,
) -> Option<String> {
    if request_prefers_fresh_data(message, command)
        || request_recently_reasked(memory_scope, message, command)
    {
        return None;
    }

    if let Some(record) = db::get_request_memory_scoped(memory_scope, message)
        .ok()
        .flatten()
    {
        if let Some(response) = request_memory_response_fresh(&record, command) {
            return Some(response);
        }
    }

    let input_signature = crate::request_memory::build_request_signature(message);
    if input_signature.is_empty() {
        return None;
    }

    let record =
        db::get_request_memory_by_signature_scoped(memory_scope, &input_signature, &[command])
            .ok()
            .flatten()?;
    if !request_memory_response_mode_supports_signature(&record.response_mode) {
        return None;
    }
    request_memory_response_fresh(&record, command)
}

fn build_local_chat_response(
    memory_scope: Option<&str>,
    message: &str,
    command: &str,
    response: String,
    source: &str,
) -> Json<ChatResponse> {
    persist_chat_response_memory(memory_scope, message, command, &response, source, 1.0);
    Json(ChatResponse {
        response,
        command: Some(command.to_string()),
        route_meta: None,
    })
}

fn load_local_cached_chat_response(
    memory_scope: Option<&str>,
    message: &str,
    command: &str,
) -> Option<Json<ChatResponse>> {
    let response = load_cached_request_response(memory_scope, message, command)?;
    persist_chat_response_memory(
        memory_scope,
        message,
        command,
        &response,
        "api.chat.local_cache",
        1.0,
    );
    Some(Json(ChatResponse {
        response,
        command: Some(command.to_string()),
        route_meta: None,
    }))
}

fn parse_u32_param(
    value: Option<&Value>,
    default_value: u32,
    min_value: u32,
    max_value: u32,
) -> u32 {
    value
        .and_then(|raw| {
            raw.as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .or_else(|| raw.as_str().and_then(|v| v.trim().parse::<u32>().ok()))
        })
        .unwrap_or(default_value)
        .clamp(min_value, max_value)
}

fn execution_memory_supported(command: &str) -> bool {
    matches!(command, "calendar_today" | "calendar_week" | "gmail_list")
}

fn execution_memory_tool_path(command: &str) -> &'static str {
    match command {
        "calendar_today" => "integrations.calendar.list_today",
        "calendar_week" => "integrations.calendar.list_week",
        "gmail_list" => "integrations.gmail.list_messages",
        _ => "api.chat",
    }
}

fn execution_memory_ttl_secs(command: &str) -> Option<i64> {
    request_memory_response_cache_ttl_secs(command, "ttl_response_signature")
}

fn execution_memory_params(command: &str, intent: &Value) -> Option<(String, Value)> {
    match command {
        "gmail_list" => {
            let count = parse_u32_param(intent["params"].get("count"), 5, 1, 20);
            Some((format!("count={}", count), json!({ "count": count })))
        }
        "calendar_today" | "calendar_week" => Some(("default".to_string(), json!({}))),
        _ => None,
    }
}

fn execution_memory_should_store(command: &str, response: &str, confidence: f64) -> bool {
    execution_memory_supported(command)
        && request_memory_response_is_successful(response)
        && confidence >= request_memory_min_confidence()
        && execution_memory_ttl_secs(command).unwrap_or(0) > 0
}

fn persist_execution_memory(
    memory_scope: Option<&str>,
    message: &str,
    command: &str,
    intent: &Value,
    response: &str,
    source: &str,
) {
    let confidence = intent["confidence"].as_f64().unwrap_or(0.0);
    if !execution_memory_should_store(command, response, confidence) {
        return;
    }

    let Some((params_key, params_json)) = execution_memory_params(command, intent) else {
        return;
    };
    let Some(ttl_secs) = execution_memory_ttl_secs(command) else {
        return;
    };
    let request_signature = crate::request_memory::build_request_signature(message);
    let signature = if request_signature.trim().is_empty() {
        None
    } else {
        Some(request_signature.as_str())
    };
    let _ = db::upsert_execution_memory_scoped(
        memory_scope,
        command,
        &params_key,
        Some(&params_json),
        signature,
        response,
        ttl_secs,
        source,
        execution_memory_tool_path(command),
    );
}

fn execution_memory_response_fresh(
    record: &db::ExecutionMemoryRecord,
    command: &str,
) -> Option<String> {
    if !record.success || record.intent_command != command {
        return None;
    }
    if !feedback_allows_response_reuse(
        record.positive_feedback_count,
        record.negative_feedback_count,
    ) {
        return None;
    }

    let response = record.response_text.trim();
    if response.is_empty() {
        return None;
    }

    let configured_ttl = execution_memory_ttl_secs(command).unwrap_or(record.freshness_ttl_seconds);
    let ttl_secs = if record.freshness_ttl_seconds > 0 {
        configured_ttl.min(record.freshness_ttl_seconds)
    } else {
        configured_ttl
    };
    if ttl_secs <= 0 {
        return None;
    }

    let updated_at = chrono::DateTime::parse_from_rfc3339(&record.updated_at).ok()?;
    let age_secs = chrono::Utc::now()
        .signed_duration_since(updated_at.with_timezone(&chrono::Utc))
        .num_seconds();
    if age_secs < 0 || age_secs > ttl_secs {
        return None;
    }

    Some(response.to_string())
}

pub(crate) fn load_cached_execution_response(
    memory_scope: Option<&str>,
    message: &str,
    command: &str,
    intent: &Value,
) -> Option<String> {
    if request_prefers_fresh_data(message, command)
        || request_recently_reasked(memory_scope, message, command)
    {
        return None;
    }

    if !execution_memory_supported(command) {
        return None;
    }

    let (params_key, _) = execution_memory_params(command, intent)?;
    let record = db::get_execution_memory_scoped(memory_scope, command, &params_key)
        .ok()
        .flatten()?;
    if let Some(window_secs) = repeat_request_fresh_window_secs() {
        if execution_memory_recent_chat_reask(&record, command, window_secs) {
            return None;
        }
    }

    let cached = execution_memory_response_fresh(&record, command)?;
    persist_execution_memory(
        memory_scope,
        message,
        command,
        intent,
        &cached,
        "api.chat.execution_cache",
    );
    Some(cached)
}

fn request_memory_intent_for_feedback(
    memory_scope: Option<&str>,
    request_text: &str,
) -> Option<Value> {
    let record = db::get_request_memory_scoped(memory_scope, request_text)
        .ok()
        .flatten()?;
    let raw = record.intent_json?;
    serde_json::from_str(&raw).ok()
}

pub(crate) fn load_cached_request_intent(
    memory_scope: Option<&str>,
    message: &str,
) -> Option<(Value, f64)> {
    if let Some(record) = db::get_request_memory_scoped(memory_scope, message)
        .ok()
        .flatten()
    {
        if record.confidence < request_memory_min_confidence() {
            return None;
        }
        if !feedback_allows_response_reuse(
            record.positive_feedback_count,
            record.negative_feedback_count,
        ) {
            return None;
        }
        let raw = record.intent_json?;
        let parsed: Value = serde_json::from_str(&raw).ok()?;
        return Some((parsed, record.confidence));
    }

    let input_signature = crate::request_memory::build_request_signature(message);
    if input_signature.is_empty() {
        return None;
    }

    let allowed_commands = ["calendar_today", "calendar_week", "gmail_list"];
    let record = db::get_request_memory_by_signature_scoped(
        memory_scope,
        &input_signature,
        &allowed_commands,
    )
    .ok()
    .flatten()?;
    if record.confidence < request_memory_min_confidence() {
        return None;
    }
    if !feedback_allows_response_reuse(
        record.positive_feedback_count,
        record.negative_feedback_count,
    ) {
        return None;
    }
    let raw = record.intent_json?;
    let parsed: Value = serde_json::from_str(&raw).ok()?;
    let command = parsed.get("command").and_then(|v| v.as_str()).unwrap_or("");
    if !request_memory_signature_cache_supported(command) {
        return None;
    }
    Some((parsed, record.confidence))
}

fn load_deterministic_chat_intent(message: &str) -> Option<(Value, f64)> {
    let intent = crate::deterministic_intent::classify_chat_intent(message)?;
    let confidence = intent
        .get("confidence")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    Some((intent, confidence))
}

#[derive(Default, Clone, Copy)]
struct ChatOpsFlags {
    freshness_bypassed: bool,
    intent_memory_hit: bool,
    request_memory_hit: bool,
    execution_memory_hit: bool,
    deterministic_used: bool,
    llm_used: bool,
    ai_digest_used: bool,
    local_only: bool,
}

fn safe_text_preview(value: &str, max_chars: usize) -> String {
    let normalized = value.trim().replace('\n', " ");
    let mut chars = normalized.chars();
    let preview: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}...", preview)
    } else {
        preview
    }
}

fn chat_ops_message_preview(message: &str) -> String {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return "<empty>".to_string();
    }
    safe_text_preview(trimmed, 160)
}

fn chat_ops_outcome_from_response(response: &str) -> &'static str {
    if request_memory_response_is_successful(response) {
        "success"
    } else {
        "error"
    }
}

fn record_chat_ops_event(
    req: &ChatRequest,
    memory_scope: Option<&str>,
    message: &str,
    route_kind: &str,
    command: Option<&str>,
    outcome: &str,
    confidence: Option<f64>,
    flags: ChatOpsFlags,
    note: Option<&str>,
) {
    let preview = chat_ops_message_preview(message);
    if let Err(err) = db::record_launch_ops_event(
        req.channel.as_deref(),
        memory_scope,
        &preview,
        route_kind,
        command,
        outcome,
        confidence,
        flags.freshness_bypassed,
        flags.intent_memory_hit,
        flags.request_memory_hit,
        flags.execution_memory_hit,
        flags.deterministic_used,
        flags.llm_used,
        flags.ai_digest_used,
        flags.local_only,
        note,
    ) {
        eprintln!("Failed to record launch ops event: {}", err);
    }
}

fn build_chat_route_meta(
    memory_scope: Option<&str>,
    route_kind: &str,
    outcome: &str,
    confidence: Option<f64>,
    flags: ChatOpsFlags,
    note: Option<&str>,
) -> ChatRouteMeta {
    ChatRouteMeta {
        route_kind: route_kind.to_string(),
        outcome: outcome.to_string(),
        confidence,
        note: note.map(|value| value.to_string()),
        memory_scope: memory_scope.map(|value| value.to_string()),
        freshness_bypassed: flags.freshness_bypassed,
        intent_memory_hit: flags.intent_memory_hit,
        request_memory_hit: flags.request_memory_hit,
        execution_memory_hit: flags.execution_memory_hit,
        deterministic_used: flags.deterministic_used,
        llm_used: flags.llm_used,
        ai_digest_used: flags.ai_digest_used,
        local_only: flags.local_only,
    }
}

fn chat_nl_run_should_record(route_kind: &str, command: Option<&str>, message: &str) -> bool {
    if crate::request_memory::normalize_request_text(message).is_empty() {
        return false;
    }

    if matches!(
        route_kind,
        "empty_message" | "gate_blocked" | "system_command" | "local_command" | "vision_demo"
    ) {
        return false;
    }

    !matches!(
        command.unwrap_or(""),
        "help_local"
            | "greeting_local"
            | "system_status"
            | "telegram_listener_start"
            | "telegram_listener_status"
            | "n8n_restart"
    )
}

fn chat_nl_run_status(route_kind: &str, response: &ChatResponse, outcome: &str) -> &'static str {
    if route_kind == "gate_blocked" || outcome == "blocked" {
        return "blocked";
    }
    if response.command.as_deref() == Some("build_workflow")
        || response.response.contains("승인 후 생성")
        || response.response.contains("approval")
    {
        return "approval_required";
    }
    if response.response.contains("수동으로")
        || response.response.contains("직접 확인")
        || response.response.contains("manual")
    {
        return "manual_required";
    }
    if outcome == "success" {
        "completed"
    } else {
        "error"
    }
}

fn chat_nl_run_summary(response: &str) -> Option<String> {
    let trimmed = response.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(safe_text_preview(trimmed, 160))
}

fn record_chat_nl_run(
    req: &ChatRequest,
    memory_scope: Option<&str>,
    message: &str,
    response: &ChatResponse,
    route_kind: &str,
    outcome: &str,
    confidence: Option<f64>,
    flags: ChatOpsFlags,
    note: Option<&str>,
) {
    if !chat_nl_run_should_record(route_kind, response.command.as_deref(), message) {
        return;
    }

    let status = chat_nl_run_status(route_kind, response, outcome);
    let summary = chat_nl_run_summary(&response.response);
    let details = json!({
        "source": "api.chat",
        "route_kind": route_kind,
        "command": response.command,
        "channel": req.channel,
        "chat_type": req.chat_type,
        "sender": req.sender,
        "memory_scope": memory_scope,
        "outcome": outcome,
        "confidence": confidence,
        "note": note,
        "flags": {
            "freshness_bypassed": flags.freshness_bypassed,
            "intent_memory_hit": flags.intent_memory_hit,
            "request_memory_hit": flags.request_memory_hit,
            "execution_memory_hit": flags.execution_memory_hit,
            "deterministic_used": flags.deterministic_used,
            "llm_used": flags.llm_used,
            "ai_digest_used": flags.ai_digest_used,
            "local_only": flags.local_only
        }
    });
    let details_json = serde_json::to_string(&details).ok();
    let intent = response.command.as_deref().unwrap_or(route_kind);
    if let Err(err) = db::insert_nl_run(
        intent,
        message,
        status,
        summary.as_deref(),
        details_json.as_deref(),
    ) {
        eprintln!("Failed to record chat nl_run: {}", err);
    }
}

fn respond_chat(
    req: &ChatRequest,
    memory_scope: Option<&str>,
    message: &str,
    mut response: ChatResponse,
    route_kind: &str,
    outcome: &str,
    confidence: Option<f64>,
    flags: ChatOpsFlags,
    note: Option<&str>,
) -> Json<ChatResponse> {
    response.route_meta = Some(build_chat_route_meta(
        memory_scope,
        route_kind,
        outcome,
        confidence,
        flags,
        note,
    ));
    record_chat_ops_event(
        req,
        memory_scope,
        message,
        route_kind,
        response.command.as_deref(),
        outcome,
        confidence,
        flags,
        note,
    );
    record_chat_nl_run(
        req,
        memory_scope,
        message,
        &response,
        route_kind,
        outcome,
        confidence,
        flags,
        note,
    );
    Json(response)
}

fn execution_recently_reasked(memory_scope: Option<&str>, command: &str, intent: &Value) -> bool {
    if !execution_memory_supported(command) {
        return false;
    }
    let Some(window_secs) = repeat_request_fresh_window_secs() else {
        return false;
    };
    let Some((params_key, _)) = execution_memory_params(command, intent) else {
        return false;
    };
    db::get_execution_memory_scoped(memory_scope, command, &params_key)
        .ok()
        .flatten()
        .is_some_and(|record| execution_memory_recent_chat_reask(&record, command, window_secs))
}

pub async fn process_chat_request(state: AppState, req: ChatRequest) -> ChatResponse {
    handle_chat(State(state), Json(req)).await.0
}

async fn run_ai_digest_chat_response(request_text: &str) -> ChatResponse {
    let response = match ai_digest::trigger_program_webhook_human_summary(request_text, None).await
    {
        Ok(summary) => summary,
        Err(e) => format!("❌ News Digest 트리거 실패: {}", e),
    };
    ChatResponse {
        response,
        command: Some("ai_digest_program".to_string()),
        route_meta: None,
    }
}

async fn handle_chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Json<ChatResponse> {
    let memory_scope = chat_request_memory_scope(&req);
    let memory_scope_ref = memory_scope.as_deref();
    let gate = crate::chat_gate::ChatGateConfig::from_env();
    let gate_ctx = crate::chat_gate::ChatGateContext {
        channel: req.channel.clone(),
        chat_type: req.chat_type.clone(),
        sender: req.sender.clone(),
        mentioned: req.mentioned,
    };
    if !gate.is_allowed(&gate_ctx) {
        return respond_chat(
            &req,
            memory_scope_ref,
            &req.message,
            ChatResponse {
                response: "⛔️ 이 채널에서는 현재 요청을 처리할 수 없습니다.".to_string(),
                command: None,
                route_meta: None,
            },
            "gate_blocked",
            "blocked",
            None,
            ChatOpsFlags::default(),
            Some("chat gate policy blocked this channel"),
        );
    }

    let sanitized = chat_sanitize::sanitize_chat_input(&req.message);
    if !sanitized.flags.is_empty() {
        eprintln!("⚠️ Chat sanitize flags: {:?}", sanitized.flags);
    }
    let mut message = sanitized.text.trim().to_string();
    if message.is_empty() {
        return respond_chat(
            &req,
            memory_scope_ref,
            &req.message,
            ChatResponse {
                response: "❓ 메시지가 비어있어요. 다시 입력해주세요.".to_string(),
                command: None,
                route_meta: None,
            },
            "empty_message",
            "error",
            None,
            ChatOpsFlags::default(),
            Some("sanitization removed all content"),
        );
    }

    // [Memory] Save User Message
    if let Err(e) = db::insert_chat_message("user", &message) {
        eprintln!("Failed to save user chat: {}", e);
    }
    let mut message_lc = message.to_lowercase();

    // 1. Intercept explicit system commands (Bypass LLM)
    if message_lc == "analyze_patterns" || message == "패턴 분석" {
        let results = run_analysis_internal();
        let response_text = if results.is_empty() {
            "🔍 분석 완료! 새로운 패턴을 찾지 못했습니다.\n(하지만 시연을 위해 데모 항목을 생성했습니다. 오른쪽을 확인하세요!)".to_string()
        } else {
            format!(
                "🔍 분석 완료! {}개의 패턴을 찾았습니다:\n{}",
                results.len(),
                results.join("\n")
            )
        };

        return respond_chat(
            &req,
            memory_scope_ref,
            &message,
            ChatResponse {
                response: response_text,
                command: Some("analyze_patterns".to_string()),
                route_meta: None,
            },
            "system_command",
            "success",
            Some(1.0),
            ChatOpsFlags {
                local_only: true,
                ..Default::default()
            },
            None,
        );
    }

    if message_lc == "telegram listener status"
        || message_lc == "telegram_listen status"
        || message_lc == "telegram-listen status"
        || message_lc == "telegram status"
        || message.contains("텔레그램 리스너 상태")
    {
        let active = telegram_listener_started_flag().load(Ordering::SeqCst);
        return respond_chat(
            &req,
            memory_scope_ref,
            &message,
            ChatResponse {
                response: if active {
                    "🤖 Telegram listener 상태: running".to_string()
                } else {
                    "⚪️ Telegram listener 상태: stopped".to_string()
                },
                command: Some("telegram_listener_status".to_string()),
                route_meta: None,
            },
            "system_command",
            "success",
            Some(1.0),
            ChatOpsFlags {
                local_only: true,
                ..Default::default()
            },
            None,
        );
    }

    if message_lc == "telegram_listen"
        || message_lc == "telegram-listen"
        || message_lc == "telegram listener start"
        || message_lc == "telegram listen"
        || message.contains("텔레그램 리스너 시작")
    {
        let started_flag = telegram_listener_started_flag();
        if started_flag.load(Ordering::SeqCst) {
            return respond_chat(
                &req,
                memory_scope_ref,
                &message,
                ChatResponse {
                    response: "ℹ️ Telegram listener가 이미 실행 중입니다.".to_string(),
                    command: Some("telegram_listener_start".to_string()),
                    route_meta: None,
                },
                "system_command",
                "success",
                Some(1.0),
                ChatOpsFlags {
                    local_only: true,
                    ..Default::default()
                },
                None,
            );
        }

        let llm = match state.llm_client.clone() {
            Some(v) => v,
            None => {
                return respond_chat(
                    &req,
                    memory_scope_ref,
                    &message,
                    ChatResponse {
                        response:
                            "❌ LLM 클라이언트가 없어 Telegram listener를 시작할 수 없습니다."
                                .to_string(),
                        command: Some("telegram_listener_start".to_string()),
                        route_meta: None,
                    },
                    "system_command",
                    "error",
                    Some(1.0),
                    ChatOpsFlags {
                        local_only: true,
                        ..Default::default()
                    },
                    Some("telegram listener requires llm client"),
                )
            }
        };
        let response = match try_spawn_telegram_listener(llm) {
            Ok(TelegramListenerStartOutcome::Started) => ChatResponse {
                response: "🤖 Telegram listener 시작됨 (long polling)".to_string(),
                command: Some("telegram_listener_start".to_string()),
                route_meta: None,
            },
            Ok(TelegramListenerStartOutcome::AlreadyRunning) => ChatResponse {
                response: "ℹ️ Telegram listener가 이미 실행 중입니다.".to_string(),
                command: Some("telegram_listener_start".to_string()),
                route_meta: None,
            },
            Err("missing_telegram_token") => ChatResponse {
                response: "❌ TELEGRAM_BOT_TOKEN이 없어 listener를 시작할 수 없습니다.".to_string(),
                command: Some("telegram_listener_start".to_string()),
                route_meta: None,
            },
            Err(_) => ChatResponse {
                response: "❌ Telegram listener 시작 중 알 수 없는 오류가 발생했습니다."
                    .to_string(),
                command: Some("telegram_listener_start".to_string()),
                route_meta: None,
            },
        };
        let outcome = chat_ops_outcome_from_response(&response.response);
        return respond_chat(
            &req,
            memory_scope_ref,
            &message,
            response,
            "system_command",
            outcome,
            Some(1.0),
            ChatOpsFlags {
                local_only: true,
                ..Default::default()
            },
            None,
        );
    }

    // Issue #4 Fix: n8n restart command
    if message_lc == "n8n restart" || message.contains("n8n 재시작") {
        let api = match crate::n8n_api::N8nApi::from_env() {
            Ok(v) => v,
            Err(e) => {
                return respond_chat(
                    &req,
                    memory_scope_ref,
                    &message,
                    ChatResponse {
                        response: format!("❌ n8n 초기화 실패: {}", e),
                        command: None,
                        route_meta: None,
                    },
                    "system_command",
                    "error",
                    Some(1.0),
                    ChatOpsFlags {
                        local_only: true,
                        ..Default::default()
                    },
                    Some("n8n api init failed"),
                )
            }
        };
        match api.restart_server().await {
            Ok(_) => {
                return respond_chat(
                    &req,
                    memory_scope_ref,
                    &message,
                    ChatResponse {
                        response: "🔄 n8n 서버를 재시작했습니다.".to_string(),
                        command: Some("n8n_restart".to_string()),
                        route_meta: None,
                    },
                    "system_command",
                    "success",
                    Some(1.0),
                    ChatOpsFlags {
                        local_only: true,
                        ..Default::default()
                    },
                    None,
                )
            }
            Err(e) => {
                return respond_chat(
                    &req,
                    memory_scope_ref,
                    &message,
                    ChatResponse {
                        response: format!("❌ n8n 재시작 실패: {}", e),
                        command: None,
                        route_meta: None,
                    },
                    "system_command",
                    "error",
                    Some(1.0),
                    ChatOpsFlags {
                        local_only: true,
                        ..Default::default()
                    },
                    Some("n8n restart failed"),
                )
            }
        }
    }

    let ai_digest_route = ai_digest::infer_program_route(&message, req.channel.as_deref());
    if let Some(route) = ai_digest_route
        .as_ref()
        .filter(|route| route.kind == ai_digest::AiDigestProgramRouteKind::Explicit)
    {
        let response = run_ai_digest_chat_response(&route.request_text).await;
        let outcome = chat_ops_outcome_from_response(&response.response);
        return respond_chat(
            &req,
            memory_scope_ref,
            &message,
            response,
            "ai_digest_explicit",
            outcome,
            Some(1.0),
            ChatOpsFlags {
                ai_digest_used: true,
                ..Default::default()
            },
            None,
        );
    }
    let auto_ai_digest_request = ai_digest_route.and_then(|route| {
        (route.kind == ai_digest::AiDigestProgramRouteKind::Auto).then_some(route.request_text)
    });
    message = ai_digest::strip_local_execution_prefix(&message);
    message_lc = message.to_lowercase();

    // 1.5 Local lightweight chat commands (LLM-free).
    // Keep these commands available even when OpenAI quota/network is unavailable.
    if message_lc == "help" || message == "도움말" || message == "명령어" {
        if let Some(cached) =
            load_local_cached_chat_response(memory_scope_ref, &message, "help_local")
        {
            return respond_chat(
                &req,
                memory_scope_ref,
                &message,
                cached.0,
                "request_memory",
                "success",
                Some(1.0),
                ChatOpsFlags {
                    request_memory_hit: true,
                    local_only: true,
                    ..Default::default()
                },
                Some("local help response cache hit"),
            );
        }
        return respond_chat(
            &req,
            memory_scope_ref,
            &message,
            build_local_chat_response(
            memory_scope_ref,
            &message,
            "help_local",
            "💡 바로 실행 가능한 명령:\n• analyze_patterns / 패턴 분석\n• n8n restart / n8n 재시작\n• telegram listener start / 텔레그램 리스너 시작\n• telegram listener status / 텔레그램 리스너 상태\n• 뉴스 5개 요약해서 노션에 정리해줘\n• /n8n 스포츠 뉴스 5개 요약해서 노션에 정리해줘 (명시 digest)\n• /local 메모장 열고 체크리스트 작성해줘 (로컬 실행 강제)\n• /capture (화면 분석)\n• system_status / 시스템 상태".to_string(),
            "api.chat.local",
        ).0,
            "local_command",
            "success",
            Some(1.0),
            ChatOpsFlags {
                local_only: true,
                ..Default::default()
            },
            None,
        );
    }
    if message == "안녕" || message_lc == "hello" || message_lc == "hi" {
        if let Some(cached) =
            load_local_cached_chat_response(memory_scope_ref, &message, "greeting_local")
        {
            return respond_chat(
                &req,
                memory_scope_ref,
                &message,
                cached.0,
                "request_memory",
                "success",
                Some(1.0),
                ChatOpsFlags {
                    request_memory_hit: true,
                    local_only: true,
                    ..Default::default()
                },
                Some("local greeting cache hit"),
            );
        }
        return respond_chat(
            &req,
            memory_scope_ref,
            &message,
            build_local_chat_response(
            memory_scope_ref,
            &message,
            "greeting_local",
            "👋 안녕하세요! 간단 작업은 바로 실행할 수 있어요.\n원하면 `패턴 분석` 또는 `시스템 상태`라고 입력해보세요.".to_string(),
            "api.chat.local",
        ).0,
            "local_command",
            "success",
            Some(1.0),
            ChatOpsFlags {
                local_only: true,
                ..Default::default()
            },
            None,
        );
    }
    if message == "야"
        || message == "야!"
        || message == "야?"
        || message_lc == "hey"
        || message_lc == "yo"
    {
        if let Some(cached) =
            load_local_cached_chat_response(memory_scope_ref, &message, "greeting_local")
        {
            return respond_chat(
                &req,
                memory_scope_ref,
                &message,
                cached.0,
                "request_memory",
                "success",
                Some(1.0),
                ChatOpsFlags {
                    request_memory_hit: true,
                    local_only: true,
                    ..Default::default()
                },
                Some("local greeting cache hit"),
            );
        }
        return respond_chat(
            &req,
            memory_scope_ref,
            &message,
            build_local_chat_response(
            memory_scope_ref,
            &message,
            "greeting_local",
            "응, 듣고 있어요. 바로 할 일을 말해줘.\n예: `오늘 일정 보여줘`, `패턴 분석`, `n8n 열어줘`".to_string(),
            "api.chat.local",
        ).0,
            "local_command",
            "success",
            Some(1.0),
            ChatOpsFlags {
                local_only: true,
                ..Default::default()
            },
            None,
        );
    }
    if message_lc == "system_status" || message == "시스템 상태" || message == "코어 상태"
    {
        if let Some(cached) =
            load_local_cached_chat_response(memory_scope_ref, &message, "system_status")
        {
            return respond_chat(
                &req,
                memory_scope_ref,
                &message,
                cached.0,
                "request_memory",
                "success",
                Some(1.0),
                ChatOpsFlags {
                    request_memory_hit: true,
                    local_only: true,
                    ..Default::default()
                },
                Some("local system status cache hit"),
            );
        }
        let mut rm = monitor::ResourceMonitor::new();
        return respond_chat(
            &req,
            memory_scope_ref,
            &message,
            build_local_chat_response(
                memory_scope_ref,
                &message,
                "system_status",
                format!("📊 {}", rm.get_status()),
                "api.chat.local",
            )
            .0,
            "local_command",
            "success",
            Some(1.0),
            ChatOpsFlags {
                local_only: true,
                ..Default::default()
            },
            None,
        );
    }

    // 2. Vision Command (Explicit Only)
    // Prevent accidental capture on "screen" mention. Require explicit command.
    if message.trim() == "/capture"
        || message.contains("화면 분석해줘")
        || message.contains("analyze screen")
    {
        if let Some(llm) = &state.llm_client {
            match crate::visual_driver::VisualDriver::capture_screen() {
                Ok((b64, _scale)) => {
                    let prompt = "Describe what is on the user's screen briefly. Identify active applications and context.";
                    match llm.analyze_screen(prompt, &b64).await {
                        Ok(desc) => {
                            return respond_chat(
                                &req,
                                memory_scope_ref,
                                &message,
                                ChatResponse {
                                    response: format!("👁️ 화면 분석 결과:\n{}", desc),
                                    command: None,
                                    route_meta: None,
                                },
                                "vision_command",
                                "success",
                                Some(1.0),
                                ChatOpsFlags::default(),
                                None,
                            )
                        }
                        Err(e) => {
                            return respond_chat(
                                &req,
                                memory_scope_ref,
                                &message,
                                ChatResponse {
                                    response: format!("❌ Vision API 오류: {}", e),
                                    command: None,
                                    route_meta: None,
                                },
                                "vision_command",
                                "error",
                                Some(1.0),
                                ChatOpsFlags::default(),
                                Some("vision model analysis failed"),
                            )
                        }
                    }
                }
                Err(e) => {
                    return respond_chat(
                        &req,
                        memory_scope_ref,
                        &message,
                        ChatResponse {
                            response: format!("❌ 화면 캡처 실패: {}", e),
                            command: None,
                            route_meta: None,
                        },
                        "vision_command",
                        "error",
                        Some(1.0),
                        ChatOpsFlags::default(),
                        Some("screen capture failed"),
                    )
                }
            }
        } else {
            return respond_chat(
                &req,
                memory_scope_ref,
                &message,
                ChatResponse {
                    response: "❌ LLM 클라이언트가 초기화되지 않았습니다.".to_string(),
                    command: None,
                    route_meta: None,
                },
                "vision_command",
                "error",
                None,
                ChatOpsFlags::default(),
                Some("vision command requires llm client"),
            );
        }
    }

    // 3. Demo Vision Workflow Trigger
    if message == "demo_vision" {
        if let Some(llm) = &state.llm_client {
            let llm_clone = llm.clone(); // Clone for async task

            tokio::spawn(async move {
                let mut driver = crate::visual_driver::VisualDriver::new();
                use crate::visual_driver::{SmartStep, UiAction};

                // Step 1: Open Google
                driver.add_step(
                    SmartStep::new(
                        UiAction::OpenUrl("https://www.google.com".to_string()),
                        "Open Google",
                    )
                    .with_post_check("Is the Google search homepage visible?"),
                );

                // Step 2: Wait for load
                driver.add_step(SmartStep::new(UiAction::Wait(3), "Wait for Load"));

                // Step 3: Type 'Hello World'
                // Pre-check: Ensure search bar exists
                // Post-check: Ensure text appears
                driver.add_step(
                    SmartStep::new(
                        UiAction::Type("Hello World".to_string()),
                        "Type Search Query",
                    )
                    .with_pre_check("Is there a search input field visible?")
                    .with_post_check("Is the text 'Hello World' visible in the search bar?"),
                );

                if let Err(e) = driver.execute(Some(llm_clone.as_ref())).await {
                    eprintln!("❌ Vision Demo Failed: {}", e);
                } else {
                    println!("✅ Vision Demo Completed Successfully.");
                }
            });

            return respond_chat(
                &req,
                memory_scope_ref,
                &message,
                ChatResponse {
                    response: "🚀 Vision 검증 데모(Smart Mode)를 시작합니다.\n(Google 접속 -> [검증] -> 키 입력 -> [검증])".to_string(),
                    command: None,
                route_meta: None,
                },
                "vision_demo",
                "success",
                Some(1.0),
                ChatOpsFlags::default(),
                None,
            );
        }
    }

    let (intent, confidence, ops_route_kind, mut ops_flags) = if let Some((
        cached_intent,
        cached_confidence,
    )) =
        load_cached_request_intent(memory_scope_ref, &message)
    {
        (
            cached_intent,
            cached_confidence,
            "intent_memory",
            ChatOpsFlags {
                intent_memory_hit: true,
                ..Default::default()
            },
        )
    } else if let Some((deterministic_intent, deterministic_confidence)) =
        load_deterministic_chat_intent(&message)
    {
        (
            deterministic_intent,
            deterministic_confidence,
            "deterministic",
            ChatOpsFlags {
                deterministic_used: true,
                ..Default::default()
            },
        )
    } else {
        let Some(brain) = &state.llm_client else {
            if let Some(request_text) = auto_ai_digest_request.as_deref() {
                let response = run_ai_digest_chat_response(request_text).await;
                let outcome = chat_ops_outcome_from_response(&response.response);
                return respond_chat(
                    &req,
                    memory_scope_ref,
                    &message,
                    response,
                    "ai_digest_auto",
                    outcome,
                    None,
                    ChatOpsFlags {
                        ai_digest_used: true,
                        ..Default::default()
                    },
                    Some("llm unavailable, used digest auto fallback"),
                );
            }
            return respond_chat(
                &req,
                memory_scope_ref,
                &message,
                ChatResponse {
                    response: "⚠️ LLM 클라이언트가 없습니다.".to_string(),
                    command: None,
                    route_meta: None,
                },
                "llm_unavailable",
                "error",
                None,
                ChatOpsFlags::default(),
                None,
            );
        };
        let history =
            db::get_recent_chat_history(context_pruning::history_fetch_limit()).unwrap_or_default();
        match brain.parse_intent_with_history(&message, &history).await {
            Ok(intent) => {
                let confidence = intent["confidence"].as_f64().unwrap_or(0.0);
                (
                    intent,
                    confidence,
                    "llm",
                    ChatOpsFlags {
                        llm_used: true,
                        ..Default::default()
                    },
                )
            }
            Err(e) => {
                if let Some(request_text) = auto_ai_digest_request.as_deref() {
                    let response = run_ai_digest_chat_response(request_text).await;
                    let outcome = chat_ops_outcome_from_response(&response.response);
                    return respond_chat(
                        &req,
                        memory_scope_ref,
                        &message,
                        response,
                        "ai_digest_auto",
                        outcome,
                        None,
                        ChatOpsFlags {
                            llm_used: true,
                            ai_digest_used: true,
                            ..Default::default()
                        },
                        Some("llm parse failed, used digest auto fallback"),
                    );
                }
                let err_text = e.to_string();
                let response = if err_text.to_lowercase().contains("insufficient_quota") {
                    "⚠️ OpenAI 사용량 한도를 초과했습니다. 잠시 후 다시 시도하거나 API 키/요금제를 확인해주세요.\n\n지금도 가능한 로컬 명령:\n• 패턴 분석\n• 시스템 상태\n• n8n 재시작".to_string()
                } else {
                    format!("❌ 오류: {}", err_text)
                };
                return respond_chat(
                    &req,
                    memory_scope_ref,
                    &message,
                    ChatResponse {
                        response,
                        command: None,
                        route_meta: None,
                    },
                    "llm_error",
                    "error",
                    None,
                    ChatOpsFlags {
                        llm_used: true,
                        ..Default::default()
                    },
                    Some("llm parse_intent_with_history failed"),
                );
            }
        }
    };

    let command = intent["command"].as_str().unwrap_or("unknown").to_string();
    ops_flags.freshness_bypassed = request_prefers_fresh_data(&message, &command)
        || request_recently_reasked(memory_scope_ref, &message, &command)
        || execution_recently_reasked(memory_scope_ref, &command, &intent);

    if confidence < 0.5 {
        if let Some(request_text) = auto_ai_digest_request.as_deref() {
            let mut flags = ops_flags;
            flags.ai_digest_used = true;
            let response = run_ai_digest_chat_response(request_text).await;
            let outcome = chat_ops_outcome_from_response(&response.response);
            return respond_chat(
                &req,
                memory_scope_ref,
                &message,
                response,
                "ai_digest_auto",
                outcome,
                Some(confidence),
                flags,
                Some("low confidence route used digest auto fallback"),
            );
        }
        return respond_chat(
            &req,
            memory_scope_ref,
            &message,
            ChatResponse {
                response: "❓ 무슨 말인지 잘 모르겠어요. 다시 말씀해주세요.".to_string(),
                command: None,
                route_meta: None,
            },
            "low_confidence",
            "error",
            Some(confidence),
            ops_flags,
            None,
        );
    }

    if let Some(cached_response) =
        load_cached_execution_response(memory_scope_ref, &message, &command, &intent)
    {
        if let Err(e) = db::insert_chat_message("assistant", &cached_response) {
            eprintln!("Failed to save cached execution chat: {}", e);
        }
        let _ = db::upsert_request_memory_scoped(
            memory_scope_ref,
            &message,
            Some(&intent),
            Some(&cached_response),
            request_memory_response_mode(&command),
            "api.chat.execution_cache",
            confidence,
        );
        let mut flags = ops_flags;
        flags.execution_memory_hit = true;
        return respond_chat(
            &req,
            memory_scope_ref,
            &message,
            ChatResponse {
                response: cached_response,
                command: if command == "unknown" {
                    None
                } else {
                    Some(command)
                },
                route_meta: None,
            },
            "execution_memory",
            "success",
            Some(confidence),
            flags,
            None,
        );
    }

    if let Some(cached_response) =
        load_cached_request_response(memory_scope_ref, &message, &command)
    {
        if let Err(e) = db::insert_chat_message("assistant", &cached_response) {
            eprintln!("Failed to save cached assistant chat: {}", e);
        }
        let _ = db::upsert_request_memory_scoped(
            memory_scope_ref,
            &message,
            Some(&intent),
            Some(&cached_response),
            request_memory_response_mode(&command),
            "api.chat.response_cache",
            confidence,
        );
        let mut flags = ops_flags;
        flags.request_memory_hit = true;
        return respond_chat(
            &req,
            memory_scope_ref,
            &message,
            ChatResponse {
                response: cached_response,
                command: if command == "unknown" {
                    None
                } else {
                    Some(command)
                },
                route_meta: None,
            },
            "request_memory",
            "success",
            Some(confidence),
            flags,
            None,
        );
    }

    if command == "unknown" {
        if let Some(request_text) = auto_ai_digest_request.as_deref() {
            let mut flags = ops_flags;
            flags.ai_digest_used = true;
            let response = run_ai_digest_chat_response(request_text).await;
            let outcome = chat_ops_outcome_from_response(&response.response);
            return respond_chat(
                &req,
                memory_scope_ref,
                &message,
                response,
                "ai_digest_auto",
                outcome,
                Some(confidence),
                flags,
                Some("unknown command route used digest auto fallback"),
            );
        }
    }

    let response = match command.as_str() {
        "analyze_patterns" => {
            let results = run_analysis_internal();
            if results.is_empty() {
                "🔍 분석 완료! 새로운 패턴을 찾지 못했습니다.".to_string()
            } else {
                format!("🔍 분석 완료!:\n{}", results.join("\n"))
            }
        }
        "gmail_list" => {
            let count = parse_u32_param(intent["params"].get("count"), 5, 1, 20);
            match integrations::gmail::GmailClient::new().await {
                Ok(client) => match client.list_messages(count).await {
                    Ok(msgs) => {
                        if msgs.is_empty() {
                            "📭 새 메일이 없습니다.".to_string()
                        } else {
                            let mut s = format!("📧 최근 이메일 {}건:\n", count);
                            for (_, subj, from) in msgs {
                                s.push_str(&format!("• {} ({})\n", subj, from));
                            }
                            s
                        }
                    }
                    Err(e) => format!("❌ 이메일 가져오기 실패: {}", e),
                },
                Err(e) => format!("⚠️ Gmail 인증 실패: {}", e),
            }
        }
        "calendar_today" => match integrations::calendar::CalendarClient::new().await {
            Ok(client) => match client.list_today().await {
                Ok(events) => {
                    if events.is_empty() {
                        "📅 오늘 일정이 없습니다.".to_string()
                    } else {
                        let mut s = String::from("📅 오늘 일정:\n");
                        for (_, summary, start_time) in events {
                            s.push_str(&format!("• {} ({})\n", summary, start_time));
                        }
                        s
                    }
                }
                Err(e) => format!("❌ 일정 확인 실패: {}", e),
            },
            Err(e) => format!("⚠️ Calendar 인증 실패: {}", e),
        },
        "calendar_week" => match integrations::calendar::CalendarClient::new().await {
            Ok(client) => match client.list_week().await {
                Ok(events) => {
                    if events.is_empty() {
                        "📅 이번 주 일정이 없습니다.".to_string()
                    } else {
                        let mut s = String::from("📅 이번 주 일정:\n");
                        for (_, summary, start_time) in events {
                            s.push_str(&format!("• {} ({})\n", summary, start_time));
                        }
                        s
                    }
                }
                Err(e) => format!("❌ 일정 확인 실패: {}", e),
            },
            Err(e) => format!("⚠️ Calendar 인증 실패: {}", e),
        },
        "system_status" => {
            let mut rm = monitor::ResourceMonitor::new();
            format!("📊 {}", rm.get_status())
        }
        "build_workflow" => {
            let prompt_str = intent["params"]["prompt"]
                .as_str()
                .or_else(|| intent["params"]["description"].as_str())
                .unwrap_or(&message);
            match workflow_intake::queue_manual_workflow_recommendation(
                prompt_str,
                "api.chat.build_workflow",
            ) {
                Ok(outcome) => {
                    let rec_id = outcome.recommendation_id;
                    let inserted = outcome.inserted;
                    let queued = if inserted { "생성" } else { "재사용" };
                    format!(
                        "📝 워크플로우 제안을 {}했습니다 (ID: {}).\n승인 게이트 정책상 즉시 생성은 차단되며, `/api/recommendations/{}/approve` 또는 CLI `approve {}`로 승인 후 생성됩니다.",
                        queued, rec_id, rec_id, rec_id
                    )
                }
                Err(e) => format!("❌ 워크플로우 제안 저장 실패: {}", e),
            }
        }
        "create_routine" => {
            let params = intent["params"].as_object();
            if let Some(p) = params {
                let name = p
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("New Routine");
                let cron = p
                    .get("cron")
                    .and_then(|v| v.as_str())
                    .unwrap_or("* * * * *");

                if std::str::FromStr::from_str(cron as &str)
                    .map(|_: cron::Schedule| ())
                    .is_err()
                {
                    format!("❌ 잘못된 Cron 표현식입니다: {}", cron)
                } else {
                    let prompt = p
                        .get("prompt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Check status");

                    match crate::db::create_routine(name, cron, prompt) {
                        Ok(_id) => format!(
                            "✅ 루틴이 등록되었습니다!\n• 이름: {}\n• 주기: {}\n• 명령: {}",
                            name, cron, prompt
                        ),
                        Err(e) => format!("❌ 루틴 등록 실패: {}", e),
                    }
                }
            } else {
                "❌ 루틴 정보를 파악할 수 없습니다.".to_string()
            }
        }
        "help" => "💡 사용 가능한 명령:\n• '이메일 보여줘'\n• '오늘 일정 뭐야?'\n• '매일 아침 9시 뉴스 요약해줘' (New!)".to_string(),
        _ => "🤔 요청을 정확히 해석하지 못했어요.\n원하는 작업을 한 문장으로 더 구체적으로 말해줘.\n예: `오늘 일정 보여줘`, `최근 메일 5개 요약해줘`, `n8n 열어줘`".to_string(),
    };

    let response_command = if command == "unknown" {
        None
    } else {
        Some(command.clone())
    };

    let final_response = ChatResponse {
        response: response.clone(),
        command: response_command,
        route_meta: None,
    };

    if let Err(e) = db::insert_chat_message("assistant", &response) {
        eprintln!("Failed to save AI chat: {}", e);
    }

    persist_execution_memory(
        memory_scope_ref,
        &message,
        &command,
        &intent,
        &response,
        "api.chat.execution",
    );

    if request_memory_should_store(&command, confidence, &response) {
        let _ = db::upsert_request_memory_scoped(
            memory_scope_ref,
            &message,
            Some(&intent),
            Some(&response),
            request_memory_response_mode(&command),
            chat_response_memory_source(&intent),
            confidence,
        );
    }

    let route_kind = if command == "unknown" {
        "unknown"
    } else {
        ops_route_kind
    };
    let outcome = chat_ops_outcome_from_response(&response);
    respond_chat(
        &req,
        memory_scope_ref,
        &message,
        final_response,
        route_kind,
        outcome,
        Some(confidence),
        ops_flags,
        None,
    )
}

// --- Routine Handlers ---

#[derive(serde::Deserialize)]
struct IngestCollectorHandoffRequest {
    config_path: Option<String>,
}

#[derive(serde::Serialize)]
struct IngestCollectorHandoffResponse {
    status: String,
    detail: String,
    package_id: Option<String>,
    recommendation_id: Option<i64>,
    inserted: bool,
}

async fn ingest_collector_handoff_handler(
    Json(payload): Json<IngestCollectorHandoffRequest>,
) -> impl IntoResponse {
    match workflow_intake::ingest_latest_collector_handoff(payload.config_path.as_deref()) {
        Ok(outcome) => (
            StatusCode::OK,
            Json(IngestCollectorHandoffResponse {
                status: outcome.status,
                detail: outcome.detail,
                package_id: outcome.package_id,
                recommendation_id: outcome.recommendation_id,
                inserted: outcome.inserted,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(IngestCollectorHandoffResponse {
                status: "error".to_string(),
                detail: e.to_string(),
                package_id: None,
                recommendation_id: None,
                inserted: false,
            }),
        ),
    }
}

async fn list_routines() -> Json<Vec<crate::db::Routine>> {
    match crate::db::get_all_routines() {
        Ok(routines) => Json(routines),
        Err(e) => {
            eprintln!("Failed to list routines: {}", e);
            Json(Vec::new())
        }
    }
}

#[derive(serde::Deserialize)]
struct CreateRoutineRequest {
    name: String,
    #[serde(alias = "cron_expression")] // Accept both "cron" and "cron_expression"
    cron: String,
    prompt: String,
}

async fn create_routine_handler(
    Json(payload): Json<CreateRoutineRequest>,
) -> Json<serde_json::Value> {
    match crate::db::create_routine(&payload.name, &payload.cron, &payload.prompt) {
        Ok(id) => Json(serde_json::json!({ "status": "ok", "id": id })),
        Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
    }
}

// --- Issue #2 Fix: Toggle Routine ---
#[derive(serde::Deserialize)]
struct ToggleRoutineRequest {
    enabled: bool,
}

async fn toggle_routine_handler(
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(payload): Json<ToggleRoutineRequest>,
) -> Json<serde_json::Value> {
    match crate::db::toggle_routine(id, payload.enabled) {
        Ok(_) => Json(serde_json::json!({ "status": "ok" })),
        Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
    }
}

#[derive(serde::Deserialize)]
struct RecQueryParams {
    status: Option<String>,
    category: Option<String>,
}

fn n8n_editor_base_url() -> String {
    let normalize = |input: &str| {
        let mut base = input.trim().trim_end_matches('/').to_string();
        for (from, to) in [
            ("http://127.0.0.1", "http://localhost"),
            ("https://127.0.0.1", "https://localhost"),
            ("http://0.0.0.0", "http://localhost"),
            ("https://0.0.0.0", "https://localhost"),
            ("http://[::1]", "http://localhost"),
            ("https://[::1]", "https://localhost"),
        ] {
            if base.starts_with(from) {
                base = base.replacen(from, to, 1);
                break;
            }
        }
        base
    };

    if let Ok(editor) = std::env::var("N8N_EDITOR_URL") {
        let trimmed = editor.trim().trim_end_matches('/');
        if !trimmed.is_empty() {
            return normalize(trimmed);
        }
    }

    let api = std::env::var("STEER_N8N_API_URL")
        .or_else(|_| std::env::var("N8N_API_URL"))
        .ok()
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "http://localhost:5678/api/v1".to_string());

    for suffix in ["/api/v1", "/api/v2", "/api"] {
        if let Some(stripped) = api.strip_suffix(suffix) {
            let candidate = stripped.trim_end_matches('/');
            if !candidate.is_empty() {
                return normalize(candidate);
            }
        }
    }

    normalize(&api)
}

fn workflow_editor_url(workflow_id: &str) -> Option<String> {
    let trimmed = workflow_id.trim();
    if trimmed.is_empty() || trimmed.starts_with("provisioning:") {
        return None;
    }
    Some(format!("{}/workflow/{}", n8n_editor_base_url(), trimmed))
}

fn auto_recommendation_allowed(
    proposal: &crate::recommendation::AutomationProposal,
    source: &str,
) -> bool {
    let history_limit = crate::recommendation_policy::auto_recommendation_history_limit();
    let recent = db::get_recent_recommendations(history_limit).unwrap_or_default();
    let decision = crate::recommendation_policy::admit_auto_recommendation(proposal, &recent);
    if decision.accepted {
        return true;
    }

    eprintln!(
        "🧹 [{}] Suppressing auto recommendation: {} [{} / {} / {:.2}] {}",
        source,
        proposal.title,
        decision.pending_same_category,
        decision.pending_limit,
        decision.priority_score,
        decision.reasons.join(", ")
    );
    false
}

fn recommendation_effective_category_and_score(rec: &db::Recommendation) -> (String, f64) {
    let derived = crate::recommendation_policy::classify_recommendation_record(rec);
    let effective_category = if rec.category.trim().is_empty()
        || rec
            .category
            .eq_ignore_ascii_case(crate::recommendation_policy::CATEGORY_UNKNOWN)
    {
        derived.category
    } else {
        rec.category.clone()
    };
    let effective_business_score = if rec.business_score > 0.0 {
        rec.business_score
    } else {
        derived.business_score
    };
    (effective_category, effective_business_score)
}

fn recommendation_is_snoozed(rec: &db::Recommendation) -> bool {
    let Some(snoozed_until) = rec
        .snoozed_until
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return false;
    };
    chrono::DateTime::parse_from_rfc3339(snoozed_until)
        .map(|ts| ts.with_timezone(&chrono::Utc) > chrono::Utc::now())
        .unwrap_or(false)
}

fn recommendation_effective_status(rec: &db::Recommendation) -> String {
    if rec.status.eq_ignore_ascii_case("pending")
        && rec
            .last_error
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_some()
    {
        "failed".to_string()
    } else if rec.status.eq_ignore_ascii_case("pending") && recommendation_is_snoozed(rec) {
        "later".to_string()
    } else {
        rec.status.clone()
    }
}

fn recommendation_approval_block_message(id: i64, reasons: &[String]) -> String {
    if reasons.is_empty() {
        return format!("Recommendation {} is not ready for approval yet.", id);
    }
    format!(
        "Recommendation {} needs stronger business evidence before approval: {}",
        id,
        reasons.join(" ")
    )
}

fn recommendation_review_actor(actor: Option<&str>, fallback: &str) -> String {
    actor
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn log_recommendation_review_event(
    recommendation_id: i64,
    recommendation_title: &str,
    status_after: Option<&str>,
    category: Option<&str>,
    action: &str,
    actor: Option<&str>,
    note: Option<&str>,
    ok: bool,
    message: Option<&str>,
) {
    let _ = db::record_recommendation_review_event(
        recommendation_id,
        recommendation_title,
        status_after,
        category,
        action,
        actor,
        note,
        ok,
        message,
    );
}

fn recommendation_feedback_snooze_hours() -> i64 {
    std::env::var("ALLVIA_RECOMMENDATION_REFINE_SNOOZE_HOURS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .map(|v| v.clamp(1, 24 * 30))
        .unwrap_or(24 * 14)
}

fn classify_recommendation_feedback(feedback: &str) -> &'static str {
    let normalized = feedback.trim().to_lowercase();
    if normalized.is_empty() {
        return "refine";
    }

    let has_negative_signal = [
        "싫",
        "별로",
        "원치",
        "필요없",
        "아냐",
        "아님",
        "잘못",
        "틀렸",
        "노이즈",
        "중복",
        "spam",
        "duplicate",
        "irrelevant",
        "wrong",
        "not useful",
        "don't",
        "do not",
        "no need",
    ]
    .iter()
    .any(|token| normalized.contains(token));
    if has_negative_signal {
        return "negative";
    }

    let has_positive_signal = [
        "좋",
        "괜찮",
        "유용",
        "원하던",
        "마음에",
        "keep",
        "good",
        "great",
        "helpful",
        "useful",
        "correct",
    ]
    .iter()
    .any(|token| normalized.contains(token));
    if has_positive_signal {
        return "positive";
    }

    "refine"
}

fn limit_visible_recommendations(
    recs: Vec<db::Recommendation>,
    category_filter: Option<&str>,
    preference_history: &[db::Recommendation],
) -> Vec<db::Recommendation> {
    let limit = crate::recommendation_policy::pending_recommendation_display_limit();
    let preference_profile =
        crate::recommendation_policy::build_recommendation_preference_profile(preference_history);
    if limit == 0 {
        return recs
            .into_iter()
            .filter(|rec| {
                let (category, _) = recommendation_effective_category_and_score(rec);
                crate::recommendation_policy::matches_category_filter(&category, category_filter)
            })
            .collect();
    }

    let mut manual_pending = Vec::new();
    let mut auto_pending = Vec::new();
    let mut others = Vec::new();

    for rec in recs {
        let (effective_category, _) = recommendation_effective_category_and_score(&rec);
        if !crate::recommendation_policy::matches_category_filter(
            &effective_category,
            category_filter,
        ) {
            continue;
        }
        let effective_status = recommendation_effective_status(&rec);
        if effective_status.eq_ignore_ascii_case("pending") {
            if rec.pattern_id.is_some() {
                let readiness =
                    crate::recommendation_policy::evaluate_recommendation_approval_readiness(&rec);
                if !readiness.ready {
                    continue;
                }
                auto_pending.push(rec);
            } else {
                manual_pending.push(rec);
            }
        } else {
            others.push(rec);
        }
    }

    auto_pending.sort_by(|left, right| {
        let right_ready =
            crate::recommendation_policy::evaluate_recommendation_approval_readiness(right).ready;
        let left_ready =
            crate::recommendation_policy::evaluate_recommendation_approval_readiness(left).ready;
        right_ready.cmp(&left_ready).then_with(|| {
            crate::recommendation_policy::recommendation_record_priority_score_with_profile(
                right,
                &preference_profile,
            )
            .partial_cmp(
                &crate::recommendation_policy::recommendation_record_priority_score_with_profile(
                    left,
                    &preference_profile,
                ),
            )
            .unwrap_or(CmpOrdering::Equal)
        })
    });

    let mut visible = manual_pending;
    visible.extend(auto_pending.into_iter().take(limit));
    visible.extend(others);
    visible
}

async fn list_recommendations(
    Query(params): Query<RecQueryParams>,
) -> Json<Vec<RecommendationItem>> {
    let _ = db::reconcile_workflow_provision_ops(50);
    // Treat empty string as None; default to "all" for history view.
    let filter = params
        .status
        .as_deref()
        .filter(|s| !s.is_empty())
        .or(Some("all"));
    let category_filter =
        crate::recommendation_policy::normalize_list_category_filter(params.category.as_deref());

    match db::get_recommendations_with_filter(filter) {
        Ok(recs) => {
            let preference_history = db::get_recent_recommendations(
                crate::recommendation_policy::auto_recommendation_history_limit(),
            )
            .unwrap_or_default();
            Json(
                limit_visible_recommendations(
                    recs,
                    category_filter.as_deref(),
                    &preference_history,
                )
                .into_iter()
                .map(|r| {
                    let (effective_category, effective_business_score) =
                        recommendation_effective_category_and_score(&r);
                    let approval_readiness =
                        crate::recommendation_policy::evaluate_recommendation_approval_readiness(
                            &r,
                        );
                    RecommendationItem {
                        id: r.id,
                        status: recommendation_effective_status(&r),
                        title: r.title,
                        summary: r.summary,
                        confidence: r.confidence,
                        category: effective_category,
                        business_score: effective_business_score,
                        evidence: r.evidence,
                        last_error: r.last_error,
                        workflow_id: r.workflow_id.clone(),
                        workflow_url: r.workflow_id.as_deref().and_then(workflow_editor_url),
                        snoozed_until: r.snoozed_until.clone(),
                        approval_ready: approval_readiness.ready,
                        approval_reasons: approval_readiness.reasons,
                    }
                })
                .collect(),
            )
        }
        Err(_) => Json(vec![]),
    }
}

async fn approve_recommendation(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    payload: Option<Json<RecommendationReviewActionRequest>>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    println!("🔔 Received approval request for Recommendation ID: {}", id);
    // NOTE: keep approve endpoint async-only to avoid frontend request timeout and duplicate submissions.
    // Synchronous approval/provisioning was causing >20s hangs in production-like runs.
    let latest_op_snapshot = |recommendation_id: i64| {
        db::latest_workflow_provision_op(recommendation_id)
            .ok()
            .flatten()
            .map(|op| (op.id, op.status, op.updated_at))
    };
    let actor = recommendation_review_actor(
        payload.as_ref().and_then(|p| p.actor.as_deref()),
        "api.recommendation_review",
    );
    let note = payload.as_ref().and_then(|p| p.note.as_deref());

    let rec = db::get_recommendation(id)
        .map_err(|e| {
            let details = e.to_string();
            log_recommendation_review_event(
                id,
                &format!("recommendation:{}", id),
                Some("error"),
                None,
                "approve",
                Some(actor.as_str()),
                note,
                false,
                Some(&details),
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "recommendation_lookup_failed",
                    "details": details
                })),
            )
        })?
        .ok_or_else(|| {
            log_recommendation_review_event(
                id,
                &format!("recommendation:{}", id),
                Some("missing"),
                None,
                "approve",
                Some(actor.as_str()),
                note,
                false,
                Some("recommendation not found"),
            );
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "recommendation_not_found",
                    "id": id
                })),
            )
        })?;

    if rec.status.eq_ignore_ascii_case("rejected") {
        log_recommendation_review_event(
            rec.id,
            &rec.title,
            Some(rec.status.as_str()),
            Some(rec.category.as_str()),
            "approve",
            Some(actor.as_str()),
            note,
            false,
            Some("recommendation is rejected"),
        );
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "recommendation_rejected",
                "details": format!("recommendation {} is rejected", id),
            })),
        ));
    }

    let approval_readiness =
        crate::recommendation_policy::evaluate_recommendation_approval_readiness(&rec);
    if !approval_readiness.ready {
        let details = recommendation_approval_block_message(id, &approval_readiness.reasons);
        log_recommendation_review_event(
            rec.id,
            &rec.title,
            Some(rec.status.as_str()),
            Some(rec.category.as_str()),
            "approve",
            Some(actor.as_str()),
            note,
            false,
            Some(&details),
        );
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({
                "error": details,
                "details": details,
                "reasons": approval_readiness.reasons,
            })),
        ));
    }

    let approved_now = !rec.status.eq_ignore_ascii_case("approved");
    if approved_now {
        db::update_recommendation_review_status(id, "approved").map_err(|e| {
            let details = e.to_string();
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some(rec.status.as_str()),
                Some(rec.category.as_str()),
                "approve",
                Some(actor.as_str()),
                note,
                false,
                Some(&details),
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "recommendation_approve_update_failed",
                    "details": details,
                })),
            )
        })?;
    }

    if let Some(existing_id) = rec
        .workflow_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if existing_id.starts_with("provisioning:") {
            let latest_op = latest_op_snapshot(id);
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some("approved"),
                Some(rec.category.as_str()),
                "approve",
                Some(actor.as_str()),
                note,
                true,
                Some("workflow provisioning already in progress"),
            );
            return Ok((
                StatusCode::ACCEPTED,
                Json(serde_json::json!({
                    "status": "accepted",
                    "id": serde_json::Value::Null,
                    "workflow_id": serde_json::Value::Null,
                    "workflow_url": serde_json::Value::Null,
                    "provision_op_id": latest_op.as_ref().map(|(op_id, _, _)| *op_id),
                    "provision_status": latest_op.as_ref().map(|(_, status, _)| status.clone()),
                    "provision_updated_at": latest_op.as_ref().map(|(_, _, updated_at)| updated_at.clone()),
                    "provision_claim": existing_id,
                    "approved_now": approved_now,
                    "reused_existing": false,
                    "message": "Workflow provisioning already in progress",
                })),
            ));
        }

        let workflow_url = workflow_editor_url(existing_id);
        log_recommendation_review_event(
            rec.id,
            &rec.title,
            Some("approved"),
            Some(rec.category.as_str()),
            "approve",
            Some(actor.as_str()),
            note,
            true,
            Some("workflow already existed; reused existing workflow_id"),
        );
        return Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "success",
                "id": existing_id,
                "workflow_id": existing_id,
                "workflow_url": workflow_url,
                "provision_op_id": serde_json::Value::Null,
                "provision_status": serde_json::Value::Null,
                "provision_updated_at": serde_json::Value::Null,
                "approved_now": approved_now,
                "reused_existing": true,
                "message": "Workflow already existed; reused existing workflow_id",
            })),
        ));
    }

    let provisioning = match recommendation_executor::precreate_async_provisioning(id) {
        Ok(p) => p,
        Err(error) => {
            let message = error.to_string();
            if message.contains("already being provisioned") {
                let latest_op = latest_op_snapshot(id);
                log_recommendation_review_event(
                    rec.id,
                    &rec.title,
                    Some("approved"),
                    Some(rec.category.as_str()),
                    "approve",
                    Some(actor.as_str()),
                    note,
                    true,
                    Some("workflow provisioning already in progress"),
                );
                return Ok((
                    StatusCode::ACCEPTED,
                    Json(serde_json::json!({
                        "status": "accepted",
                        "id": serde_json::Value::Null,
                        "workflow_id": serde_json::Value::Null,
                        "workflow_url": serde_json::Value::Null,
                        "provision_op_id": latest_op.as_ref().map(|(op_id, _, _)| *op_id),
                        "provision_status": latest_op.as_ref().map(|(_, status, _)| status.clone()),
                        "provision_updated_at": latest_op.as_ref().map(|(_, _, updated_at)| updated_at.clone()),
                        "approved_now": approved_now,
                        "reused_existing": false,
                        "message": "Workflow provisioning already in progress",
                    })),
                ));
            }
            if message.contains("already provisioned") {
                if let Ok(Some(latest_rec)) = db::get_recommendation(id) {
                    if let Some(existing_id) = latest_rec
                        .workflow_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .filter(|s| !s.starts_with("provisioning:"))
                    {
                        let workflow_url = workflow_editor_url(existing_id);
                        log_recommendation_review_event(
                            latest_rec.id,
                            &latest_rec.title,
                            Some("approved"),
                            Some(latest_rec.category.as_str()),
                            "approve",
                            Some(actor.as_str()),
                            note,
                            true,
                            Some("workflow already existed; reused existing workflow_id"),
                        );
                        return Ok((
                            StatusCode::OK,
                            Json(serde_json::json!({
                                "status": "success",
                                "id": existing_id,
                                "workflow_id": existing_id,
                                "workflow_url": workflow_url,
                                "provision_op_id": serde_json::Value::Null,
                                "provision_status": serde_json::Value::Null,
                                "provision_updated_at": serde_json::Value::Null,
                                "approved_now": approved_now,
                                "reused_existing": true,
                                "message": "Workflow already existed; reused existing workflow_id",
                            })),
                        ));
                    }
                }
            }
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some("approved"),
                Some(rec.category.as_str()),
                "approve",
                Some(actor.as_str()),
                note,
                false,
                Some(&message),
            );
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "workflow_provision_prepare_failed",
                    "details": message,
                })),
            ));
        }
    };

    let llm_client = state.llm_client.clone();
    let provisioning_for_spawn = provisioning.clone();
    tokio::spawn(async move {
        match recommendation_executor::execute_approved_recommendation_with_preclaim(
            id,
            llm_client,
            provisioning_for_spawn,
        )
        .await
        {
            Ok(workflow_id) => {
                println!(
                    "✅ Async workflow provisioning completed: recommendation={} workflow_id={}",
                    id, workflow_id
                );
            }
            Err(error) => {
                eprintln!(
                    "⚠️ Async workflow provisioning failed: recommendation={} error={}",
                    id, error
                );
            }
        }
    });

    let latest_op = latest_op_snapshot(id);
    log_recommendation_review_event(
        rec.id,
        &rec.title,
        Some("approved"),
        Some(rec.category.as_str()),
        "approve",
        Some(actor.as_str()),
        note,
        true,
        Some("approval accepted; workflow provisioning running asynchronously"),
    );
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "status": "accepted",
            "id": serde_json::Value::Null,
            "workflow_id": serde_json::Value::Null,
            "workflow_url": serde_json::Value::Null,
            "provision_op_id": latest_op
                .as_ref()
                .map(|(op_id, _, _)| *op_id)
                .or(Some(provisioning.provision_op_id)),
            "provision_status": latest_op
                .as_ref()
                .map(|(_, status, _)| status.clone())
                .or(Some("requested".to_string())),
            "provision_updated_at": latest_op.as_ref().map(|(_, _, updated_at)| updated_at.clone()),
            "provision_claim": provisioning.claim_token,
            "approved_now": approved_now,
            "reused_existing": false,
            "message": "Approval accepted. Workflow provisioning is running asynchronously.",
        })),
    ))
}

async fn reject_recommendation(
    axum::extract::Path(id): axum::extract::Path<i64>,
    payload: Option<Json<RecommendationReviewActionRequest>>,
) -> StatusCode {
    let actor = recommendation_review_actor(
        payload.as_ref().and_then(|p| p.actor.as_deref()),
        "api.recommendation_review",
    );
    let note = payload.as_ref().and_then(|p| p.note.as_deref());
    let rec = match db::get_recommendation(id) {
        Ok(Some(rec)) => rec,
        Ok(None) => {
            log_recommendation_review_event(
                id,
                &format!("recommendation:{}", id),
                Some("missing"),
                None,
                "reject",
                Some(actor.as_str()),
                note,
                false,
                Some("recommendation not found"),
            );
            return StatusCode::NOT_FOUND;
        }
        Err(error) => {
            let details = error.to_string();
            log_recommendation_review_event(
                id,
                &format!("recommendation:{}", id),
                Some("error"),
                None,
                "reject",
                Some(actor.as_str()),
                note,
                false,
                Some(&details),
            );
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };
    match db::update_recommendation_review_status(id, "rejected") {
        Ok(_) => {
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some("rejected"),
                Some(rec.category.as_str()),
                "reject",
                Some(actor.as_str()),
                note,
                true,
                Some("recommendation rejected"),
            );
            StatusCode::OK
        }
        Err(error) => {
            let details = error.to_string();
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some(rec.status.as_str()),
                Some(rec.category.as_str()),
                "reject",
                Some(actor.as_str()),
                note,
                false,
                Some(&details),
            );
            if details.contains("not found") {
                StatusCode::NOT_FOUND
            } else if details.contains("invalid recommendation transition") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

async fn submit_recommendation_feedback(
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(req): Json<RecommendationFeedbackRequest>,
) -> Result<(StatusCode, Json<RecommendationFeedbackResponse>), StatusCode> {
    let feedback = req.feedback.trim();
    let actor = recommendation_review_actor(req.actor.as_deref(), "api.recommendation_feedback");
    if feedback.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(RecommendationFeedbackResponse {
                ok: false,
                sentiment: "refine".to_string(),
                status: "invalid".to_string(),
                message: "피드백 내용을 입력해 주세요.".to_string(),
                suppressed_similar: false,
            }),
        ));
    }

    let rec = db::get_recommendation(id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let sentiment = classify_recommendation_feedback(feedback).to_string();
    let recorded = db::record_recommendation_feedback(id, &sentiment, feedback)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !recorded {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    match sentiment.as_str() {
        "negative" => {
            db::update_recommendation_review_status(id, "rejected")
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
        "refine" => {
            if !rec.status.eq_ignore_ascii_case("approved")
                && !rec.status.eq_ignore_ascii_case("rejected")
            {
                db::snooze_recommendation(id, recommendation_feedback_snooze_hours())
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            }
        }
        _ => {}
    }

    let updated = db::get_recommendation(id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let effective_status = recommendation_effective_status(&updated);

    let message = match sentiment.as_str() {
        "negative" => "피드백을 기록했고 이 추천은 거절 처리했습니다. 유사 자동 추천도 계속 억제됩니다.",
        "refine" => "피드백을 기록했고 이 추천은 잠시 숨겼습니다. 승인하면 수정 지시를 워크플로우 생성에 반영합니다.",
        _ => "피드백을 기록했습니다.",
    };
    let review_action = match sentiment.as_str() {
        "negative" => "feedback_negative",
        "refine" => "feedback_refine",
        _ => "feedback_positive",
    };
    log_recommendation_review_event(
        updated.id,
        &updated.title,
        Some(effective_status.as_str()),
        Some(updated.category.as_str()),
        review_action,
        Some(actor.as_str()),
        Some(feedback),
        true,
        Some(message),
    );

    Ok((
        StatusCode::OK,
        Json(RecommendationFeedbackResponse {
            ok: true,
            sentiment,
            status: effective_status,
            message: message.to_string(),
            suppressed_similar: updated
                .feedback_status
                .as_deref()
                .map(|value| value != "positive")
                .unwrap_or(false),
        }),
    ))
}

async fn later_recommendation(
    axum::extract::Path(id): axum::extract::Path<i64>,
    payload: Option<Json<RecommendationReviewActionRequest>>,
) -> StatusCode {
    let actor = recommendation_review_actor(
        payload.as_ref().and_then(|p| p.actor.as_deref()),
        "api.recommendation_review",
    );
    let note = payload.as_ref().and_then(|p| p.note.as_deref());
    let rec = match db::get_recommendation(id) {
        Ok(Some(rec)) => rec,
        Ok(None) => {
            log_recommendation_review_event(
                id,
                &format!("recommendation:{}", id),
                Some("missing"),
                None,
                "later",
                Some(actor.as_str()),
                note,
                false,
                Some("recommendation not found"),
            );
            return StatusCode::NOT_FOUND;
        }
        Err(error) => {
            let details = error.to_string();
            log_recommendation_review_event(
                id,
                &format!("recommendation:{}", id),
                Some("error"),
                None,
                "later",
                Some(actor.as_str()),
                note,
                false,
                Some(&details),
            );
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };
    let hours = std::env::var("ALLVIA_RECOMMENDATION_SNOOZE_HOURS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .map(|v| v.clamp(1, 24 * 30))
        .unwrap_or(24 * 7);
    match db::snooze_recommendation(id, hours) {
        Ok(_) => {
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some("pending"),
                Some(rec.category.as_str()),
                "later",
                Some(actor.as_str()),
                note,
                true,
                Some("recommendation snoozed"),
            );
            StatusCode::OK
        }
        Err(error) => {
            let details = error.to_string();
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some(rec.status.as_str()),
                Some(rec.category.as_str()),
                "later",
                Some(actor.as_str()),
                note,
                false,
                Some(&details),
            );
            if details.contains("not found") {
                StatusCode::NOT_FOUND
            } else if details.contains("cannot snooze recommendation") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

async fn restore_recommendation(
    axum::extract::Path(id): axum::extract::Path<i64>,
    payload: Option<Json<RecommendationReviewActionRequest>>,
) -> StatusCode {
    let actor = recommendation_review_actor(
        payload.as_ref().and_then(|p| p.actor.as_deref()),
        "api.recommendation_review",
    );
    let note = payload.as_ref().and_then(|p| p.note.as_deref());
    let rec = match db::get_recommendation(id) {
        Ok(Some(rec)) => rec,
        Ok(None) => {
            log_recommendation_review_event(
                id,
                &format!("recommendation:{}", id),
                Some("missing"),
                None,
                "restore",
                Some(actor.as_str()),
                note,
                false,
                Some("recommendation not found"),
            );
            return StatusCode::NOT_FOUND;
        }
        Err(error) => {
            let details = error.to_string();
            log_recommendation_review_event(
                id,
                &format!("recommendation:{}", id),
                Some("error"),
                None,
                "restore",
                Some(actor.as_str()),
                note,
                false,
                Some(&details),
            );
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };
    match db::restore_recommendation(id) {
        Ok(_) => {
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some("pending"),
                Some(rec.category.as_str()),
                "restore",
                Some(actor.as_str()),
                note,
                true,
                Some("recommendation restored"),
            );
            StatusCode::OK
        }
        Err(error) => {
            let details = error.to_string();
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some(rec.status.as_str()),
                Some(rec.category.as_str()),
                "restore",
                Some(actor.as_str()),
                note,
                false,
                Some(&details),
            );
            if details.contains("not found") {
                StatusCode::NOT_FOUND
            } else if details.contains("cannot restore recommendation") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

async fn list_exec_approvals(
    Query(query): Query<ExecApprovalQuery>,
) -> Json<Vec<db::ExecApproval>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let status = match query.status.as_deref() {
        Some("all") => None,
        other => other,
    };
    let approvals = db::list_exec_approvals(status, limit).unwrap_or_default();
    Json(approvals)
}

async fn approve_exec_approval(
    Path(id): Path<String>,
    payload: Option<Json<ExecApprovalResolve>>,
) -> StatusCode {
    let resolved_by = payload.as_ref().and_then(|p| p.resolved_by.as_deref());
    let decision = payload
        .as_ref()
        .and_then(|p| p.decision.as_deref())
        .unwrap_or("allow-once");

    if decision == "allow-always" {
        if let Ok(Some(approval)) = db::get_exec_approval(&id) {
            let _ = db::add_exec_allowlist(&approval.command, approval.cwd.as_deref());
        }
    }

    match db::resolve_exec_approval(&id, "approved", resolved_by, Some(decision)) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn reject_exec_approval(
    Path(id): Path<String>,
    payload: Option<Json<ExecApprovalResolve>>,
) -> StatusCode {
    let resolved_by = payload.as_ref().and_then(|p| p.resolved_by.as_deref());
    match db::resolve_exec_approval(&id, "rejected", resolved_by, Some("deny")) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn list_routine_runs(Query(query): Query<RoutineRunsQuery>) -> Json<Vec<db::RoutineRun>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let runs = db::list_routine_runs(limit).unwrap_or_default();
    Json(runs)
}

async fn list_exec_allowlist(
    Query(query): Query<ExecAllowlistQuery>,
) -> Json<Vec<db::ExecAllowlistEntry>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let entries = db::list_exec_allowlist(limit).unwrap_or_default();
    Json(entries)
}

async fn list_exec_results(Query(query): Query<ExecResultsQuery>) -> Json<Vec<db::ExecResult>> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let status = query.status.as_deref();
    let results = db::list_exec_results(status, limit).unwrap_or_default();
    Json(results)
}

async fn list_verification_runs(
    Query(query): Query<VerificationRunsQuery>,
) -> Json<Vec<db::VerificationRun>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let runs = db::list_verification_runs(limit).unwrap_or_default();
    Json(runs)
}

async fn list_nl_runs_handler(Query(query): Query<NLRunQuery>) -> Json<Vec<db::NLRun>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let runs = db::list_nl_runs(limit).unwrap_or_default();
    Json(runs)
}

async fn nl_run_metrics_handler(Query(query): Query<NLRunMetricsQuery>) -> Json<db::NLRunMetrics> {
    let limit = query.limit.unwrap_or(50).clamp(1, 500);
    let metrics = db::get_nl_run_metrics(limit).unwrap_or(db::NLRunMetrics {
        total: 0,
        completed: 0,
        manual_required: 0,
        approval_required: 0,
        blocked: 0,
        error: 0,
        success_rate: 0.0,
    });
    Json(metrics)
}

async fn list_task_runs_handler(
    Query(query): Query<TaskRunsQuery>,
) -> Json<Vec<db::TaskRunRecord>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let status = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let runs = db::list_task_runs(limit, status).unwrap_or_default();
    Json(runs)
}

async fn list_collector_handoff_receipts_handler(
    Query(query): Query<CollectorHandoffReceiptsQuery>,
) -> Json<Vec<db::CollectorHandoffReceiptRecord>> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let rows = db::list_collector_handoff_receipts(limit).unwrap_or_default();
    Json(rows)
}

async fn runtime_db_paths_handler() -> Json<RuntimeDbPathsResponse> {
    let cfg_path = std::env::var("STEER_COLLECTOR_CONFIG")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "configs/config.yaml".to_string());
    let collector = collector_pipeline::resolve_db_path(Some(std::path::Path::new(&cfg_path)));
    let collector_norm = collector
        .canonicalize()
        .unwrap_or_else(|_| collector.clone())
        .to_string_lossy()
        .to_string();

    let core_opt = db::current_db_path();
    let core_norm_opt = core_opt.as_ref().map(|p| {
        let path = std::path::Path::new(p);
        path.canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string()
    });

    let mismatch = match core_norm_opt.as_deref() {
        Some(core) => core != collector_norm,
        None => false,
    };
    let allow_mismatch = std::env::var("STEER_ALLOW_COLLECTOR_DB_MISMATCH")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);

    Json(RuntimeDbPathsResponse {
        core_db_path: core_norm_opt,
        collector_db_path: collector_norm,
        mismatch,
        allow_mismatch,
    })
}

async fn runtime_info_handler() -> Json<RuntimeInfoResponse> {
    let api_port = std::env::var("STEER_API_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(5680);
    let started_at = API_SERVER_STARTED_AT
        .get()
        .cloned()
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    Json(RuntimeInfoResponse {
        service: "AllvIa Core API".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        profile: if cfg!(debug_assertions) {
            "debug".to_string()
        } else {
            "release".to_string()
        },
        pid: std::process::id(),
        api_port,
        allow_no_key: crate::env_flag("STEER_API_ALLOW_NO_KEY"),
        started_at,
        binary_path: std::env::current_exe()
            .ok()
            .map(|p| p.to_string_lossy().to_string()),
        current_dir: std::env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().to_string()),
    })
}

async fn lock_metrics_handler() -> Json<LockMetricsResponse> {
    let snapshot = crate::singleton_lock::lock_metrics_snapshot();
    Json(LockMetricsResponse {
        acquired: snapshot.acquired,
        bypassed: snapshot.bypassed,
        blocked: snapshot.blocked,
        stale_recovered: snapshot.stale_recovered,
        rejected: snapshot.rejected,
    })
}

async fn list_workflow_provision_ops_handler(
    Query(query): Query<WorkflowProvisionOpsQuery>,
) -> Json<Vec<db::WorkflowProvisionOpRecord>> {
    // Opportunistically reconcile stale requested/created ops on read so UI polling
    // can surface terminal status without relying on a separate scheduler tick.
    let _ = db::reconcile_workflow_provision_ops(50);
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let status = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let rows =
        db::list_workflow_provision_ops(limit, status, query.recommendation_id).unwrap_or_default();
    Json(rows)
}

async fn get_task_run_handler(Path(run_id): Path<String>) -> impl IntoResponse {
    match db::get_task_run(&run_id) {
        Ok(Some(run)) => (StatusCode::OK, Json(json!(run))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "task_run_not_found", "run_id": run_id })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "task_run_lookup_failed", "details": e.to_string() })),
        )
            .into_response(),
    }
}

async fn list_task_stage_runs_handler(Path(run_id): Path<String>) -> impl IntoResponse {
    match db::get_task_run(&run_id) {
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "task_run_not_found", "run_id": run_id })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "task_run_lookup_failed", "details": e.to_string() })),
        )
            .into_response(),
        Ok(Some(_)) => match db::list_task_stage_runs(&run_id) {
            Ok(stages) => (StatusCode::OK, Json(json!(stages))).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "task_stage_runs_failed", "details": e.to_string() })),
            )
                .into_response(),
        },
    }
}

async fn list_task_stage_assertions_handler(Path(run_id): Path<String>) -> impl IntoResponse {
    match db::get_task_run(&run_id) {
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "task_run_not_found", "run_id": run_id })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "task_run_lookup_failed", "details": e.to_string() })),
        )
            .into_response(),
        Ok(Some(_)) => match db::list_task_stage_assertions(&run_id) {
            Ok(assertions) => (StatusCode::OK, Json(json!(assertions))).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "task_stage_assertions_failed", "details": e.to_string() })),
            )
                .into_response(),
        },
    }
}

async fn list_task_run_artifacts_handler(Path(run_id): Path<String>) -> impl IntoResponse {
    match db::get_task_run(&run_id) {
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "task_run_not_found", "run_id": run_id })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "task_run_lookup_failed", "details": e.to_string() })),
        )
            .into_response(),
        Ok(Some(_)) => match db::list_task_run_artifacts(&run_id) {
            Ok(artifacts) => Json(json!({ "artifacts": artifacts })).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "task_run_artifacts_failed", "details": e.to_string() })),
            )
                .into_response(),
        },
    }
}

async fn list_nl_approval_policies(
    Query(query): Query<ApprovalPolicyQuery>,
) -> Json<Vec<ApprovalPolicyResponse>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let policies = db::list_approval_policies(limit).unwrap_or_default();
    let mapped = policies
        .into_iter()
        .map(|policy| ApprovalPolicyResponse {
            policy_key: policy.policy_key,
            decision: policy.decision,
            updated_at: policy.updated_at,
        })
        .collect();
    Json(mapped)
}

async fn set_nl_approval_policy(Json(payload): Json<ApprovalPolicyRequest>) -> StatusCode {
    if payload.policy_key.trim().is_empty() || payload.decision.trim().is_empty() {
        return StatusCode::BAD_REQUEST;
    }
    match db::upsert_approval_policy(&payload.policy_key, &payload.decision) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn remove_nl_approval_policy(Path(key): Path<String>) -> StatusCode {
    if key.trim().is_empty() {
        return StatusCode::BAD_REQUEST;
    }
    match db::delete_approval_policy(&key) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn log_verification_run(kind: &str, ok: bool, summary: &str, details: Option<serde_json::Value>) {
    let details_str = details.map(|v| v.to_string());
    let _ = db::insert_verification_run(kind, ok, summary, details_str.as_deref());
}

async fn add_exec_allowlist(Json(payload): Json<ExecAllowlistRequest>) -> StatusCode {
    if payload.pattern.trim().is_empty() {
        return StatusCode::BAD_REQUEST;
    }
    match db::add_exec_allowlist(&payload.pattern, payload.cwd.as_deref()) {
        Ok(_) => StatusCode::CREATED,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("exec allowlist pattern rejected") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

async fn remove_exec_allowlist(Path(id): Path<i64>) -> StatusCode {
    match db::remove_exec_allowlist(id) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[derive(Serialize)]
pub struct RecommendationMetricsResponse {
    total: i64,
    approved: i64,
    rejected: i64,
    failed: i64,
    pending: i64,
    later: i64,
    legacy_other: i64,
    approval_rate: f64,
    last_created_at: Option<String>,
}

async fn get_recommendation_metrics() -> Json<RecommendationMetricsResponse> {
    let metrics = db::get_recommendation_metrics().unwrap_or(crate::db::RecommendationMetrics {
        total: 0,
        approved: 0,
        rejected: 0,
        failed: 0,
        pending: 0,
        later: 0,
        legacy_other: 0,
        last_created_at: None,
    });
    let visible_queue_counts = db::get_recommendations_with_filter(Some("all"))
        .ok()
        .map(|recs| {
            let work_filter =
                crate::recommendation_policy::normalize_list_category_filter(None);
            let mut pending = 0_i64;
            let mut later = 0_i64;
            let mut failed = 0_i64;
            for rec in recs {
                let (category, _) = recommendation_effective_category_and_score(&rec);
                if !crate::recommendation_policy::matches_category_filter(
                    &category,
                    work_filter.as_deref(),
                ) {
                    continue;
                }
                match recommendation_effective_status(&rec).as_str() {
                    "pending" => {
                        if rec.pattern_id.is_some()
                            && !crate::recommendation_policy::evaluate_recommendation_approval_readiness(&rec).ready
                        {
                            continue;
                        }
                        pending += 1;
                    }
                    "later" => later += 1,
                    "failed" => failed += 1,
                    _ => {}
                }
            }
            (pending, later, failed)
        })
        .unwrap_or((metrics.pending, metrics.later, metrics.failed));

    let approval_rate = if metrics.total > 0 {
        (metrics.approved as f64 / metrics.total as f64) * 100.0
    } else {
        0.0
    };

    Json(RecommendationMetricsResponse {
        total: metrics.total,
        approved: metrics.approved,
        rejected: metrics.rejected,
        failed: visible_queue_counts.2,
        pending: visible_queue_counts.0,
        later: visible_queue_counts.1,
        legacy_other: metrics.legacy_other,
        approval_rate,
        last_created_at: metrics.last_created_at,
    })
}

async fn list_recommendation_review_events_handler(
    Query(query): Query<RecommendationReviewEventsQuery>,
) -> Json<Vec<db::RecommendationReviewEventRecord>> {
    let limit = memory_admin_limit(query.limit);
    Json(db::list_recommendation_review_events(limit).unwrap_or_default())
}

fn launch_ops_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(200).clamp(20, 500)
}

fn launch_eval_candidate_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(12).clamp(1, 30)
}

async fn get_launch_ops_handler(Query(query): Query<LaunchOpsQuery>) -> Json<LaunchOpsResponse> {
    let limit = launch_ops_limit(query.limit);
    let chat_metrics = db::get_launch_ops_metrics(limit).unwrap_or(db::LaunchOpsMetrics {
        window_size: limit,
        total_requests: 0,
        blocked_requests: 0,
        intent_memory_hits: 0,
        request_memory_hits: 0,
        execution_memory_hits: 0,
        cached_response_hit_rate: 0.0,
        deterministic_routes: 0,
        llm_routes: 0,
        ai_digest_routes: 0,
        ai_digest_auto_routes: 0,
        local_routes: 0,
        freshness_bypasses: 0,
        low_confidence_routes: 0,
        unknown_routes: 0,
        error_routes: 0,
        last_event_at: None,
        route_breakdown: Vec::new(),
    });
    let memory_metrics = db::get_memory_ops_metrics().unwrap_or(db::MemoryOpsMetrics {
        request_active: 0,
        request_suppressed: 0,
        execution_active: 0,
        execution_suppressed: 0,
        last_request_used_at: None,
        last_execution_used_at: None,
    });
    let nl_run_metrics = db::get_nl_run_metrics(limit).unwrap_or(db::NLRunMetrics {
        total: 0,
        completed: 0,
        manual_required: 0,
        approval_required: 0,
        blocked: 0,
        error: 0,
        success_rate: 0.0,
    });
    let exec_approval_metrics =
        db::get_exec_approval_metrics(limit).unwrap_or(db::ExecApprovalMetrics {
            window_size: limit,
            total: 0,
            pending: 0,
            approved: 0,
            rejected: 0,
            expired_pending: 0,
            allow_once: 0,
            allow_always: 0,
            deny: 0,
            approval_rate: 0.0,
            oldest_pending_created_at: None,
            last_created_at: None,
            last_resolved_at: None,
        });
    let recommendation_metrics = get_recommendation_metrics().await.0;
    let recommendation_review_metrics =
        db::get_recommendation_review_metrics(limit).unwrap_or(db::RecommendationReviewMetrics {
            window_size: limit,
            total_events: 0,
            approve_actions: 0,
            reject_actions: 0,
            later_actions: 0,
            restore_actions: 0,
            feedback_positive: 0,
            feedback_refine: 0,
            feedback_negative: 0,
            failed_actions: 0,
            action_failure_rate: 0.0,
            non_positive_feedback_rate: 0.0,
            last_event_at: None,
        });
    let recent_events = db::list_launch_ops_events(limit.min(12)).unwrap_or_default();

    Json(LaunchOpsResponse {
        chat_metrics,
        memory_metrics,
        nl_run_metrics,
        exec_approval_metrics,
        recommendation_metrics,
        recommendation_review_metrics,
        recent_events,
    })
}

async fn get_launch_eval_candidates_handler(
    Query(query): Query<LaunchEvalCandidatesQuery>,
) -> Json<Vec<launch_eval::LaunchEvalCandidate>> {
    let workdir = resolve_operational_workdir(query.workdir.as_deref());
    let provenance_filter =
        launch_eval::parse_launch_eval_candidate_provenance_filter(query.provenance.as_deref());
    Json(launch_eval::generate_launch_eval_candidates_with_filter(
        &workdir,
        launch_eval_candidate_limit(query.limit),
        provenance_filter,
    ))
}

async fn get_launch_eval_candidate_snapshot_info_handler(
    Query(query): Query<LaunchEvalSnapshotInfoQuery>,
) -> Result<Json<launch_eval::LaunchEvalCandidateSnapshotInfo>, StatusCode> {
    if requested_operational_override(query.output_path.as_deref())
        && !allow_operational_path_override()
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let workdir = resolve_operational_workdir(query.workdir.as_deref());
    let provenance_filter =
        launch_eval::parse_launch_eval_candidate_provenance_filter(query.provenance.as_deref());
    let output_path = resolve_operational_file_override(query.output_path.as_deref())
        .map(|value| value.display().to_string());
    Ok(Json(
        launch_eval::read_launch_eval_candidate_snapshot_info_with_filter(
            &workdir,
            output_path.as_deref(),
            provenance_filter,
        ),
    ))
}

async fn write_launch_eval_candidate_snapshot_handler(
    Json(payload): Json<LaunchEvalSnapshotRequest>,
) -> Result<Json<launch_eval::LaunchEvalCandidateSnapshot>, StatusCode> {
    if requested_operational_override(payload.output_path.as_deref())
        && !allow_operational_path_override()
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let limit = launch_eval_candidate_limit(payload.limit);
    let workdir = resolve_operational_workdir(payload.workdir.as_deref());
    let provenance_filter =
        launch_eval::parse_launch_eval_candidate_provenance_filter(payload.provenance.as_deref());
    let output_path = resolve_operational_file_override(payload.output_path.as_deref())
        .map(|value| value.display().to_string());
    let snapshot = launch_eval::write_launch_eval_candidate_snapshot_with_filter(
        &workdir,
        output_path.as_deref(),
        limit,
        provenance_filter,
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(snapshot))
}

fn memory_admin_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(12).clamp(1, 50)
}

fn memory_admin_action_message(ok: bool, kind: &str, action: &str) -> String {
    if ok {
        format!("{} {} succeeded.", kind, action)
    } else {
        format!("{} {} target was not found.", kind, action)
    }
}

fn memory_admin_action_response(
    ok: bool,
    kind: &str,
    action: &str,
) -> (StatusCode, Json<MemoryAdminActionResponse>) {
    let message = memory_admin_action_message(ok, kind, action);
    (
        if ok {
            StatusCode::OK
        } else {
            StatusCode::NOT_FOUND
        },
        Json(MemoryAdminActionResponse {
            ok,
            kind: kind.to_string(),
            action: action.to_string(),
            message,
        }),
    )
}

async fn list_memory_records_handler(
    Query(query): Query<MemoryRecordsQuery>,
) -> Json<MemoryRecordsResponse> {
    let limit = memory_admin_limit(query.limit);
    let include_suppressed = query.include_suppressed.unwrap_or(true);
    let metrics = db::get_memory_ops_metrics().unwrap_or(crate::db::MemoryOpsMetrics {
        request_active: 0,
        request_suppressed: 0,
        execution_active: 0,
        execution_suppressed: 0,
        last_request_used_at: None,
        last_execution_used_at: None,
    });
    let request_memory =
        db::list_request_memory_records(limit, include_suppressed).unwrap_or_default();
    let execution_memory =
        db::list_execution_memory_records(limit, include_suppressed).unwrap_or_default();
    let recent_admin_events = db::list_memory_admin_events(limit).unwrap_or_default();
    Json(MemoryRecordsResponse {
        metrics,
        request_memory,
        execution_memory,
        recent_admin_events,
    })
}

async fn suppress_request_memory_handler(
    Json(req): Json<RequestMemoryAdminActionRequest>,
) -> (StatusCode, Json<MemoryAdminActionResponse>) {
    let ok = if req.request_text.trim().is_empty() {
        false
    } else {
        db::suppress_request_memory_scoped(
            req.memory_scope.as_deref(),
            &req.request_text,
            req.reason.as_deref(),
        )
        .unwrap_or(false)
    };
    let message = memory_admin_action_message(ok, "request_memory", "suppress");
    let actor = req.actor.as_deref().unwrap_or("api.memory_admin");
    let _ = db::record_memory_admin_event(
        "request_memory",
        "suppress",
        req.memory_scope.as_deref(),
        &req.request_text,
        req.reason.as_deref(),
        Some(actor),
        ok,
        Some(&message),
    );
    memory_admin_action_response(ok, "request_memory", "suppress")
}

async fn restore_request_memory_handler(
    Json(req): Json<RequestMemoryAdminActionRequest>,
) -> (StatusCode, Json<MemoryAdminActionResponse>) {
    let ok = if req.request_text.trim().is_empty() {
        false
    } else {
        db::restore_request_memory_scoped(req.memory_scope.as_deref(), &req.request_text)
            .unwrap_or(false)
    };
    let message = memory_admin_action_message(ok, "request_memory", "restore");
    let actor = req.actor.as_deref().unwrap_or("api.memory_admin");
    let _ = db::record_memory_admin_event(
        "request_memory",
        "restore",
        req.memory_scope.as_deref(),
        &req.request_text,
        req.reason.as_deref(),
        Some(actor),
        ok,
        Some(&message),
    );
    memory_admin_action_response(ok, "request_memory", "restore")
}

async fn delete_request_memory_handler(
    Json(req): Json<RequestMemoryAdminActionRequest>,
) -> (StatusCode, Json<MemoryAdminActionResponse>) {
    let ok = if req.request_text.trim().is_empty() {
        false
    } else {
        db::delete_request_memory_scoped(req.memory_scope.as_deref(), &req.request_text)
            .unwrap_or(false)
    };
    let message = memory_admin_action_message(ok, "request_memory", "delete");
    let actor = req.actor.as_deref().unwrap_or("api.memory_admin");
    let _ = db::record_memory_admin_event(
        "request_memory",
        "delete",
        req.memory_scope.as_deref(),
        &req.request_text,
        req.reason.as_deref(),
        Some(actor),
        ok,
        Some(&message),
    );
    memory_admin_action_response(ok, "request_memory", "delete")
}

async fn suppress_execution_memory_handler(
    Json(req): Json<ExecutionMemoryAdminActionRequest>,
) -> (StatusCode, Json<MemoryAdminActionResponse>) {
    let ok = if req.intent_command.trim().is_empty() || req.params_key.trim().is_empty() {
        false
    } else {
        db::suppress_execution_memory_scoped(
            req.memory_scope.as_deref(),
            &req.intent_command,
            &req.params_key,
            req.reason.as_deref(),
        )
        .unwrap_or(false)
    };
    let message = memory_admin_action_message(ok, "execution_memory", "suppress");
    let actor = req.actor.as_deref().unwrap_or("api.memory_admin");
    let target_key = format!("{}::{}", req.intent_command.trim(), req.params_key.trim());
    let _ = db::record_memory_admin_event(
        "execution_memory",
        "suppress",
        req.memory_scope.as_deref(),
        &target_key,
        req.reason.as_deref(),
        Some(actor),
        ok,
        Some(&message),
    );
    memory_admin_action_response(ok, "execution_memory", "suppress")
}

async fn restore_execution_memory_handler(
    Json(req): Json<ExecutionMemoryAdminActionRequest>,
) -> (StatusCode, Json<MemoryAdminActionResponse>) {
    let ok = if req.intent_command.trim().is_empty() || req.params_key.trim().is_empty() {
        false
    } else {
        db::restore_execution_memory_scoped(
            req.memory_scope.as_deref(),
            &req.intent_command,
            &req.params_key,
        )
        .unwrap_or(false)
    };
    let message = memory_admin_action_message(ok, "execution_memory", "restore");
    let actor = req.actor.as_deref().unwrap_or("api.memory_admin");
    let target_key = format!("{}::{}", req.intent_command.trim(), req.params_key.trim());
    let _ = db::record_memory_admin_event(
        "execution_memory",
        "restore",
        req.memory_scope.as_deref(),
        &target_key,
        req.reason.as_deref(),
        Some(actor),
        ok,
        Some(&message),
    );
    memory_admin_action_response(ok, "execution_memory", "restore")
}

async fn delete_execution_memory_handler(
    Json(req): Json<ExecutionMemoryAdminActionRequest>,
) -> (StatusCode, Json<MemoryAdminActionResponse>) {
    let ok = if req.intent_command.trim().is_empty() || req.params_key.trim().is_empty() {
        false
    } else {
        db::delete_execution_memory_scoped(
            req.memory_scope.as_deref(),
            &req.intent_command,
            &req.params_key,
        )
        .unwrap_or(false)
    };
    let message = memory_admin_action_message(ok, "execution_memory", "delete");
    let actor = req.actor.as_deref().unwrap_or("api.memory_admin");
    let target_key = format!("{}::{}", req.intent_command.trim(), req.params_key.trim());
    let _ = db::record_memory_admin_event(
        "execution_memory",
        "delete",
        req.memory_scope.as_deref(),
        &target_key,
        req.reason.as_deref(),
        Some(actor),
        ok,
        Some(&message),
    );
    memory_admin_action_response(ok, "execution_memory", "delete")
}

// Add at top: use crate::recommendation::AutomationProposal;

async fn analyze_patterns() -> Json<Vec<String>> {
    Json(run_analysis_internal())
}

fn run_analysis_internal() -> Vec<String> {
    let detector = pattern_detector::PatternDetector::new();
    let patterns = detector.analyze();
    let preference_history = db::get_recent_recommendations(
        crate::recommendation_policy::auto_recommendation_history_limit(),
    )
    .unwrap_or_default();

    // 1. Save detected patterns to DB
    for p in &patterns {
        if !detector.should_recommend(p) {
            continue;
        }
        let proposal = crate::recommendation::AutomationProposal {
            title: format!("New Pattern: {}", p.description),
            summary: format!(
                "Detected {} repeats across {} distinct day(s). AI suggests automating this.",
                p.occurrences, p.distinct_days
            ),
            trigger: format!("Pattern Type: {:?}", p.pattern_type),
            actions: vec!["Analyze".to_string(), "Automate".to_string()],
            n8n_prompt: format!("Create an automation for: {}", p.description),
            confidence: p.similarity_score,
            evidence: vec![
                format!("Pattern: {}", p.description),
                format!("Frequency: {} occurrences", p.occurrences),
                format!("Span: {} distinct day(s)", p.distinct_days),
                format!(
                    "Work context: weekday {} / work-hour {} occurrences",
                    p.weekday_occurrences, p.work_hour_occurrences
                ),
            ],
            pattern_id: Some(p.pattern_id.clone()),
            category: crate::recommendation_policy::CATEGORY_UNKNOWN.to_string(),
            business_score: 0.0,
        };
        let mut proposal = proposal;
        let decision = crate::recommendation_policy::apply_mvp_policy(&mut proposal, Some(p));
        if !decision.accepted {
            continue;
        }
        crate::recommendation_policy::apply_recommendation_preferences(
            &mut proposal,
            &preference_history,
        );
        if !auto_recommendation_allowed(&proposal, "api.patterns.analyze") {
            continue;
        }
        if let Err(e) = db::insert_recommendation(&proposal) {
            eprintln!("Failed to save pattern: {}", e);
        }
    }

    // 2. Fallback: If empty, create a random demo recommendation (For User Experience)
    // DISABLED: Random spam fix
    /*
    if patterns.is_empty() {
        let timestamp = chrono::Utc::now().timestamp() % 1000;
        let proposal = crate::recommendation::AutomationProposal {
            title: format!("Smart Recommendation #{}", timestamp),
            summary: "AI has identified a potential workflow improvement based on recent activity.".to_string(),
            trigger: "System Activity Analysis".to_string(),
            actions: vec!["Log Activity".to_string(), "Send Notification".to_string()],
            n8n_prompt: "Create a workflow that logs system activity and sends a summary notification.".to_string(),
            confidence: 0.85,
            category: crate::recommendation_policy::CATEGORY_UNKNOWN.to_string(),
            business_score: 0.0,
        };
        if let Err(e) = db::insert_recommendation(&proposal) {
            eprintln!("⚠️ Failed to save recommendation analysis: {}", e);
        }
        return vec![];
    }
    */

    patterns
        .into_iter()
        .filter(|pattern| detector.should_recommend(pattern))
        .map(|p| format!("{} ({} occurrences)", p.description, p.occurrences))
        .collect()
}

async fn get_quality_metrics() -> Json<QualityMetrics> {
    let collector = feedback_collector::FeedbackCollector::new();
    let metrics = collector.get_quality_metrics();

    Json(QualityMetrics {
        total: metrics.total_executions,
        success: metrics.successful_executions,
        rate: metrics.success_rate,
    })
}

#[derive(serde::Deserialize)]
struct GoalRequest {
    goal: String,
}

#[derive(Deserialize)]
struct RunGoalRequest {
    goal: String,
    session_key: Option<String>,
}

#[derive(Serialize)]
struct RunGoalResponse {
    run_id: String,
    planner_complete: bool,
    execution_complete: bool,
    business_complete: bool,
    status: String,
    summary: Option<String>,
}

async fn execute_goal_handler(
    State(state): State<AppState>,
    Json(payload): Json<GoalRequest>,
) -> Json<serde_json::Value> {
    if let Ok(mut guard) = state.current_goal.lock() {
        *guard = Some(payload.goal.clone());
    }
    if let Some(llm) = state.llm_client {
        // Spawn background task for OODA loop
        tokio::spawn(async move {
            let planner = crate::controller::planner::Planner::new(llm, None);
            match planner.run_goal_tracked(&payload.goal, None).await {
                Ok(outcome) => println!(
                    "✅ Goal Execution Success (run_id={}, planner={}, execution={}, business={})",
                    outcome.run_id,
                    outcome.planner_complete,
                    outcome.execution_complete,
                    outcome.business_complete
                ),
                Err(e) => println!("❌ Goal Execution Failed: {}", e),
            }
        });

        Json(serde_json::json!({
            "status": "started",
            "message": "Autonmous Agent started. Monitor logs for progress."
        }))
    } else {
        Json(serde_json::json!({
            "status": "error",
            "message": "LLM Client not available"
        }))
    }
}

async fn run_goal_sync_handler(
    State(state): State<AppState>,
    Json(payload): Json<RunGoalRequest>,
) -> impl IntoResponse {
    let goal = payload.goal.trim();
    if goal.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "goal_empty" })),
        )
            .into_response();
    }

    let Some(llm) = state.llm_client.clone() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "llm_client_not_available" })),
        )
            .into_response();
    };

    let _ = db::mark_stale_running_task_runs_finished();
    let allow_goal_queue = env_truthy_default("STEER_ALLOW_GOAL_QUEUE", false);
    if !allow_goal_queue {
        if let Ok(Some(active)) = db::get_latest_inflight_task_run() {
            return (
                StatusCode::ACCEPTED,
                Json(RunGoalResponse {
                    run_id: active.run_id,
                    planner_complete: false,
                    execution_complete: false,
                    business_complete: false,
                    status: "busy".to_string(),
                    summary: Some(format!(
                        "existing run in progress (status={}). join active run instead of queueing a new one.",
                        active.status
                    )),
                }),
            )
                .into_response();
        }
    }

    let run_id = format!(
        "surf_{}_{}",
        chrono::Utc::now().format("%Y%m%d_%H%M%S"),
        uuid::Uuid::new_v4().simple()
    );
    // Pre-register run row so clients can immediately observe status by run_id
    // even when execution waits on serialized GUI lock.
    let _ = db::create_task_run(&run_id, "surf_goal", goal, "queued");
    let spawned_run_id = run_id.clone();
    let goal_owned = goal.to_string();
    let session_owned = payload.session_key.clone();
    let planner = crate::controller::planner::Planner::new(llm, None);
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let result = planner
            .run_goal_tracked_with_run_id(&spawned_run_id, &goal_owned, session_owned.as_deref())
            .await;
        let _ = tx.send(result);
    });

    let wait_for_result = std::env::var("STEER_GOAL_SYNC_WAIT_FOR_RESULT")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);
    if !wait_for_result {
        return (
            StatusCode::ACCEPTED,
            Json(RunGoalResponse {
                run_id,
                planner_complete: false,
                execution_complete: false,
                business_complete: false,
                status: "accepted".to_string(),
                summary: Some("goal accepted and running asynchronously".to_string()),
            }),
        )
            .into_response();
    }

    let sync_timeout_sec = std::env::var("STEER_GOAL_SYNC_TIMEOUT_SEC")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30);

    match tokio::time::timeout(std::time::Duration::from_secs(sync_timeout_sec), rx).await {
        Ok(Ok(Ok(outcome))) => (
            StatusCode::OK,
            Json(RunGoalResponse {
                run_id: outcome.run_id,
                planner_complete: outcome.planner_complete,
                execution_complete: outcome.execution_complete,
                business_complete: outcome.business_complete,
                status: outcome.status,
                summary: outcome.summary,
            }),
        )
            .into_response(),
        Ok(Ok(Err(e))) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "goal_run_failed", "detail": e.to_string() })),
        )
            .into_response(),
        Ok(Err(_recv_err)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "goal_run_failed", "detail": "planner_channel_closed" })),
        )
            .into_response(),
        Err(_) => (
            StatusCode::ACCEPTED,
            Json(RunGoalResponse {
                run_id,
                planner_complete: false,
                execution_complete: false,
                business_complete: false,
                status: "accepted".to_string(),
                summary: Some(format!(
                    "goal still running asynchronously (sync timeout {}s)",
                    sync_timeout_sec
                )),
            }),
        )
            .into_response(),
    }
}

async fn get_current_goal(State(state): State<AppState>) -> Json<serde_json::Value> {
    let goal = state
        .current_goal
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_default();
    Json(serde_json::json!({ "goal": goal }))
}

async fn agent_intent_handler(Json(payload): Json<AgentIntentRequest>) -> impl IntoResponse {
    let intent_result = intent_router::classify_intent(&payload.text);
    let fill = slot_filler::fill_slots(&intent_result.intent, intent_result.slots.clone());
    let session = nl_store::create_session(
        intent_result.clone(),
        fill.slots.clone(),
        payload.text.clone(),
    );

    let response = AgentIntentResponse {
        session_id: session.session_id,
        intent: intent_result.intent.as_str().to_string(),
        confidence: intent_result.confidence,
        slots: fill.slots,
        missing_slots: fill.missing,
        follow_up: fill.follow_up,
    };

    (StatusCode::OK, Json(response)).into_response()
}

async fn agent_plan_handler(Json(payload): Json<AgentPlanRequest>) -> impl IntoResponse {
    let Some(mut session) = nl_store::get_session(&payload.session_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "session_not_found" })),
        )
            .into_response();
    };

    if let Some(updates) = payload.slots.as_ref() {
        if let Some(updated) = nl_store::update_session_slots(&payload.session_id, updates) {
            session = updated;
        }
    }

    let fill = slot_filler::fill_slots(&session.intent.intent, session.slots.clone());
    let plan = plan_builder::build_plan(&session.intent.intent, &fill.slots);
    let _ = nl_store::set_session_plan(&payload.session_id, plan.clone());

    let response = AgentPlanResponse {
        plan_id: plan.plan_id,
        intent: session.intent.intent.as_str().to_string(),
        steps: plan.steps,
        missing_slots: fill.missing,
    };

    (StatusCode::OK, Json(response)).into_response()
}

async fn agent_execute_handler(Json(payload): Json<AgentExecuteRequest>) -> impl IntoResponse {
    let Some(plan) = nl_store::get_plan(&payload.plan_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "plan_not_found" })),
        )
            .into_response();
    };
    let _exec_guard = match acquire_agent_execution(&payload.plan_id) {
        Ok(g) => g,
        Err(conflict) => {
            let error_code = if conflict.scope == "global" {
                "agent_execution_in_progress_global"
            } else {
                "plan_execution_in_progress"
            };
            return (
                StatusCode::CONFLICT,
                Json(json!({
                    "error": error_code,
                    "plan_id": payload.plan_id,
                    "lock_scope": conflict.scope,
                    "active_plan_id": conflict.active_plan_id,
                    "message": "다른 실행이 진행 중입니다. 현재 실행이 끝난 뒤 다시 시도하세요."
                })),
            )
                .into_response();
        }
    };
    let session = nl_store::find_session_by_plan(&payload.plan_id);
    let run_intent = session
        .as_ref()
        .map(|s| s.intent.intent.as_str().to_string())
        .unwrap_or_else(|| plan.intent.as_str().to_string());
    let run_prompt = session
        .as_ref()
        .map(|s| s.prompt.clone())
        .unwrap_or_else(|| format!("plan_id={}", payload.plan_id));
    let run_id = format!(
        "{}_{}",
        payload.plan_id,
        chrono::Utc::now().timestamp_millis()
    );
    match db::claim_task_run(&plan.plan_id, &run_id, &run_intent, &run_prompt, "running") {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "plan_execution_in_progress_db",
                    "plan_id": payload.plan_id
                })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "task_run_claim_failed",
                    "detail": e.to_string()
                })),
            )
                .into_response();
        }
    }

    let mut resume_from = payload
        .resume_from
        .unwrap_or_else(|| nl_store::get_plan_progress(&plan.plan_id).unwrap_or(0));
    let mut resume_hint: Option<String> = payload
        .resume_from
        .map(|idx| format!("resume_from_override={}", idx));

    if let Some(raw_token) = payload
        .resume_token
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
    {
        match parse_resume_token(raw_token) {
            Ok(parsed) => {
                if parsed.plan_id != plan.plan_id {
                    crate::diagnostic_events::emit(
                        "agent.resume_token.invalid",
                        json!({
                            "reason": "plan_id_mismatch",
                            "request_plan_id": plan.plan_id,
                            "token_plan_id": parsed.plan_id
                        }),
                    );
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({
                            "error": "resume_token_plan_mismatch",
                            "plan_id": plan.plan_id
                        })),
                    )
                        .into_response();
                }
                resume_from = parsed.step_index;
                resume_hint = Some(format!(
                    "resume_token(reason={}, step={})",
                    parsed.reason, parsed.step_index
                ));
            }
            Err(err) => {
                crate::diagnostic_events::emit(
                    "agent.resume_token.invalid",
                    json!({
                        "reason": "parse_error",
                        "detail": err
                    }),
                );
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "resume_token_invalid",
                        "detail": err
                    })),
                )
                    .into_response();
            }
        }
    }

    if resume_from >= plan.steps.len() {
        if payload.resume_from.is_some() || payload.resume_token.is_some() {
            crate::diagnostic_events::emit(
                "agent.resume_token.invalid",
                json!({
                    "reason": "step_out_of_range",
                    "step_index": resume_from,
                    "plan_steps": plan.steps.len()
                }),
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "resume_step_out_of_range",
                    "step_index": resume_from,
                    "plan_steps": plan.steps.len()
                })),
            )
                .into_response();
        }
        nl_store::clear_plan_progress(&plan.plan_id);
        resume_from = 0;
    }
    let execution_profile = payload.profile.unwrap_or_default();
    let execution_options = execution_profile.execution_options();
    let mut result =
        execution_controller::execute_plan(&plan, resume_from, execution_options).await;
    stamp_run_scope_evidence(&mut result.logs, &run_id, &plan.plan_id);
    result.logs.push(format!(
        "Execution profile selected: {} (collision_policy={})",
        execution_profile.as_str(),
        execution_options.input_collision_policy.as_str()
    ));
    if let Some(hint) = resume_hint {
        result.logs.push(format!("Resume hint: {}", hint));
    }
    if resume_from > 0 {
        result
            .logs
            .insert(0, format!("Resuming from step {}", resume_from + 1));
    }
    let mut verify = verification_engine::verify_execution(&plan, &result.logs);
    if !verify.ok {
        result
            .logs
            .push(format!("Verification failed: {}", verify.issues.join("; ")));
    } else {
        result.logs.push("Verification passed".to_string());
    }

    // Simple auto-replan: one retry on failure or verification issues
    let auto_replan_env = std::env::var("STEER_AUTO_REPLAN")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(true);
    let auto_replan = if execution_profile.default_auto_replan_enabled() {
        auto_replan_env
    } else {
        false
    };
    let allow_replan = !matches!(
        result.status.as_str(),
        "manual_required" | "approval_required" | "blocked"
    );
    if auto_replan && allow_replan && (result.status == "error" || !verify.ok) {
        result
            .logs
            .push("Auto-replan: retrying once after short wait".to_string());
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let retry = execution_controller::execute_plan(&plan, 0, execution_options).await;
        result.logs = retry.logs;
        stamp_run_scope_evidence(&mut result.logs, &run_id, &plan.plan_id);
        result.status = retry.status;
        result.approval = retry.approval;
        result.manual_steps = retry.manual_steps;
        result.resume_from = retry.resume_from;
        result.resume_token = retry.resume_token;
        verify = verification_engine::verify_execution(&plan, &result.logs);
        if !verify.ok {
            result
                .logs
                .push(format!("Verification failed: {}", verify.issues.join("; ")));
        } else {
            result.logs.push("Verification passed".to_string());
        }
    }

    if matches!(
        result.status.as_str(),
        "manual_required" | "approval_required"
    ) {
        if let Some(next_step) = result.resume_from {
            nl_store::set_plan_progress(&plan.plan_id, next_step);
        }
    } else {
        nl_store::clear_plan_progress(&plan.plan_id);
    }

    let planner_complete = !plan.steps.is_empty();
    let execution_complete = result.status == "completed";
    let verification_ok = verify.ok;
    let (evidence_ok, evidence_detail) = evaluate_business_evidence(&plan, &result.logs);
    let business_complete =
        planner_complete && execution_complete && verification_ok && evidence_ok;

    let planner_actual = planner_complete.to_string();
    let execution_actual = execution_complete.to_string();
    let verify_actual = verification_ok.to_string();
    let evidence_actual = evidence_ok.to_string();
    let business_actual = business_complete.to_string();
    let mut stage_dod: Vec<AgentStageDodCheck> = Vec::new();

    let _ = db::record_task_stage_run(
        &run_id,
        "planner",
        1,
        "running",
        Some("planner outcome evaluation"),
    );
    let _ = db::record_task_stage_run(
        &run_id,
        "planner",
        1,
        if planner_complete {
            "completed"
        } else {
            "failed"
        },
        Some(&format!("plan_steps={}", plan.steps.len())),
    );
    let _ = db::record_task_stage_assertion(
        &run_id,
        "planner",
        "planner.plan_steps_non_empty",
        "true",
        &planner_actual,
        planner_complete,
        Some(&format!("plan_id={}", plan.plan_id)),
    );
    stage_dod.push(AgentStageDodCheck {
        stage: "planner".to_string(),
        key: "planner.plan_steps_non_empty".to_string(),
        expected: "true".to_string(),
        actual: planner_actual.clone(),
        passed: planner_complete,
        evidence: Some(format!("plan_id={}", plan.plan_id)),
    });
    let _ = db::record_task_stage_run(
        &run_id,
        "execution",
        2,
        "running",
        Some("execution result evaluation"),
    );
    let _ = db::record_task_stage_run(
        &run_id,
        "execution",
        2,
        if execution_complete {
            "completed"
        } else {
            result.status.as_str()
        },
        Some(&format!(
            "manual_steps={} logs={}",
            result.manual_steps.len(),
            result.logs.len()
        )),
    );
    let _ = db::record_task_stage_assertion(
        &run_id,
        "execution",
        "execution.status_completed",
        "true",
        &execution_actual,
        execution_complete,
        Some(result.status.as_str()),
    );
    stage_dod.push(AgentStageDodCheck {
        stage: "execution".to_string(),
        key: "execution.status_completed".to_string(),
        expected: "true".to_string(),
        actual: execution_actual.clone(),
        passed: execution_complete,
        evidence: Some(result.status.clone()),
    });
    let _ = db::record_task_stage_run(
        &run_id,
        "verification",
        3,
        "running",
        Some("verification result evaluation"),
    );
    let _ = db::record_task_stage_run(
        &run_id,
        "verification",
        3,
        if verification_ok {
            "completed"
        } else {
            "failed"
        },
        Some(&format!("issues={}", verify.issues.join("; "))),
    );
    let _ = db::record_task_stage_assertion(
        &run_id,
        "verification",
        "verification.verify_ok",
        "true",
        &verify_actual,
        verification_ok,
        Some(&verify.issues.join("; ")),
    );
    stage_dod.push(AgentStageDodCheck {
        stage: "verification".to_string(),
        key: "verification.verify_ok".to_string(),
        expected: "true".to_string(),
        actual: verify_actual.clone(),
        passed: verification_ok,
        evidence: Some(verify.issues.join("; ")),
    });
    let business_stage_requirements =
        "requires planner_complete && execution_complete && verify_ok && business_evidence_ok";
    let business_stage_details = format!(
        "{}; evidence={}",
        business_stage_requirements, evidence_detail
    );
    let _ = db::record_task_stage_run(
        &run_id,
        "business",
        4,
        "running",
        Some("business evidence evaluation"),
    );
    let _ = db::record_task_stage_run(
        &run_id,
        "business",
        4,
        if business_complete {
            "completed"
        } else {
            "failed"
        },
        Some(&business_stage_details),
    );
    let _ = db::record_task_stage_assertion(
        &run_id,
        "business",
        "business.business_evidence_ok",
        "true",
        &evidence_actual,
        evidence_ok,
        Some(&evidence_detail),
    );
    let _ = db::upsert_task_run_artifact(
        &run_id,
        "business",
        "business.business_evidence_ok",
        &evidence_actual,
        Some(&evidence_detail),
    );
    stage_dod.push(AgentStageDodCheck {
        stage: "business".to_string(),
        key: "business.business_evidence_ok".to_string(),
        expected: "true".to_string(),
        actual: evidence_actual.clone(),
        passed: evidence_ok,
        evidence: Some(evidence_detail.clone()),
    });
    let _ = db::record_task_stage_assertion(
        &run_id,
        "business",
        "business.business_complete",
        "true",
        &business_actual,
        business_complete,
        Some(business_stage_requirements),
    );
    let _ = db::upsert_task_run_artifact(
        &run_id,
        "business",
        "business.business_complete",
        &business_actual,
        Some(business_stage_requirements),
    );
    stage_dod.push(AgentStageDodCheck {
        stage: "business".to_string(),
        key: "business.business_complete".to_string(),
        expected: "true".to_string(),
        actual: business_actual.clone(),
        passed: business_complete,
        evidence: Some(business_stage_requirements.to_string()),
    });
    for assertion in detect_artifact_evidence_assertions(&plan, &result.logs) {
        let _ = db::record_task_stage_assertion(
            &run_id,
            "business",
            assertion.key,
            assertion.expected.as_str(),
            assertion.actual.as_str(),
            assertion.passed,
            Some(assertion.evidence.as_str()),
        );
        let metadata = json!({
            "expected": assertion.expected.as_str(),
            "passed": assertion.passed,
            "evidence": assertion.evidence.as_str()
        })
        .to_string();
        let _ = db::upsert_task_run_artifact(
            &run_id,
            "artifact_assertion",
            assertion.key,
            assertion.actual.as_str(),
            Some(metadata.as_str()),
        );
        stage_dod.push(AgentStageDodCheck {
            stage: "business".to_string(),
            key: assertion.key.to_string(),
            expected: assertion.expected.to_string(),
            actual: assertion.actual.to_string(),
            passed: assertion.passed,
            evidence: Some(assertion.evidence.to_string()),
        });
    }

    if result.status == "completed" && !business_complete {
        if !evidence_ok {
            result
                .logs
                .push(format!("Business evidence failed: {}", evidence_detail));
        }
        result.logs.push(
            "Final status downgraded: planner/execution complete but business completion failed"
                .to_string(),
        );
        result.status = "error".to_string();
    }

    let task_run_status = if business_complete {
        "business_completed"
    } else if matches!(
        result.status.as_str(),
        "manual_required" | "approval_required" | "blocked"
    ) {
        "business_incomplete"
    } else {
        "business_failed"
    };

    let mut final_nl_status = if business_complete {
        "completed".to_string()
    } else if matches!(
        result.status.as_str(),
        "manual_required" | "approval_required" | "blocked"
    ) {
        result.status.clone()
    } else {
        "error".to_string()
    };

    let mut completion_score = compute_completion_score(
        &final_nl_status,
        planner_complete,
        execution_complete,
        business_complete,
        verification_ok,
        evidence_ok,
        verify.issues.len(),
        result.manual_steps.len(),
    );
    if final_nl_status == "completed" && !completion_score.pass {
        result.logs.push(format!(
            "Final status downgraded: completion score below pass threshold (score={} threshold={})",
            completion_score.score,
            completion_score_pass_threshold()
        ));
        final_nl_status = "error".to_string();
        completion_score = compute_completion_score(
            &final_nl_status,
            planner_complete,
            execution_complete,
            business_complete,
            verification_ok,
            evidence_ok,
            verify.issues.len(),
            result.manual_steps.len(),
        );
    }
    let completion_expected = format!(">= {}", completion_score_pass_threshold());
    let completion_actual = completion_score.score.to_string();
    let completion_reasons = if completion_score.reasons.is_empty() {
        "none".to_string()
    } else {
        completion_score.reasons.join("; ")
    };
    let _ = db::record_task_stage_assertion(
        &run_id,
        "business",
        "business.completion_score",
        &completion_expected,
        &completion_actual,
        completion_score.pass,
        Some(&completion_reasons),
    );
    stage_dod.push(AgentStageDodCheck {
        stage: "business".to_string(),
        key: "business.completion_score".to_string(),
        expected: completion_expected.clone(),
        actual: completion_actual.clone(),
        passed: completion_score.pass,
        evidence: Some(completion_reasons.clone()),
    });
    result.logs.push(format!(
        "Completion score: {} ({}) pass={}",
        completion_score.score, completion_score.label, completion_score.pass
    ));

    let summary = extract_summary(&result.logs);
    let details_json = serde_json::to_string(&result.logs).unwrap_or_default();
    let _ = db::update_task_run_outcome(
        &run_id,
        planner_complete,
        execution_complete,
        business_complete,
        task_run_status,
        summary.as_deref(),
        Some(&details_json),
    );

    if let Some(state) = session {
        let _ = db::insert_nl_run(
            state.intent.intent.as_str(),
            &state.prompt,
            &final_nl_status,
            summary.as_deref(),
            Some(&details_json),
        );
    }
    let response = AgentExecuteResponse {
        status: final_nl_status,
        logs: result.logs,
        approval: result.approval,
        manual_steps: result.manual_steps,
        resume_from: result.resume_from,
        resume_token: result.resume_token,
        run_id: Some(run_id),
        planner_complete,
        execution_complete,
        business_complete,
        completion_score: Some(completion_score),
        profile: Some(execution_profile.as_str().to_string()),
        collision_policy: Some(
            execution_options
                .input_collision_policy
                .as_str()
                .to_string(),
        ),
        stage_dod,
    };

    (StatusCode::OK, Json(response)).into_response()
}

async fn agent_verify_handler(Json(payload): Json<AgentVerifyRequest>) -> impl IntoResponse {
    let Some(plan) = nl_store::get_plan(&payload.plan_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "plan_not_found" })),
        )
            .into_response();
    };

    let result = verification_engine::verify_plan(&plan);
    let response = AgentVerifyResponse {
        ok: result.ok,
        issues: result.issues,
    };

    (StatusCode::OK, Json(response)).into_response()
}

async fn agent_approve_handler(Json(payload): Json<AgentApproveRequest>) -> impl IntoResponse {
    let Some(plan) = nl_store::get_plan(&payload.plan_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "plan_not_found" })),
        )
            .into_response();
    };

    if let Some(decision) = payload.decision.as_deref() {
        approval_gate::register_decision(decision, &payload.action, &plan);
    }
    let decision = approval_gate::preview_approval(&payload.action, &plan);
    let response = AgentApproveResponse {
        status: decision.status,
        requires_approval: decision.requires_approval,
        message: decision.message,
        risk_level: decision.risk_level,
        policy: decision.policy,
    };

    (StatusCode::OK, Json(response)).into_response()
}

struct ArtifactEvidenceAssertion {
    key: &'static str,
    expected: String,
    actual: String,
    passed: bool,
    evidence: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct BusinessEvidenceAssertionSummary {
    pub key: String,
    pub expected: String,
    pub actual: String,
    pub passed: bool,
    pub evidence: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct BusinessEvidenceEvaluation {
    pub ok: bool,
    pub detail: String,
    pub assertions: Vec<BusinessEvidenceAssertionSummary>,
}

fn parse_pipe_fields_with_prefix(line: &str, prefix: &str) -> Option<HashMap<String, String>> {
    let trimmed = line.trim();
    if !trimmed
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
    {
        return None;
    }
    let mut out = HashMap::new();
    for segment in trimmed.split('|').skip(1) {
        if let Some((key, value)) = segment.split_once('=') {
            let key_norm = key.trim().to_ascii_lowercase();
            if key_norm.is_empty() {
                continue;
            }
            out.insert(key_norm, value.trim().to_string());
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn parse_evidence_fields(line: &str) -> Option<HashMap<String, String>> {
    parse_pipe_fields_with_prefix(line, "evidence|")
}

fn parse_run_scope_fields(line: &str) -> Option<HashMap<String, String>> {
    parse_pipe_fields_with_prefix(line, "run_scope|")
}

fn run_scoped_evidence_required() -> bool {
    std::env::var("STEER_REQUIRE_RUN_SCOPED_EVIDENCE")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(true)
}

fn current_run_scope_id(logs: &[String]) -> Option<String> {
    logs.iter().rev().find_map(|line| {
        let fields = parse_run_scope_fields(line)?;
        fields
            .get("run_id")
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    })
}

fn evidence_fields_match_run_scope(
    fields: &HashMap<String, String>,
    run_scope_id: Option<&str>,
) -> bool {
    if !run_scoped_evidence_required() {
        return true;
    }
    let Some(expected_run_id) = run_scope_id else {
        return true;
    };
    fields
        .get("run_id")
        .map(|actual| actual.trim() == expected_run_id)
        .unwrap_or(false)
}

fn stamp_run_scope_evidence(logs: &mut Vec<String>, run_id: &str, plan_id: &str) {
    if logs.is_empty() {
        return;
    }
    if !logs.iter().any(|line| {
        line.to_ascii_lowercase()
            .contains(&format!("run_scope|run_id={}", run_id).to_ascii_lowercase())
    }) {
        logs.insert(
            0,
            format!("RUN_SCOPE|run_id={}|plan_id={}", run_id, plan_id),
        );
    }

    for line in logs.iter_mut() {
        let lower = line.to_ascii_lowercase();
        let is_evidence_line =
            lower.starts_with("evidence|") || lower.starts_with("mail_send_proof|");
        if !is_evidence_line {
            continue;
        }
        if !lower.contains("|run_id=") {
            line.push_str(&format!("|run_id={}", run_id));
        }
        if !lower.contains("|plan_id=") {
            line.push_str(&format!("|plan_id={}", plan_id));
        }
    }
}

fn logs_have_evidence_fields(logs: &[String], expected: &[(&str, &str)]) -> bool {
    let run_scope_id = current_run_scope_id(logs);
    logs.iter().any(|line| {
        let Some(fields) = parse_evidence_fields(line) else {
            return false;
        };
        if !evidence_fields_match_run_scope(&fields, run_scope_id.as_deref()) {
            return false;
        }
        expected.iter().all(|(key, value)| {
            fields
                .get(&key.to_ascii_lowercase())
                .map(|actual| actual.eq_ignore_ascii_case(value))
                .unwrap_or(false)
        })
    })
}

fn latest_evidence_fields(
    logs: &[String],
    target: &str,
    event: &str,
) -> Option<HashMap<String, String>> {
    let run_scope_id = current_run_scope_id(logs);
    logs.iter().rev().find_map(|line| {
        let fields = parse_evidence_fields(line)?;
        if !evidence_fields_match_run_scope(&fields, run_scope_id.as_deref()) {
            return None;
        }
        let target_ok = fields
            .get("target")
            .map(|v| v.eq_ignore_ascii_case(target))
            .unwrap_or(false);
        let event_ok = fields
            .get("event")
            .map(|v| v.eq_ignore_ascii_case(event))
            .unwrap_or(false);
        if target_ok && event_ok {
            Some(fields)
        } else {
            None
        }
    })
}

fn latest_evidence_field(
    logs: &[String],
    target: &str,
    event: &str,
    field: &str,
) -> Option<String> {
    latest_evidence_fields(logs, target, event).and_then(|fields| {
        fields
            .get(&field.to_ascii_lowercase())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn latest_legacy_mail_send_field(logs: &[String], field: &str) -> Option<String> {
    let run_scope_id = current_run_scope_id(logs);
    logs.iter().rev().find_map(|line| {
        let fields = parse_pipe_fields_with_prefix(line, "mail_send_proof|")?;
        if !evidence_fields_match_run_scope(&fields, run_scope_id.as_deref()) {
            return None;
        }
        fields
            .get(&field.to_ascii_lowercase())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn latest_evidence_int(logs: &[String], target: &str, event: &str, field: &str) -> Option<i64> {
    latest_evidence_field(logs, target, event, field).and_then(|v| v.parse::<i64>().ok())
}

fn detect_artifact_evidence_assertions(
    plan: &crate::nl_automation::Plan,
    logs: &[String],
) -> Vec<ArtifactEvidenceAssertion> {
    let plan_text = plan
        .steps
        .iter()
        .map(|step| step.description.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    let lowered_logs: Vec<String> = logs.iter().map(|line| line.to_lowercase()).collect();
    let joined_logs = lowered_logs.join("\n");

    let keyword_required =
        |keywords: &[&str]| -> bool { keywords.iter().any(|keyword| plan_text.contains(keyword)) };
    let marker_confirmed =
        |markers: &[&str]| -> bool { markers.iter().any(|marker| joined_logs.contains(marker)) };

    let mail_required = keyword_required(&["mail", "email", "이메일"]);
    let notes_required = keyword_required(&["notes", "note", "메모"]);
    let textedit_required = keyword_required(&["textedit", "텍스트편집", "text edit"]);
    let notion_required = keyword_required(&["notion", "노션"]);
    let telegram_required = keyword_required(&["telegram", "텔레그램"]);

    let mail_sent_confirmed =
        logs_have_evidence_fields(
            logs,
            &[
                ("target", "mail"),
                ("event", "send"),
                ("status", "sent_confirmed"),
            ],
        ) || marker_confirmed(&["mail_send_proof|status=sent_confirmed", "mail sent"]);
    let notes_write_confirmed = logs_have_evidence_fields(
        logs,
        &[
            ("target", "notes"),
            ("event", "write"),
            ("status", "confirmed"),
        ],
    ) || marker_confirmed(&[
        "notes_write_confirmed",
        "notes_write_text",
        "notes appended",
    ]);
    let textedit_write_confirmed = logs_have_evidence_fields(
        logs,
        &[
            ("target", "textedit"),
            ("event", "write"),
            ("status", "confirmed"),
        ],
    ) || marker_confirmed(&[
        "textedit_write_confirmed",
        "textedit_append_text",
        "shared via textedit",
    ]);
    let textedit_save_confirmed =
        logs_have_evidence_fields(
            logs,
            &[
                ("target", "textedit"),
                ("event", "save"),
                ("status", "confirmed"),
            ],
        ) || marker_confirmed(&["textedit_save_confirmed", "saved in textedit", "cmd+s"]);
    let notion_write_confirmed =
        logs_have_evidence_fields(
            logs,
            &[
                ("target", "notion"),
                ("event", "write"),
                ("status", "confirmed"),
            ],
        ) || marker_confirmed(&["notion: https://www.notion.so", "notion page created"]);
    let telegram_delivery_confirmed = logs_have_evidence_fields(
        logs,
        &[
            ("target", "telegram"),
            ("event", "send"),
            ("status", "sent"),
        ],
    ) || marker_confirmed(&["telegram: sent", "telegram sent"]);
    let notes_structured = latest_evidence_fields(logs, "notes", "write");
    let notes_note_id = notes_structured
        .as_ref()
        .and_then(|fields| fields.get("note_id").map(|v| v.trim().to_string()))
        .unwrap_or_default();
    let notes_body_len = latest_evidence_int(logs, "notes", "write", "body_len").unwrap_or(-1);
    let notes_note_id_required = notes_required && notes_structured.is_some();

    let textedit_write_structured = latest_evidence_fields(logs, "textedit", "write");
    let textedit_doc_id = textedit_write_structured
        .as_ref()
        .and_then(|fields| fields.get("doc_id").map(|v| v.trim().to_string()))
        .unwrap_or_default();
    let textedit_body_len =
        latest_evidence_int(logs, "textedit", "write", "body_len").unwrap_or(-1);
    let textedit_doc_id_required = textedit_required && textedit_write_structured.is_some();
    let run_scope_id = current_run_scope_id(logs);
    let scoped_evidence_count = logs
        .iter()
        .filter_map(|line| parse_evidence_fields(line))
        .filter(|fields| evidence_fields_match_run_scope(fields, run_scope_id.as_deref()))
        .count();
    let run_scope_required = run_scoped_evidence_required();
    let run_scope_present = run_scope_id.is_some();

    let mut out = vec![
        ArtifactEvidenceAssertion {
            key: "artifact.run_scope_present",
            expected: if run_scope_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: run_scope_present.to_string(),
            passed: if run_scope_required {
                run_scope_present
            } else {
                true
            },
            evidence: format!(
                "run_scope_required={} run_scope_id={} scoped_evidence_count={}",
                run_scope_required,
                run_scope_id.clone().unwrap_or_default(),
                scoped_evidence_count
            ),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.mail_sent_confirmed",
            expected: if mail_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: mail_sent_confirmed.to_string(),
            passed: if mail_required {
                mail_sent_confirmed
            } else {
                true
            },
            evidence: format!(
                "required={} marker_hint={} plan_keywords={}",
                mail_required,
                "EVIDENCE target=mail,event=send,status=sent_confirmed or legacy marker",
                plan_text
            ),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.notes_write_confirmed",
            expected: if notes_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: notes_write_confirmed.to_string(),
            passed: if notes_required {
                notes_write_confirmed
            } else {
                true
            },
            evidence: format!(
                "required={} marker_hint={} plan_keywords={}",
                notes_required,
                "EVIDENCE target=notes,event=write,status=confirmed or legacy marker",
                plan_text
            ),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.notes_note_id_present",
            expected: if notes_note_id_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: (!notes_note_id.is_empty()).to_string(),
            passed: if notes_note_id_required {
                !notes_note_id.is_empty()
            } else {
                true
            },
            evidence: format!(
                "structured_required={} note_id={}",
                notes_note_id_required, notes_note_id
            ),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.notes_body_nonempty",
            expected: if notes_required {
                ">2".to_string()
            } else {
                "optional".to_string()
            },
            actual: notes_body_len.to_string(),
            passed: if notes_required {
                notes_body_len > 2
            } else {
                true
            },
            evidence: "notes write body_len from EVIDENCE target=notes,event=write".to_string(),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.textedit_write_confirmed",
            expected: if textedit_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: textedit_write_confirmed.to_string(),
            passed: if textedit_required {
                textedit_write_confirmed
            } else {
                true
            },
            evidence: format!(
                "required={} marker_hint={} plan_keywords={}",
                textedit_required,
                "EVIDENCE target=textedit,event=write,status=confirmed or legacy marker",
                plan_text
            ),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.textedit_doc_id_present",
            expected: if textedit_doc_id_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: (!textedit_doc_id.is_empty()).to_string(),
            passed: if textedit_doc_id_required {
                !textedit_doc_id.is_empty()
            } else {
                true
            },
            evidence: format!(
                "structured_required={} doc_id={}",
                textedit_doc_id_required, textedit_doc_id
            ),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.textedit_body_nonempty",
            expected: if textedit_required {
                ">2".to_string()
            } else {
                "optional".to_string()
            },
            actual: textedit_body_len.to_string(),
            passed: if textedit_required {
                textedit_body_len > 2
            } else {
                true
            },
            evidence: "textedit write body_len from EVIDENCE target=textedit,event=write"
                .to_string(),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.textedit_save_confirmed",
            expected: if textedit_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: textedit_save_confirmed.to_string(),
            passed: if textedit_required {
                textedit_save_confirmed
            } else {
                true
            },
            evidence: format!(
                "required={} marker_hint={} plan_keywords={}",
                textedit_required,
                "EVIDENCE target=textedit,event=save,status=confirmed or legacy marker",
                plan_text
            ),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.notion_write_confirmed",
            expected: if notion_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: notion_write_confirmed.to_string(),
            passed: if notion_required {
                notion_write_confirmed
            } else {
                true
            },
            evidence: format!(
                "required={} marker_hint={} plan_keywords={}",
                notion_required,
                "EVIDENCE target=notion,event=write,status=confirmed or legacy marker",
                plan_text
            ),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.telegram_delivery_confirmed",
            expected: if telegram_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: telegram_delivery_confirmed.to_string(),
            passed: if telegram_required {
                telegram_delivery_confirmed
            } else {
                true
            },
            evidence: format!(
                "required={} marker_hint={} plan_keywords={}",
                telegram_required,
                "EVIDENCE target=telegram,event=send,status=sent or legacy marker",
                plan_text
            ),
        },
    ];

    if mail_required {
        let mail_recipient = latest_evidence_field(logs, "mail", "send", "recipient")
            .or_else(|| latest_legacy_mail_send_field(logs, "recipient"))
            .unwrap_or_default();
        let recipient_present = !mail_recipient.trim().is_empty();
        out.push(ArtifactEvidenceAssertion {
            key: "artifact.mail_recipient_present",
            expected: "true".to_string(),
            actual: recipient_present.to_string(),
            passed: recipient_present,
            evidence: format!("mail_recipient={}", mail_recipient),
        });

        let mail_body_len = latest_evidence_int(logs, "mail", "send", "body_len").or_else(|| {
            latest_legacy_mail_send_field(logs, "body_len").and_then(|v| v.parse::<i64>().ok())
        });
        let body_len_value = mail_body_len.unwrap_or(-1);
        out.push(ArtifactEvidenceAssertion {
            key: "artifact.mail_body_nonempty",
            expected: ">2".to_string(),
            actual: body_len_value.to_string(),
            passed: body_len_value > 2,
            evidence: "mail body evidence from EVIDENCE/mail_send_proof".to_string(),
        });
    }

    let notion_structured = latest_evidence_fields(logs, "notion", "write");
    let notion_page_required = notion_required && notion_structured.is_some();
    let notion_page_id = notion_structured
        .as_ref()
        .and_then(|fields| fields.get("page_id").map(|v| v.trim().to_string()))
        .unwrap_or_default();
    out.push(ArtifactEvidenceAssertion {
        key: "artifact.notion_page_id_present",
        expected: if notion_page_required {
            "true".to_string()
        } else {
            "optional".to_string()
        },
        actual: (!notion_page_id.is_empty()).to_string(),
        passed: if notion_page_required {
            !notion_page_id.is_empty()
        } else {
            true
        },
        evidence: format!(
            "structured_required={} page_id={}",
            notion_page_required, notion_page_id
        ),
    });

    let telegram_structured = latest_evidence_fields(logs, "telegram", "send");
    let telegram_message_required = telegram_required && telegram_structured.is_some();
    let telegram_message_id = telegram_structured
        .as_ref()
        .and_then(|fields| fields.get("message_id").map(|v| v.trim().to_string()))
        .unwrap_or_default();
    out.push(ArtifactEvidenceAssertion {
        key: "artifact.telegram_message_id_present",
        expected: if telegram_message_required {
            "true".to_string()
        } else {
            "optional".to_string()
        },
        actual: (!telegram_message_id.is_empty()).to_string(),
        passed: if telegram_message_required {
            !telegram_message_id.is_empty()
        } else {
            true
        },
        evidence: format!(
            "structured_required={} message_id={}",
            telegram_message_required, telegram_message_id
        ),
    });

    out
}

fn evaluate_business_evidence(
    plan: &crate::nl_automation::Plan,
    logs: &[String],
) -> (bool, String) {
    let lowered_logs: Vec<String> = logs.iter().map(|line| line.to_lowercase()).collect();
    let mut issues: Vec<String> = Vec::new();

    let blocking_markers = [
        "execution paused awaiting approval",
        "execution paused for manual input",
        "approval required before continuing",
        "execution blocked by policy",
        "manual input required",
        "manual filters required",
    ];
    for marker in blocking_markers {
        if lowered_logs.iter().any(|line| line.contains(marker)) {
            issues.push(format!("blocking signal present: {}", marker));
            break;
        }
    }

    if lowered_logs
        .iter()
        .any(|line| line.contains("no summary extracted"))
    {
        issues.push("summary extraction returned empty".to_string());
    }

    let summaries: Vec<String> = logs
        .iter()
        .filter_map(|line| line.strip_prefix("Summary: ").map(|s| s.trim().to_string()))
        .collect();
    let meaningful_summary = summaries
        .iter()
        .find(|summary| is_meaningful_summary(plan, summary))
        .cloned();
    let generic_intent = matches!(plan.intent, crate::nl_automation::IntentType::GenericTask);
    if meaningful_summary.is_none() && (!generic_intent || !summaries.is_empty()) {
        issues.push("missing meaningful summary output".to_string());
    }
    issues.extend(validate_intent_business_contract(
        plan,
        meaningful_summary.as_deref(),
        logs,
    ));

    if issues.is_empty() {
        let summary = meaningful_summary
            .or_else(|| summaries.first().cloned())
            .unwrap_or_else(|| "n/a".to_string());
        return (true, format!("summary=\"{}\"", summary));
    }

    (false, issues.join("; "))
}

pub(crate) fn evaluate_business_evidence_with_assertions(
    plan: &crate::nl_automation::Plan,
    logs: &[String],
) -> BusinessEvidenceEvaluation {
    let (ok, detail) = evaluate_business_evidence(plan, logs);
    let assertions = detect_artifact_evidence_assertions(plan, logs)
        .into_iter()
        .map(|assertion| BusinessEvidenceAssertionSummary {
            key: assertion.key.to_string(),
            expected: assertion.expected,
            actual: assertion.actual,
            passed: assertion.passed,
            evidence: assertion.evidence,
        })
        .collect();
    BusinessEvidenceEvaluation {
        ok,
        detail,
        assertions,
    }
}

fn normalize_contract_token(input: &str) -> String {
    input
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || ['-', '_', '@', '.'].contains(c))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn summary_contains_token(summary: &str, token: &str) -> bool {
    let token_norm = normalize_contract_token(token);
    if token_norm.is_empty() {
        return true;
    }
    let summary_norm = normalize_contract_token(summary);
    summary_norm.contains(&token_norm)
}

fn slot_value(plan: &crate::nl_automation::Plan, key: &str) -> Option<String> {
    plan.slots
        .get(key)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn validate_intent_business_contract(
    plan: &crate::nl_automation::Plan,
    summary: Option<&str>,
    logs: &[String],
) -> Vec<String> {
    let mut issues = Vec::new();
    let summary_text = summary.unwrap_or("");
    let joined_logs = logs.join("\n").to_lowercase();
    let plan_text = plan
        .steps
        .iter()
        .map(|step| step.description.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let plan_text_lower = plan_text.to_lowercase();
    let contains_any = |haystack: &str, needles: &[&str]| -> bool {
        needles.iter().any(|needle| haystack.contains(needle))
    };
    let plan_data_lower = plan
        .steps
        .iter()
        .map(|step| step.data.to_string().to_lowercase())
        .collect::<Vec<_>>()
        .join("\n");
    let data_contains_any = |needles: &[&str]| -> bool { contains_any(&plan_data_lower, needles) };
    let step_matches = |app_needles: &[&str], action_needles: &[&str]| -> bool {
        plan.steps.iter().any(|step| {
            let desc = step.description.to_lowercase();
            let data = step.data.to_string().to_lowercase();
            let app_hit = app_needles
                .iter()
                .any(|needle| desc.contains(needle) || data.contains(needle));
            if !app_hit {
                return false;
            }
            action_needles
                .iter()
                .any(|needle| desc.contains(needle) || data.contains(needle))
        })
    };
    let expected_recipients = {
        let mut recipients = crate::semantic_contract::extract_expected_recipients(&plan_text);
        for value in plan.slots.values() {
            for candidate in crate::semantic_contract::extract_expected_recipients(value) {
                if !recipients
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(&candidate))
                {
                    recipients.push(candidate);
                }
            }
        }
        recipients
    };
    let mail_body_len = latest_evidence_int(logs, "mail", "send", "body_len").or_else(|| {
        latest_legacy_mail_send_field(logs, "body_len").and_then(|raw| raw.parse::<i64>().ok())
    });

    match plan.intent {
        crate::nl_automation::IntentType::FlightSearch => {
            let required = [
                ("from", slot_value(plan, "from")),
                ("to", slot_value(plan, "to")),
                ("date_start", slot_value(plan, "date_start")),
            ];
            for (key, value) in required {
                if let Some(v) = value {
                    let summary_ok = summary_contains_token(summary_text, &v);
                    let log_ok = joined_logs.contains(&format!("auto fill succeeded for {}", key))
                        || joined_logs.contains(&format!("filled {}=", key))
                        || joined_logs.contains(&format!("slot {}=", key));
                    if !summary_ok && !log_ok {
                        issues.push(format!("contract_missing_{}={}", key, v));
                    }
                }
            }
        }
        crate::nl_automation::IntentType::ShoppingCompare => {
            if let Some(product) = slot_value(plan, "product_name") {
                if !summary_contains_token(summary_text, &product) {
                    issues.push(format!("contract_missing_product_name={}", product));
                }
            }
            if summary_text.to_lowercase().contains("unknown") {
                issues.push("shopping_summary_contains_unknown".to_string());
            }
        }
        crate::nl_automation::IntentType::FormFill => {
            if let Some(purpose) = slot_value(plan, "form_purpose") {
                if !summary_contains_token(summary_text, &purpose) {
                    issues.push(format!("contract_missing_form_purpose={}", purpose));
                }
            }
            let has_fill_signal = joined_logs.contains("auto fill succeeded")
                || joined_logs.contains("auto input attempted")
                || joined_logs.contains("manual input required");
            if !has_fill_signal {
                issues.push("form_fill_execution_signal_missing".to_string());
            }
        }
        crate::nl_automation::IntentType::GenericTask => {
            let has_meaningful_summary = !summary_text.trim().is_empty();
            let has_step_signal = joined_logs.contains("step 1:")
                || joined_logs.contains("run_attempt|phase=execution_start");
            if !has_meaningful_summary && !has_step_signal {
                issues.push("generic_execution_signal_missing".to_string());
            }

            let mail_send_required =
                (contains_any(&plan_text_lower, &["mail", "email", "메일", "이메일"])
                    && contains_any(&plan_text_lower, &["send", "보내", "발송", "전송"]))
                    || data_contains_any(&["mail_send", "gmail_send", "\"action\":\"mail_send\""]);
            if mail_send_required {
                let mail_structured = latest_evidence_fields(logs, "mail", "send");
                let mail_sent_confirmed = logs_have_evidence_fields(
                    logs,
                    &[
                        ("target", "mail"),
                        ("event", "send"),
                        ("status", "sent_confirmed"),
                    ],
                ) || (mail_structured.is_none()
                    && contains_any(
                        &joined_logs,
                        &[
                            "mail_send_proof|status=sent_confirmed",
                            "mail send completed",
                            "(mail sent)",
                            "mail sent",
                        ],
                    ));
                if !mail_sent_confirmed {
                    issues.push("contract_missing_mail_send_confirmation".to_string());
                }

                let mail_recipient = latest_evidence_field(logs, "mail", "send", "recipient")
                    .or_else(|| latest_legacy_mail_send_field(logs, "recipient"))
                    .unwrap_or_default();
                if mail_sent_confirmed && mail_recipient.trim().is_empty() {
                    issues.push("contract_missing_mail_recipient_evidence".to_string());
                }

                for recipient in &expected_recipients {
                    let needle = recipient.to_lowercase();
                    if !joined_logs.contains(&needle)
                        && !summary_text.to_lowercase().contains(&needle)
                    {
                        issues.push(format!("contract_missing_mail_recipient={}", recipient));
                    }
                }

                if mail_sent_confirmed && matches!(mail_body_len, Some(len) if len <= 2) {
                    issues.push("contract_mail_body_empty".to_string());
                }
            }

            let notes_write_required = step_matches(
                &["notes", "note", "메모"],
                &["write", "append", "type", "작성", "입력", "기록", "붙여넣"],
            ) || data_contains_any(&[
                "notes_write_text",
                "\"action\":\"notes_write\"",
                "\"target\":\"notes\"",
            ]);
            if notes_write_required {
                let notes_structured = latest_evidence_fields(logs, "notes", "write");
                let notes_write_confirmed = logs_have_evidence_fields(
                    logs,
                    &[
                        ("target", "notes"),
                        ("event", "write"),
                        ("status", "confirmed"),
                    ],
                ) || (notes_structured.is_none()
                    && contains_any(
                        &joined_logs,
                        &[
                            "notes_write_confirmed",
                            "notes_write_text",
                            "(notes body)",
                            "notes appended",
                        ],
                    ));
                if !notes_write_confirmed {
                    issues.push("contract_missing_notes_write_confirmation".to_string());
                }
                if notes_write_confirmed && notes_structured.is_some() {
                    let note_id = latest_evidence_field(logs, "notes", "write", "note_id")
                        .unwrap_or_default();
                    if note_id.trim().is_empty() {
                        issues.push("contract_missing_notes_note_id".to_string());
                    }
                }
                let notes_body_len = latest_evidence_int(logs, "notes", "write", "body_len");
                if notes_write_confirmed && matches!(notes_body_len, Some(len) if len <= 2) {
                    issues.push("contract_notes_body_empty".to_string());
                }
            }

            let textedit_write_required = step_matches(
                &["textedit", "텍스트편집", "text edit"],
                &["write", "append", "type", "작성", "입력", "기록", "붙여넣"],
            ) || data_contains_any(&[
                "textedit_append_text",
                "\"action\":\"textedit_append_text\"",
                "\"target\":\"textedit\"",
            ]);
            if textedit_write_required {
                let textedit_structured = latest_evidence_fields(logs, "textedit", "write");
                let textedit_write_confirmed = logs_have_evidence_fields(
                    logs,
                    &[
                        ("target", "textedit"),
                        ("event", "write"),
                        ("status", "confirmed"),
                    ],
                ) || (textedit_structured.is_none()
                    && contains_any(
                        &joined_logs,
                        &[
                            "textedit_write_confirmed",
                            "textedit_append_text",
                            "(textedit body)",
                            "shared via textedit",
                        ],
                    ));
                if !textedit_write_confirmed {
                    issues.push("contract_missing_textedit_write_confirmation".to_string());
                }
                if textedit_write_confirmed && textedit_structured.is_some() {
                    let doc_id = latest_evidence_field(logs, "textedit", "write", "doc_id")
                        .unwrap_or_default();
                    if doc_id.trim().is_empty() {
                        issues.push("contract_missing_textedit_doc_id".to_string());
                    }
                }
                let textedit_body_len = latest_evidence_int(logs, "textedit", "write", "body_len");
                if textedit_write_confirmed && matches!(textedit_body_len, Some(len) if len <= 2) {
                    issues.push("contract_textedit_body_empty".to_string());
                }
            }

            let notion_write_required = (contains_any(&plan_text_lower, &["notion", "노션"])
                && contains_any(
                    &plan_text_lower,
                    &[
                        "write",
                        "create",
                        "append",
                        "작성",
                        "기록",
                        "저장",
                        "업데이트",
                    ],
                ))
                || data_contains_any(&[
                    "notion_create",
                    "notion_write",
                    "\"action\":\"notion_create\"",
                    "\"action\":\"notion_write\"",
                ]);
            if notion_write_required {
                let notion_structured = latest_evidence_fields(logs, "notion", "write");
                let notion_write_confirmed = logs_have_evidence_fields(
                    logs,
                    &[
                        ("target", "notion"),
                        ("event", "write"),
                        ("status", "confirmed"),
                    ],
                ) || (notion_structured.is_none()
                    && contains_any(
                        &joined_logs,
                        &[
                            "notion: https://www.notion.so",
                            "notion page created",
                            "notion_write_confirmed",
                        ],
                    ));
                if !notion_write_confirmed {
                    issues.push("contract_missing_notion_write_confirmation".to_string());
                }
                if notion_write_confirmed && notion_structured.is_some() {
                    let notion_page_id = latest_evidence_field(logs, "notion", "write", "page_id")
                        .unwrap_or_default();
                    if notion_page_id.trim().is_empty() {
                        issues.push("contract_missing_notion_page_id".to_string());
                    }
                }
            }

            let telegram_send_required =
                (contains_any(&plan_text_lower, &["telegram", "텔레그램"])
                    && contains_any(&plan_text_lower, &["send", "보내", "발송", "전송"]))
                    || data_contains_any(&[
                        "telegram_send",
                        "\"action\":\"telegram_send\"",
                        "\"type\":\"telegram\"",
                    ]);
            if telegram_send_required {
                let telegram_structured = latest_evidence_fields(logs, "telegram", "send");
                let telegram_send_confirmed = logs_have_evidence_fields(
                    logs,
                    &[
                        ("target", "telegram"),
                        ("event", "send"),
                        ("status", "sent"),
                    ],
                ) || (telegram_structured.is_none()
                    && contains_any(
                        &joined_logs,
                        &[
                            "telegram: sent",
                            "telegram sent",
                            "telegram_message_sent",
                            "telegram_send",
                        ],
                    ));
                if !telegram_send_confirmed {
                    issues.push("contract_missing_telegram_send_confirmation".to_string());
                }
                if telegram_send_confirmed && telegram_structured.is_some() {
                    let message_id = latest_evidence_field(logs, "telegram", "send", "message_id")
                        .unwrap_or_default();
                    if message_id.trim().is_empty() {
                        issues.push("contract_missing_telegram_message_id".to_string());
                    }
                }
            }

            let textedit_save_required =
                (contains_any(&plan_text_lower, &["textedit", "텍스트편집", "text edit"])
                    && contains_any(&plan_text_lower, &["save", "저장"]))
                    || data_contains_any(&[
                        "textedit_save",
                        "textedit_save_confirmed",
                        "\"app\":\"textedit\"",
                    ]);
            if textedit_save_required {
                let textedit_save_confirmed = logs_have_evidence_fields(
                    logs,
                    &[
                        ("target", "textedit"),
                        ("event", "save"),
                        ("status", "confirmed"),
                    ],
                ) || contains_any(
                    &joined_logs,
                    &[
                        "textedit_save_confirmed",
                        "saved in textedit",
                        "cmd+s",
                        "shortcut 's' + [\"command\"]",
                    ],
                );
                if !textedit_save_confirmed {
                    issues.push("contract_missing_textedit_save_confirmation".to_string());
                }
            }
        }
    }

    let mut explicit_assertions = crate::semantic_contract::extract_required_assertions(&plan_text);
    for slot_value in plan.slots.values() {
        for key in crate::semantic_contract::extract_required_assertions(slot_value) {
            if !explicit_assertions
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&key))
            {
                explicit_assertions.push(key);
            }
        }
    }
    if !explicit_assertions.is_empty() {
        let assertion_map: HashMap<String, bool> = detect_artifact_evidence_assertions(plan, logs)
            .into_iter()
            .map(|assertion| {
                let actual_true = assertion.actual.eq_ignore_ascii_case("true");
                (assertion.key.to_ascii_lowercase(), actual_true)
            })
            .collect();
        for required_key in explicit_assertions {
            let normalized = required_key.to_ascii_lowercase();
            match assertion_map.get(&normalized) {
                Some(true) => {}
                Some(false) => issues.push(format!(
                    "contract_required_assertion_failed={}",
                    required_key
                )),
                None => issues.push(format!(
                    "contract_required_assertion_unknown={}",
                    required_key
                )),
            }
        }
    }

    issues
}

fn completion_score_pass_threshold() -> u8 {
    std::env::var("STEER_COMPLETION_SCORE_PASS")
        .ok()
        .and_then(|v| v.trim().parse::<u8>().ok())
        .map(|v| v.min(100))
        .unwrap_or(75)
}

#[allow(clippy::too_many_arguments)]
fn compute_completion_score(
    status: &str,
    planner_complete: bool,
    execution_complete: bool,
    business_complete: bool,
    verification_ok: bool,
    evidence_ok: bool,
    verify_issue_count: usize,
    manual_step_count: usize,
) -> AgentCompletionScore {
    let mut score: i32 = 0;
    let mut reasons: Vec<String> = Vec::new();

    if planner_complete {
        score += 15;
    } else {
        reasons.push("planner_incomplete".to_string());
    }
    if execution_complete {
        score += 20;
    } else {
        reasons.push("execution_incomplete".to_string());
    }
    if verification_ok {
        score += 20;
    } else {
        reasons.push("verification_failed".to_string());
    }
    if evidence_ok {
        score += 15;
    } else {
        reasons.push("business_evidence_failed".to_string());
    }
    if business_complete {
        score += 20;
    } else {
        reasons.push("business_incomplete".to_string());
    }
    if matches!(status, "completed" | "success") {
        score += 10;
    } else {
        reasons.push(format!("final_status={}", status));
        if matches!(status, "error" | "failed" | "blocked") {
            score -= 10;
        }
    }

    if verify_issue_count > 0 {
        let penalty = (verify_issue_count as i32).min(5) * 2;
        score -= penalty;
        reasons.push(format!("verify_issues={}", verify_issue_count));
    }
    if manual_step_count > 0 {
        let penalty = (manual_step_count as i32).min(5) * 2;
        score -= penalty;
        reasons.push(format!("manual_steps={}", manual_step_count));
    }

    score = score.clamp(0, 100);
    let score_u8 = score as u8;
    let label = if score_u8 >= 90 {
        "Excellent"
    } else if score_u8 >= 75 {
        "Good"
    } else if score_u8 >= 60 {
        "Needs tuning"
    } else {
        "Risky"
    };
    let pass = score_u8 >= completion_score_pass_threshold();

    AgentCompletionScore {
        score: score_u8,
        label: label.to_string(),
        pass,
        reasons,
    }
}

fn is_meaningful_summary(plan: &crate::nl_automation::Plan, summary: &str) -> bool {
    let normalized = summary.trim().to_lowercase();
    if normalized.is_empty() {
        return false;
    }
    if matches!(normalized.as_str(), "need more details" | "n/a" | "unknown") {
        return false;
    }
    if normalized.contains("no summary extracted") {
        return false;
    }
    if matches!(
        plan.intent,
        crate::nl_automation::IntentType::FlightSearch
            | crate::nl_automation::IntentType::ShoppingCompare
            | crate::nl_automation::IntentType::FormFill
    ) && normalized.contains("unknown")
    {
        return false;
    }
    true
}

fn extract_summary(logs: &[String]) -> Option<String> {
    logs.iter()
        .find_map(|line| line.strip_prefix("Summary: ").map(|s| s.to_string()))
}

async fn handle_chat_feedback(Json(req): Json<ChatFeedbackRequest>) -> Json<ChatFeedbackResponse> {
    let sentiment = req.sentiment.trim().to_ascii_lowercase();
    if !matches!(sentiment.as_str(), "positive" | "negative") {
        return Json(ChatFeedbackResponse {
            ok: false,
            request_memory_updated: false,
            execution_memory_updated: false,
            reuse_suppressed: false,
        });
    }

    let memory_scope = chat_feedback_memory_scope(&req);
    let memory_scope_ref = memory_scope.as_deref();
    let request_memory_updated = db::record_request_memory_feedback_scoped(
        memory_scope_ref,
        &req.request_text,
        &req.response_text,
        &sentiment,
    )
    .unwrap_or(false);

    let intent = request_memory_intent_for_feedback(memory_scope_ref, &req.request_text);
    let command = req
        .command
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            intent
                .as_ref()
                .and_then(|value| value.get("command"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        });

    let execution_memory_updated = if let (Some(command), Some(intent)) = (command, intent) {
        if execution_memory_supported(&command) {
            if let Some((params_key, _)) = execution_memory_params(&command, &intent) {
                db::record_execution_memory_feedback_scoped(
                    memory_scope_ref,
                    &command,
                    &params_key,
                    &req.response_text,
                    &sentiment,
                )
                .unwrap_or(false)
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    Json(ChatFeedbackResponse {
        ok: request_memory_updated || execution_memory_updated,
        request_memory_updated,
        execution_memory_updated,
        reuse_suppressed: sentiment == "negative"
            && (request_memory_updated || execution_memory_updated),
    })
}

async fn handle_feedback(
    State(state): State<AppState>,
    Json(req): Json<FeedbackRequest>,
) -> Json<FeedbackResponse> {
    let goal = if req.goal.trim().is_empty() {
        state
            .current_goal
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .unwrap_or_default()
    } else {
        req.goal.clone()
    };
    let history = req
        .history_summary
        .unwrap_or_else(|| format!("Goal: {}", goal));
    if let Some(llm) = &state.llm_client {
        match llm.analyze_user_feedback(&req.feedback, &history).await {
            Ok(analysis) => {
                let action = analysis.action.clone();
                let new_goal = if action == "refine" {
                    analysis.new_goal.clone().or(Some(goal))
                } else {
                    None
                };
                let message = if action == "refine" {
                    "피드백을 반영해 목표를 업데이트했어요. 다시 실행할 수 있어요.".to_string()
                } else {
                    "피드백을 확인했어요. 작업을 완료로 표시합니다.".to_string()
                };
                return Json(FeedbackResponse {
                    action,
                    new_goal,
                    message,
                });
            }
            Err(e) => {
                eprintln!("Feedback analysis failed: {}", e);
            }
        }
    }

    Json(FeedbackResponse {
        action: "complete".to_string(),
        new_goal: None,
        message: "피드백을 확인했어요. 작업을 완료로 표시합니다.".to_string(),
    })
}

// [Context] Selection Handler
async fn get_selection_context() -> Json<serde_json::Value> {
    #[cfg(target_os = "macos")]
    {
        match crate::macos::accessibility::get_selected_text() {
            Some(text) => Json(serde_json::json!({ "found": true, "text": text })),
            None => Json(serde_json::json!({ "found": false, "text": "" })),
        }
    }
    #[cfg(not(target_os = "macos"))]
    Json(serde_json::json!({ "found": false, "text": "", "error": "Not supported on this OS" }))
}

// =====================================================
// SESSION MANAGEMENT HANDLERS (Clawdbot-ported)
// =====================================================

/// List all sessions
async fn list_sessions_handler() -> Json<serde_json::Value> {
    let _ = crate::session_store::init_session_store();

    match crate::session_store::get_session_store() {
        Ok(guard) => {
            if let Some(store) = guard.as_ref() {
                let sessions: Vec<serde_json::Value> = store
                    .list_active()
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "id": s.id,
                            "key": s.key,
                            "goal": s.goal,
                            "status": format!("{:?}", s.status),
                            "created_at": s.created_at.to_rfc3339(),
                            "updated_at": s.updated_at.to_rfc3339(),
                            "steps_count": s.steps.len(),
                            "can_resume": s.can_resume(),
                        })
                    })
                    .collect();

                Json(serde_json::json!({
                    "success": true,
                    "sessions": sessions,
                    "total": sessions.len()
                }))
            } else {
                Json(serde_json::json!({ "success": false, "error": "Store not initialized" }))
            }
        }
        Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() })),
    }
}

/// Get specific session by ID
async fn get_session_handler(Path(id): Path<String>) -> Json<serde_json::Value> {
    let _ = crate::session_store::init_session_store();

    match crate::session_store::get_session_store() {
        Ok(guard) => {
            if let Some(store) = guard.as_ref() {
                if let Some(session) = store.get(&id) {
                    Json(serde_json::json!({
                        "success": true,
                        "session": {
                            "id": session.id,
                            "key": session.key,
                            "goal": session.goal,
                            "status": format!("{:?}", session.status),
                            "created_at": session.created_at.to_rfc3339(),
                            "updated_at": session.updated_at.to_rfc3339(),
                            "messages": session.messages,
                            "steps": session.steps,
                            "can_resume": session.can_resume(),
                            "resume_point": session.get_resume_point()
                        }
                    }))
                } else {
                    Json(serde_json::json!({ "success": false, "error": "Session not found" }))
                }
            } else {
                Json(serde_json::json!({ "success": false, "error": "Store not initialized" }))
            }
        }
        Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() })),
    }
}

/// Delete a session
async fn delete_session_handler(Path(id): Path<String>) -> Json<serde_json::Value> {
    let _ = crate::session_store::init_session_store();

    match crate::session_store::get_session_store() {
        Ok(mut guard) => {
            if let Some(store) = guard.as_mut() {
                match store.delete(&id) {
                    Ok(true) => Json(serde_json::json!({ "success": true, "deleted": id })),
                    Ok(false) => {
                        Json(serde_json::json!({ "success": false, "error": "Session not found" }))
                    }
                    Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() })),
                }
            } else {
                Json(serde_json::json!({ "success": false, "error": "Store not initialized" }))
            }
        }
        Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() })),
    }
}

/// Resume a paused/failed session
async fn resume_session_handler(Path(id): Path<String>) -> Json<serde_json::Value> {
    let _ = crate::session_store::init_session_store();

    match crate::session_store::get_session_store() {
        Ok(mut guard) => {
            if let Some(store) = guard.as_mut() {
                if let Some(session) = store.get_mut(&id) {
                    if session.can_resume() {
                        let resume_point = session.get_resume_point();
                        let goal = session.goal.clone();
                        session.status = crate::session_store::SessionStatus::Active;
                        session
                            .add_message("system", &format!("Resuming from step {}", resume_point));

                        Json(serde_json::json!({
                            "success": true,
                            "resumed": true,
                            "session_id": id,
                            "goal": goal,
                            "resume_from_step": resume_point,
                            "message": format!("Session resumed from step {}", resume_point)
                        }))
                    } else {
                        Json(serde_json::json!({
                            "success": false,
                            "error": "Session cannot be resumed (status or no steps)"
                        }))
                    }
                } else {
                    Json(serde_json::json!({ "success": false, "error": "Session not found" }))
                }
            } else {
                Json(serde_json::json!({ "success": false, "error": "Store not initialized" }))
            }
        }
        Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nl_automation::{IntentType, Plan, PlanStep, StepType};
    use serde_json::json;
    use serial_test::serial;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    fn reset_memory_tables() {
        crate::db::clear_request_memory_for_tests();
        crate::db::clear_execution_memory_for_tests();
        crate::db::clear_memory_admin_events_for_tests();
        crate::db::clear_launch_ops_events_for_tests();
        crate::db::clear_nl_runs_for_tests();
    }

    fn test_plan(intent: IntentType) -> Plan {
        Plan {
            plan_id: format!("plan-{}", uuid::Uuid::new_v4()),
            intent,
            slots: HashMap::new(),
            steps: vec![PlanStep {
                step_id: "extract-1".to_string(),
                step_type: StepType::Extract,
                description: "Extract final result".to_string(),
                data: json!({}),
            }],
        }
    }

    fn test_plan_with_descriptions(intent: IntentType, descriptions: &[&str]) -> Plan {
        Plan {
            plan_id: format!("plan-{}", uuid::Uuid::new_v4()),
            intent,
            slots: HashMap::new(),
            steps: descriptions
                .iter()
                .enumerate()
                .map(|(idx, desc)| PlanStep {
                    step_id: format!("step-{}", idx + 1),
                    step_type: StepType::Extract,
                    description: (*desc).to_string(),
                    data: json!({}),
                })
                .collect(),
        }
    }

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

    #[test]
    #[serial]
    fn request_memory_signature_cache_reuses_calendar_today_for_safe_paraphrase() {
        crate::db::init().ok();
        reset_memory_tables();
        let seed_prefix = format!("allvia-cache-calendar-today-{}", uuid::Uuid::new_v4());
        let intent = json!({
            "command": "calendar_today",
            "params": {},
            "confidence": 0.92
        });

        crate::db::upsert_request_memory(
            &format!("{} 오늘 일정 보여줘", seed_prefix),
            Some(&intent),
            Some("📅 오늘 일정이 없습니다."),
            "intent_only",
            "unit.test",
            0.92,
        )
        .expect("seed request memory");

        let cached =
            load_cached_request_intent(None, &format!("{} 오늘 캘린더 알려줘", seed_prefix))
                .expect("signature cache hit");
        assert_eq!(cached.0["command"].as_str(), Some("calendar_today"));
    }

    #[test]
    #[serial]
    fn request_memory_signature_cache_keeps_time_scope_distinct() {
        crate::db::init().ok();
        reset_memory_tables();
        let seed_prefix = format!("allvia-cache-calendar-week-{}", uuid::Uuid::new_v4());
        let intent = json!({
            "command": "calendar_week",
            "params": {},
            "confidence": 0.93
        });

        crate::db::upsert_request_memory(
            &format!("{} 이번 주 일정 보여줘", seed_prefix),
            Some(&intent),
            Some("📅 이번 주 일정이 없습니다."),
            "intent_only",
            "unit.test",
            0.93,
        )
        .expect("seed request memory");

        let cached = load_cached_request_intent(None, &format!("{} 오늘 일정 알려줘", seed_prefix));
        assert!(cached.is_none(), "today request must not reuse week intent");
    }

    #[test]
    fn request_memory_signature_cache_skips_parametric_commands() {
        crate::db::init().ok();
        let seed_prefix = format!("allvia-cache-build-{}", uuid::Uuid::new_v4());
        let intent = json!({
            "command": "build_workflow",
            "params": {
                "prompt": "회의록을 노션에 저장"
            },
            "confidence": 0.96
        });

        crate::db::upsert_request_memory(
            &format!("{} 회의록 노션 워크플로우 만들어줘", seed_prefix),
            Some(&intent),
            Some("📝 워크플로우 제안을 생성했습니다."),
            "intent_only",
            "unit.test",
            0.96,
        )
        .expect("seed request memory");

        let cached = load_cached_request_intent(
            None,
            &format!("{} 리포트 슬랙 워크플로우 생성해줘", seed_prefix),
        );
        assert!(
            cached.is_none(),
            "parametric commands must stay exact-match only"
        );
    }

    #[test]
    #[serial]
    fn request_memory_response_cache_reuses_fresh_calendar_response_for_paraphrase() {
        crate::db::init().ok();
        reset_memory_tables();
        let seed_prefix = format!("allvia-response-calendar-{}", uuid::Uuid::new_v4());
        let intent = json!({
            "command": "calendar_today",
            "params": {},
            "confidence": 0.95
        });

        crate::db::upsert_request_memory(
            &format!("{} 오늘 일정 보여줘", seed_prefix),
            Some(&intent),
            Some("📅 오늘 일정이 없습니다."),
            request_memory_response_mode("calendar_today"),
            "unit.test",
            0.95,
        )
        .expect("seed response cache");

        let cached = load_cached_request_response(
            None,
            &format!("{} 오늘 캘린더 알려줘", seed_prefix),
            "calendar_today",
        );
        assert_eq!(cached.as_deref(), Some("📅 오늘 일정이 없습니다."));
    }

    #[test]
    #[serial]
    fn request_memory_response_cache_bypasses_recent_equivalent_chat_repeat() {
        crate::db::init().ok();
        reset_memory_tables();
        let seed_prefix = format!("allvia-response-repeat-{}", uuid::Uuid::new_v4());
        let intent = json!({
            "command": "calendar_today",
            "params": {},
            "confidence": 0.95,
            "source": "deterministic"
        });

        crate::db::upsert_request_memory(
            &format!("{} 오늘 일정 보여줘", seed_prefix),
            Some(&intent),
            Some("📅 오늘 일정이 없습니다."),
            request_memory_response_mode("calendar_today"),
            "api.chat.deterministic",
            0.95,
        )
        .expect("seed recent chat response");

        let cached = load_cached_request_response(
            None,
            &format!("{} 오늘 캘린더 알려줘", seed_prefix),
            "calendar_today",
        );
        assert!(
            cached.is_none(),
            "recent equivalent chat request should bypass response cache"
        );
    }

    #[test]
    #[serial]
    fn request_memory_response_cache_bypasses_when_user_requests_fresh_data() {
        crate::db::init().ok();
        reset_memory_tables();
        let seed_prefix = format!("allvia-response-refresh-{}", uuid::Uuid::new_v4());
        let intent = json!({
            "command": "calendar_today",
            "params": {},
            "confidence": 0.95
        });

        crate::db::upsert_request_memory(
            &format!("{} 오늘 일정 보여줘", seed_prefix),
            Some(&intent),
            Some("📅 오늘 일정이 없습니다."),
            request_memory_response_mode("calendar_today"),
            "unit.test",
            0.95,
        )
        .expect("seed response cache");

        let cached = load_cached_request_response(
            None,
            &format!("{} 지금 오늘 일정 새로고침해서 보여줘", seed_prefix),
            "calendar_today",
        );
        assert!(
            cached.is_none(),
            "freshness request must bypass response cache"
        );
    }

    #[test]
    #[serial]
    fn request_memory_response_cache_respects_zero_ttl() {
        crate::db::init().ok();
        reset_memory_tables();
        let seed_prefix = format!("allvia-response-ttl-zero-{}", uuid::Uuid::new_v4());
        let intent = json!({
            "command": "gmail_list",
            "params": {},
            "confidence": 0.95
        });

        crate::db::upsert_request_memory(
            &format!("{} 최근 이메일 5개 보여줘", seed_prefix),
            Some(&intent),
            Some("📭 새 메일이 없습니다."),
            request_memory_response_mode("gmail_list"),
            "unit.test",
            0.95,
        )
        .expect("seed response cache");

        std::env::set_var("ALLVIA_RESPONSE_CACHE_TTL_GMAIL_LIST", "0");
        let cached = load_cached_request_response(
            None,
            &format!("{} 최근 이메일 5개 알려줘", seed_prefix),
            "gmail_list",
        );
        std::env::remove_var("ALLVIA_RESPONSE_CACHE_TTL_GMAIL_LIST");

        assert!(cached.is_none(), "zero ttl must disable response reuse");
    }

    #[test]
    #[serial]
    fn request_memory_response_cache_reuses_help_local_exact_match() {
        crate::db::init().ok();
        reset_memory_tables();
        let request = format!("allvia-help-cache-{}", uuid::Uuid::new_v4());
        let response = "💡 바로 실행 가능한 명령".to_string();

        persist_chat_response_memory(
            None,
            &request,
            "help_local",
            &response,
            "api.chat.local",
            1.0,
        );

        let cached = load_cached_request_response(None, &request, "help_local");
        assert_eq!(cached.as_deref(), Some("💡 바로 실행 가능한 명령"));
    }

    #[test]
    #[serial]
    fn request_memory_negative_feedback_suppresses_response_reuse() {
        crate::db::init().ok();
        reset_memory_tables();
        let request = format!("allvia-feedback-suppress-{}", uuid::Uuid::new_v4());
        let intent = json!({
            "command": "calendar_today",
            "params": {},
            "confidence": 0.95
        });
        let response = "📅 오늘 일정이 없습니다.";

        crate::db::upsert_request_memory(
            &request,
            Some(&intent),
            Some(response),
            request_memory_response_mode("calendar_today"),
            "unit.test",
            0.95,
        )
        .expect("seed request memory");
        crate::db::record_request_memory_feedback(&request, response, "negative")
            .expect("negative feedback");

        let cached = load_cached_request_response(None, &request, "calendar_today");
        assert!(
            cached.is_none(),
            "negative feedback must suppress response reuse"
        );
    }

    #[test]
    #[serial]
    fn request_memory_suppressed_record_does_not_reuse_response() {
        crate::db::init().ok();
        reset_memory_tables();
        let request = format!("allvia-suppress-request-{}", uuid::Uuid::new_v4());
        let response = format!("📅 suppressed-request-{}", uuid::Uuid::new_v4());
        let intent = json!({
            "command": "calendar_today",
            "params": {},
            "confidence": 0.95
        });
        crate::db::upsert_request_memory(
            &request,
            Some(&intent),
            Some(&response),
            request_memory_response_mode("calendar_today"),
            "unit.test",
            0.95,
        )
        .expect("seed request memory");
        crate::db::suppress_request_memory(&request, Some("ops suppress"))
            .expect("suppress request memory");

        let cached = load_cached_request_response(None, &request, "calendar_today");
        assert!(
            cached.is_none(),
            "suppressed request memory must not be reused"
        );
    }

    #[test]
    #[serial]
    fn request_memory_negative_feedback_suppresses_intent_reuse() {
        crate::db::init().ok();
        reset_memory_tables();
        let request = format!("allvia-intent-suppress-{}", uuid::Uuid::new_v4());
        let intent = json!({
            "command": "calendar_today",
            "params": {},
            "confidence": 0.95
        });
        let response = "📅 오늘 일정이 없습니다.";

        crate::db::upsert_request_memory(
            &request,
            Some(&intent),
            Some(response),
            "intent_only",
            "unit.test",
            0.95,
        )
        .expect("seed request memory");
        crate::db::record_request_memory_feedback(&request, response, "negative")
            .expect("negative feedback");

        let cached = load_cached_request_intent(None, &request);
        assert!(
            cached.is_none(),
            "negative feedback must suppress intent reuse"
        );
    }

    #[test]
    #[serial]
    fn execution_memory_reuses_gmail_default_count_across_paraphrase() {
        crate::db::init().ok();
        reset_memory_tables();
        let seed_prefix = format!("allvia-exec-gmail-{}", uuid::Uuid::new_v4());
        let explicit_intent = json!({
            "command": "gmail_list",
            "params": { "count": 5 },
            "confidence": 0.95
        });
        let implicit_intent = json!({
            "command": "gmail_list",
            "params": {},
            "confidence": 0.95
        });
        let response = format!("📧 execution-cache-{}", uuid::Uuid::new_v4());

        persist_execution_memory(
            None,
            &format!("{} 최근 이메일 5개 보여줘", seed_prefix),
            "gmail_list",
            &explicit_intent,
            &response,
            "unit.test",
        );

        let cached = load_cached_execution_response(
            None,
            &format!("{} 최근 이메일 보여줘", seed_prefix),
            "gmail_list",
            &implicit_intent,
        );
        assert_eq!(cached.as_deref(), Some(response.as_str()));
    }

    #[test]
    #[serial]
    fn execution_memory_bypasses_when_user_requests_fresh_data() {
        crate::db::init().ok();
        reset_memory_tables();
        let seed_prefix = format!("allvia-exec-refresh-{}", uuid::Uuid::new_v4());
        let intent = json!({
            "command": "gmail_list",
            "params": { "count": 5 },
            "confidence": 0.95
        });
        let response = format!("📧 execution-refresh-{}", uuid::Uuid::new_v4());

        persist_execution_memory(
            None,
            &format!("{} 최근 이메일 5개 보여줘", seed_prefix),
            "gmail_list",
            &intent,
            &response,
            "unit.test",
        );

        let cached = load_cached_execution_response(
            None,
            &format!("{} 지금 최근 이메일 5개 새로고침", seed_prefix),
            "gmail_list",
            &intent,
        );
        assert!(
            cached.is_none(),
            "freshness request must bypass execution cache"
        );
    }

    #[test]
    #[serial]
    fn execution_memory_bypasses_recent_chat_repeat() {
        crate::db::init().ok();
        reset_memory_tables();
        let request = format!(
            "allvia-exec-repeat-{} 최근 이메일 5개 보여줘",
            uuid::Uuid::new_v4()
        );
        let intent = json!({
            "command": "gmail_list",
            "params": { "count": 5 },
            "confidence": 0.95
        });
        let response = format!("📧 execution-repeat-{}", uuid::Uuid::new_v4());

        persist_execution_memory(
            None,
            &request,
            "gmail_list",
            &intent,
            &response,
            "api.chat.execution",
        );

        let cached = load_cached_execution_response(None, &request, "gmail_list", &intent);
        assert!(
            cached.is_none(),
            "recent chat repeat should bypass execution cache"
        );
    }

    #[test]
    #[serial]
    fn execution_memory_keeps_gmail_count_distinct() {
        crate::db::init().ok();
        reset_memory_tables();
        let seed_prefix = format!("allvia-exec-gmail-count-{}", uuid::Uuid::new_v4());
        let five_intent = json!({
            "command": "gmail_list",
            "params": { "count": 5 },
            "confidence": 0.95
        });
        let ten_intent = json!({
            "command": "gmail_list",
            "params": { "count": 10 },
            "confidence": 0.95
        });
        let response = format!("📧 execution-cache-five-{}", uuid::Uuid::new_v4());

        persist_execution_memory(
            None,
            &format!("{} 최근 이메일 5개 보여줘", seed_prefix),
            "gmail_list",
            &five_intent,
            &response,
            "unit.test",
        );

        let cached = load_cached_execution_response(
            None,
            &format!("{} 최근 이메일 10개 보여줘", seed_prefix),
            "gmail_list",
            &ten_intent,
        );
        assert!(
            cached.is_none(),
            "different counts must not share execution cache"
        );
    }

    #[test]
    #[serial]
    fn execution_memory_negative_feedback_suppresses_reuse() {
        crate::db::init().ok();
        reset_memory_tables();
        let request = format!(
            "allvia-exec-feedback-{} 최근 이메일 17개 보여줘",
            uuid::Uuid::new_v4()
        );
        let intent = json!({
            "command": "gmail_list",
            "params": { "count": 17 },
            "confidence": 0.95
        });
        let response = format!("📧 execution-negative-{}", uuid::Uuid::new_v4());

        persist_execution_memory(
            None,
            &request,
            "gmail_list",
            &intent,
            &response,
            "unit.test",
        );
        crate::db::record_execution_memory_feedback(
            "gmail_list",
            "count=17",
            &response,
            "negative",
        )
        .expect("execution negative feedback");
        let found = crate::db::get_execution_memory("gmail_list", "count=17")
            .expect("get execution memory")
            .expect("execution memory row");
        assert_eq!(found.negative_feedback_count, 1);

        let cached = load_cached_execution_response(None, &request, "gmail_list", &intent);
        assert!(
            cached.is_none(),
            "negative feedback must suppress execution reuse"
        );
    }

    #[test]
    #[serial]
    fn execution_memory_suppressed_record_does_not_reuse_response() {
        crate::db::init().ok();
        reset_memory_tables();
        let request = format!(
            "allvia-suppress-execution-{} 최근 이메일 5개 보여줘",
            uuid::Uuid::new_v4()
        );
        let intent = json!({
            "command": "gmail_list",
            "params": { "count": 5 },
            "confidence": 0.95
        });
        let response = format!("📧 suppressed-execution-{}", uuid::Uuid::new_v4());

        persist_execution_memory(
            None,
            &request,
            "gmail_list",
            &intent,
            &response,
            "unit.test",
        );
        crate::db::suppress_execution_memory("gmail_list", "count=5", Some("ops suppress"))
            .expect("suppress execution memory");

        let cached = load_cached_execution_response(None, &request, "gmail_list", &intent);
        assert!(
            cached.is_none(),
            "suppressed execution memory must not be reused"
        );
    }

    #[test]
    fn recommendation_visibility_keeps_manual_pending_and_caps_auto_pending() {
        let manual = crate::db::Recommendation {
            id: 1,
            status: "pending".to_string(),
            title: "Manual Workflow".to_string(),
            summary: "manual".to_string(),
            trigger: "manual".to_string(),
            actions: vec![],
            n8n_prompt: "manual".to_string(),
            confidence: 0.6,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![],
            pattern_id: None,
            last_error: None,
            snoozed_until: None,
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.1,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        };
        let auto = (0..6)
            .map(|idx| crate::db::Recommendation {
                id: idx + 10,
                status: "pending".to_string(),
                title: format!("Auto Workflow {}", idx),
                summary: "auto".to_string(),
                trigger: format!("pattern {}", idx),
                actions: vec![],
                n8n_prompt: "auto".to_string(),
                confidence: 0.84 + (idx as f64 * 0.01),
                workflow_id: None,
                workflow_json: None,
                evidence: vec![
                    format!("Frequency: Found {} occurrences", 6 + idx),
                    "Span: 4 distinct day(s)".to_string(),
                    "policy.reason=strong_work_signals=slack,calendar".to_string(),
                    "policy.reason=pattern.context_score=0.82".to_string(),
                    "policy.reason=pattern.work_context=strong".to_string(),
                ],
                pattern_id: Some(format!("pattern-{}", idx)),
                last_error: None,
                snoozed_until: None,
                category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
                business_score: 0.75 + (idx as f64 * 0.02),
                feedback_status: None,
                feedback_note: None,
                feedback_count: 0,
                last_feedback_at: None,
            })
            .collect::<Vec<_>>();

        let mut input = vec![manual];
        input.extend(auto);
        let visible = limit_visible_recommendations(input, Some("work"), &[]);

        assert_eq!(visible.len(), 6);
        assert_eq!(visible[0].title, "Manual Workflow");
        assert!(visible.iter().any(|rec| rec.title == "Auto Workflow 5"));
        assert!(!visible.iter().any(|rec| rec.title == "Auto Workflow 0"));
    }

    #[test]
    fn recommendation_visibility_prefers_aligned_delivery_channel() {
        let history = vec![
            crate::db::Recommendation {
                id: 1,
                status: "approved".to_string(),
                title: "Notion Notes".to_string(),
                summary: "Save notes to Notion".to_string(),
                trigger: "meeting".to_string(),
                actions: vec![],
                n8n_prompt: "Create a workflow that writes meeting notes to Notion.".to_string(),
                confidence: 0.8,
                workflow_id: Some("wf-1".to_string()),
                workflow_json: None,
                evidence: vec![],
                pattern_id: Some("hist-notion".to_string()),
                last_error: None,
                snoozed_until: None,
                category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
                business_score: 0.8,
                feedback_status: None,
                feedback_note: None,
                feedback_count: 0,
                last_feedback_at: None,
            },
            crate::db::Recommendation {
                id: 2,
                status: "rejected".to_string(),
                title: "Telegram Digest".to_string(),
                summary: "Send digest to Telegram".to_string(),
                trigger: "digest".to_string(),
                actions: vec![],
                n8n_prompt: "Create a workflow that sends updates to Telegram.".to_string(),
                confidence: 0.8,
                workflow_id: None,
                workflow_json: None,
                evidence: vec![],
                pattern_id: Some("hist-telegram".to_string()),
                last_error: None,
                snoozed_until: None,
                category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
                business_score: 0.8,
                feedback_status: None,
                feedback_note: None,
                feedback_count: 0,
                last_feedback_at: None,
            },
        ];
        let pending = vec![
            crate::db::Recommendation {
                id: 10,
                status: "pending".to_string(),
                title: "Telegram Status Digest".to_string(),
                summary: "Send digest to Telegram".to_string(),
                trigger: "digest".to_string(),
                actions: vec![],
                n8n_prompt: "Create a workflow that sends a work digest to Telegram.".to_string(),
                confidence: 0.91,
                workflow_id: None,
                workflow_json: None,
                evidence: vec![
                    "Frequency: Found 6 occurrences".to_string(),
                    "Span: 4 distinct day(s)".to_string(),
                    "policy.reason=strong_work_signals=slack,gmail".to_string(),
                    "policy.reason=pattern.context_score=0.80".to_string(),
                    "policy.reason=pattern.work_context=strong".to_string(),
                ],
                pattern_id: Some("pending-telegram".to_string()),
                last_error: None,
                snoozed_until: None,
                category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
                business_score: 0.90,
                feedback_status: None,
                feedback_note: None,
                feedback_count: 0,
                last_feedback_at: None,
            },
            crate::db::Recommendation {
                id: 11,
                status: "pending".to_string(),
                title: "Notion Meeting Brief".to_string(),
                summary: "Save briefing to Notion".to_string(),
                trigger: "meeting".to_string(),
                actions: vec![],
                n8n_prompt: "Create a workflow that stores meeting briefs in Notion.".to_string(),
                confidence: 0.88,
                workflow_id: None,
                workflow_json: None,
                evidence: vec![
                    "Frequency: Found 5 occurrences".to_string(),
                    "Span: 4 distinct day(s)".to_string(),
                    "policy.reason=strong_work_signals=notion,calendar".to_string(),
                    "policy.reason=pattern.context_score=0.81".to_string(),
                    "policy.reason=pattern.work_context=strong".to_string(),
                ],
                pattern_id: Some("pending-notion".to_string()),
                last_error: None,
                snoozed_until: None,
                category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
                business_score: 0.86,
                feedback_status: None,
                feedback_note: None,
                feedback_count: 0,
                last_feedback_at: None,
            },
        ];

        let visible = limit_visible_recommendations(pending, Some("work"), &history);

        assert_eq!(visible[0].title, "Notion Meeting Brief");
        assert_eq!(visible[1].title, "Telegram Status Digest");
    }

    #[test]
    fn recommendation_visibility_prefers_approval_ready_auto_items() {
        let ready = crate::db::Recommendation {
            id: 10,
            status: "pending".to_string(),
            title: "Ready Workflow".to_string(),
            summary: "Detected 6 repeats across 4 distinct day(s).".to_string(),
            trigger: "ready".to_string(),
            actions: vec![],
            n8n_prompt: "Create a workflow that uses Slack and Calendar.".to_string(),
            confidence: 0.90,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![
                "Frequency: Found 6 occurrences".to_string(),
                "Span: 4 distinct day(s)".to_string(),
                "policy.reason=strong_work_signals=slack,calendar".to_string(),
                "policy.reason=pattern.context_score=0.82".to_string(),
                "policy.reason=pattern.work_context=strong".to_string(),
            ],
            pattern_id: Some("ready-pattern".to_string()),
            last_error: None,
            snoozed_until: None,
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.82,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        };
        let weak = crate::db::Recommendation {
            id: 11,
            status: "pending".to_string(),
            title: "Weak Workflow".to_string(),
            summary: "Detected 3 repeats across 1 distinct day(s).".to_string(),
            trigger: "weak".to_string(),
            actions: vec![],
            n8n_prompt: "Create a workflow that uses VSCode.".to_string(),
            confidence: 0.91,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![
                "Frequency: Found 3 occurrences".to_string(),
                "Span: 1 distinct day(s)".to_string(),
                "policy.reason=support_work_signals=vscode,terminal".to_string(),
            ],
            pattern_id: Some("weak-pattern".to_string()),
            last_error: None,
            snoozed_until: None,
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.90,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        };

        let visible = limit_visible_recommendations(vec![weak, ready], Some("work"), &[]);

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].title, "Ready Workflow");
    }

    #[test]
    fn recommendation_visibility_hides_non_ready_auto_pending_items() {
        let weak = crate::db::Recommendation {
            id: 11,
            status: "pending".to_string(),
            title: "Weak Workflow".to_string(),
            summary: "Detected 3 repeats across 1 distinct day(s).".to_string(),
            trigger: "weak".to_string(),
            actions: vec![],
            n8n_prompt: "Create a workflow that uses VSCode.".to_string(),
            confidence: 0.91,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![
                "Frequency: Found 3 occurrences".to_string(),
                "Span: 1 distinct day(s)".to_string(),
                "policy.reason=support_work_signals=vscode,terminal".to_string(),
            ],
            pattern_id: Some("weak-pattern".to_string()),
            last_error: None,
            snoozed_until: None,
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.90,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        };

        let visible = limit_visible_recommendations(vec![weak], Some("work"), &[]);

        assert!(visible.is_empty());
    }

    #[test]
    fn recommendation_effective_status_maps_snoozed_pending_to_later() {
        let snoozed = crate::db::Recommendation {
            id: 1,
            status: "pending".to_string(),
            title: "Snoozed Workflow".to_string(),
            summary: "manual".to_string(),
            trigger: "manual".to_string(),
            actions: vec![],
            n8n_prompt: "manual".to_string(),
            confidence: 0.6,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![],
            pattern_id: None,
            last_error: None,
            snoozed_until: Some((chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339()),
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.1,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        };

        assert_eq!(recommendation_effective_status(&snoozed), "later");
    }

    #[test]
    fn recommendation_feedback_classifier_is_conservative() {
        assert_eq!(
            classify_recommendation_feedback("텔레그램 말고 노션으로 바꿔줘"),
            "refine"
        );
        assert_eq!(
            classify_recommendation_feedback("이건 필요없고 중복이라 싫어"),
            "negative"
        );
        assert_eq!(
            classify_recommendation_feedback("좋아요. 이 방향이 맞아요"),
            "positive"
        );
    }

    #[test]
    fn parse_u32_param_uses_bounds_and_string_values() {
        assert_eq!(parse_u32_param(Some(&json!("12")), 5, 1, 20), 12);
        assert_eq!(parse_u32_param(Some(&json!(99)), 5, 1, 20), 20);
        assert_eq!(parse_u32_param(Some(&json!(0)), 5, 1, 20), 1);
        assert_eq!(parse_u32_param(None, 5, 1, 20), 5);
    }

    #[test]
    fn feedback_tie_suppresses_response_reuse() {
        assert!(feedback_allows_response_reuse(1, 0));
        assert!(feedback_allows_response_reuse(2, 1));
        assert!(!feedback_allows_response_reuse(1, 1));
        assert!(!feedback_allows_response_reuse(0, 1));
    }

    #[test]
    fn request_prefers_fresh_data_is_precise() {
        assert!(request_prefers_fresh_data(
            "지금 최근 이메일 5개 새로고침",
            "gmail_list"
        ));
        assert!(!request_prefers_fresh_data(
            "새로운 이메일 5개 보여줘",
            "gmail_list"
        ));
        assert!(!request_prefers_fresh_data(
            "최근 이메일 5개 보여줘",
            "build_workflow"
        ));
    }

    #[tokio::test]
    #[serial]
    async fn handle_chat_auto_routes_ai_digest_on_web_without_llm() {
        crate::db::init().ok();
        reset_memory_tables();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr = listener.local_addr().expect("listener addr");
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(|| async {
                axum::Json(json!({
                    "status": "ok",
                    "notion_url": "https://www.notion.so/test-ai-digest",
                    "top_headlines_text": "1. 헤드라인 A\n2. 헤드라인 B"
                }))
            }),
        );
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        std::env::remove_var("ALLVIA_AI_DIGEST_AUTO_ROUTE_CHANNELS");
        std::env::set_var(
            "STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL",
            format!("http://{addr}/"),
        );

        let state = AppState {
            llm_client: None,
            current_goal: Arc::new(Mutex::new(None)),
        };
        let request = ChatRequest {
            message: "AI 뉴스 5개 요약해서 노션에 정리해줘".to_string(),
            channel: Some("web".to_string()),
            chat_type: Some("direct".to_string()),
            sender: Some("web-test".to_string()),
            mentioned: None,
        };

        let Json(response) = handle_chat(State(state), Json(request)).await;

        assert_eq!(response.command.as_deref(), Some("ai_digest_program"));
        assert_eq!(
            response
                .route_meta
                .as_ref()
                .map(|meta| meta.route_kind.as_str()),
            Some("ai_digest_auto")
        );
        assert_eq!(
            response.route_meta.as_ref().map(|meta| meta.ai_digest_used),
            Some(true)
        );
        assert!(response
            .response
            .contains("노션 링크: https://www.notion.so/test-ai-digest"));
        assert!(response.response.contains("헤드라인 A"));

        std::env::remove_var("STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL");
    }

    #[tokio::test]
    #[serial]
    async fn handle_chat_recomputes_local_prefix_commands_after_strip() {
        crate::db::init().ok();
        reset_memory_tables();

        let state = AppState {
            llm_client: None,
            current_goal: Arc::new(Mutex::new(None)),
        };
        let request = ChatRequest {
            message: "/local help".to_string(),
            channel: Some("web".to_string()),
            chat_type: Some("direct".to_string()),
            sender: Some("prefix-user".to_string()),
            mentioned: None,
        };

        let Json(response) = handle_chat(State(state), Json(request)).await;

        assert_eq!(response.command.as_deref(), Some("help_local"));
        assert!(response
            .response
            .contains("뉴스 5개 요약해서 노션에 정리해줘"));
    }

    #[tokio::test]
    #[serial]
    async fn approve_recommendation_blocks_auto_items_without_readiness() {
        crate::db::init().ok();
        crate::db::clear_recommendations_for_tests();
        crate::db::clear_recommendation_review_events_for_tests();

        let proposal = crate::recommendation::AutomationProposal {
            title: "Weak Approval Candidate".to_string(),
            summary: "Detected 3 repeats across 1 distinct day(s).".to_string(),
            trigger: "weak-approval".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.74,
            n8n_prompt: "Create a workflow for weak approval candidate.".to_string(),
            evidence: vec![
                "Frequency: Found 3 occurrences".to_string(),
                "Span: 1 distinct day(s)".to_string(),
                "policy.reason=support_work_signals=vscode,terminal".to_string(),
            ],
            pattern_id: Some("weak-approval-pattern".to_string()),
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.61,
        };
        crate::db::insert_recommendation(&proposal).expect("insert recommendation");
        let rec_id = crate::db::get_recommendations_with_filter(Some("pending"))
            .expect("list recs")
            .into_iter()
            .find(|rec| rec.title == "Weak Approval Candidate")
            .map(|rec| rec.id)
            .expect("recommendation id");

        let state = AppState {
            llm_client: None,
            current_goal: Arc::new(Mutex::new(None)),
        };

        let result = approve_recommendation(
            State(state),
            axum::extract::Path(rec_id),
            Some(Json(RecommendationReviewActionRequest {
                actor: Some("test_dashboard".to_string()),
                note: Some("manual approve attempt".to_string()),
            })),
        )
        .await;

        let (status, Json(body)) = result.expect_err("approval should be blocked");
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(body["details"]
            .as_str()
            .unwrap_or_default()
            .contains("needs stronger business evidence"));

        let events = crate::db::list_recommendation_review_events(10).expect("review events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "approve");
        assert!(!events[0].ok);
        assert_eq!(events[0].actor.as_deref(), Some("test_dashboard"));
        assert_eq!(events[0].note.as_deref(), Some("manual approve attempt"));
        assert_eq!(events[0].status_after.as_deref(), Some("pending"));
    }

    #[tokio::test]
    #[serial]
    async fn later_recommendation_rejects_approved_items_and_logs_event() {
        crate::db::init().ok();
        crate::db::clear_recommendations_for_tests();
        crate::db::clear_recommendation_review_events_for_tests();

        let proposal = crate::recommendation::AutomationProposal {
            title: "Approved Later Candidate".to_string(),
            summary: "Detected 5 repeats across 4 distinct day(s).".to_string(),
            trigger: "approved-later".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.91,
            n8n_prompt: "Create a workflow.".to_string(),
            evidence: vec![
                "Frequency: Found 5 occurrences".to_string(),
                "Span: 4 distinct day(s)".to_string(),
                "policy.reason=strong_work_signals=slack,calendar".to_string(),
                "policy.reason=pattern.context_score=0.86".to_string(),
                "policy.reason=pattern.work_context=strong".to_string(),
            ],
            pattern_id: Some("approved-later".to_string()),
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.83,
        };
        crate::db::insert_recommendation(&proposal).expect("insert recommendation");
        let rec_id = crate::db::get_recommendations_with_filter(Some("pending"))
            .expect("list recommendations")
            .into_iter()
            .find(|rec| rec.title == "Approved Later Candidate")
            .map(|rec| rec.id)
            .expect("recommendation id");
        crate::db::update_recommendation_review_status(rec_id, "approved")
            .expect("approve recommendation");

        let status = later_recommendation(
            axum::extract::Path(rec_id),
            Some(Json(RecommendationReviewActionRequest {
                actor: Some("test_workflows".to_string()),
                note: Some("should not snooze approved item".to_string()),
            })),
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT);
        let rec = crate::db::get_recommendation(rec_id)
            .expect("get recommendation")
            .expect("recommendation");
        assert_eq!(rec.status, "approved");
        assert!(rec.snoozed_until.is_none());

        let events = crate::db::list_recommendation_review_events(10).expect("review events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "later");
        assert!(!events[0].ok);
        assert_eq!(events[0].status_after.as_deref(), Some("approved"));
        assert_eq!(events[0].actor.as_deref(), Some("test_workflows"));
    }

    #[tokio::test]
    #[serial]
    async fn recommendation_metrics_hide_non_ready_auto_pending_items() {
        crate::db::init().ok();
        crate::db::clear_recommendations_for_tests();

        crate::db::insert_recommendation(&crate::recommendation::AutomationProposal {
            title: "Ready Metric Candidate".to_string(),
            summary: "Detected 6 repeats across 4 distinct day(s).".to_string(),
            trigger: "ready-metric".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.88,
            n8n_prompt: "Create a workflow.".to_string(),
            evidence: vec![
                "Frequency: Found 6 occurrences".to_string(),
                "Span: 4 distinct day(s)".to_string(),
                "policy.reason=strong_work_signals=slack,calendar".to_string(),
                "policy.reason=pattern.context_score=0.82".to_string(),
                "policy.reason=pattern.work_context=strong".to_string(),
            ],
            pattern_id: Some("ready-metric".to_string()),
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.82,
        })
        .expect("insert ready recommendation");
        crate::db::insert_recommendation(&crate::recommendation::AutomationProposal {
            title: "Weak Metric Candidate".to_string(),
            summary: "Detected 3 repeats across 1 distinct day(s).".to_string(),
            trigger: "weak-metric".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.74,
            n8n_prompt: "Create a workflow.".to_string(),
            evidence: vec![
                "Frequency: Found 3 occurrences".to_string(),
                "Span: 1 distinct day(s)".to_string(),
                "policy.reason=support_work_signals=vscode,terminal".to_string(),
            ],
            pattern_id: Some("weak-metric".to_string()),
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.61,
        })
        .expect("insert weak recommendation");

        let Json(metrics) = get_recommendation_metrics().await;

        assert_eq!(metrics.pending, 1);
    }

    #[tokio::test]
    #[serial]
    async fn handle_chat_uses_deterministic_workflow_intent_without_llm() {
        crate::db::init().ok();
        crate::db::clear_recommendations_for_tests();

        let state = AppState {
            llm_client: None,
            current_goal: Arc::new(Mutex::new(None)),
        };
        let request = ChatRequest {
            message: "노션에 회의록 정리하는 워크플로우 만들어줘".to_string(),
            channel: None,
            chat_type: None,
            sender: None,
            mentioned: None,
        };

        let Json(response) = handle_chat(State(state), Json(request)).await;

        assert_eq!(response.command.as_deref(), Some("build_workflow"));
        assert_eq!(
            response
                .route_meta
                .as_ref()
                .map(|meta| meta.route_kind.as_str()),
            Some("deterministic")
        );
        assert_eq!(
            response
                .route_meta
                .as_ref()
                .map(|meta| meta.deterministic_used),
            Some(true)
        );
        assert!(response.response.contains("워크플로우 제안을"));
        let stored = crate::db::get_request_memory("노션에 회의록 정리하는 워크플로우 만들어줘")
            .expect("request memory lookup")
            .expect("request memory row");
        assert_eq!(stored.source, "api.chat.deterministic");
    }

    #[tokio::test]
    #[serial]
    async fn handle_chat_scopes_local_memory_by_sender() {
        crate::db::init().ok();
        reset_memory_tables();

        let state = AppState {
            llm_client: None,
            current_goal: Arc::new(Mutex::new(None)),
        };

        let request_alice = ChatRequest {
            message: "도움말".to_string(),
            channel: Some("web".to_string()),
            chat_type: Some("direct".to_string()),
            sender: Some("alice".to_string()),
            mentioned: None,
        };
        let request_bob = ChatRequest {
            message: "도움말".to_string(),
            channel: Some("web".to_string()),
            chat_type: Some("direct".to_string()),
            sender: Some("bob".to_string()),
            mentioned: None,
        };

        let Json(response_alice) = handle_chat(State(state.clone()), Json(request_alice)).await;
        let Json(response_bob) = handle_chat(State(state), Json(request_bob)).await;

        assert_eq!(response_alice.command.as_deref(), Some("help_local"));
        assert_eq!(response_bob.command.as_deref(), Some("help_local"));

        let alice = crate::db::get_request_memory_scoped(
            Some("channel_web__type_direct__sender_alice"),
            "도움말",
        )
        .expect("alice scoped lookup")
        .expect("alice scoped row");
        let bob = crate::db::get_request_memory_scoped(
            Some("channel_web__type_direct__sender_bob"),
            "도움말",
        )
        .expect("bob scoped lookup")
        .expect("bob scoped row");

        assert_eq!(alice.memory_scope, "channel_web_type_direct_sender_alice");
        assert_eq!(bob.memory_scope, "channel_web_type_direct_sender_bob");
        assert!(crate::db::get_request_memory("도움말")
            .expect("global lookup")
            .is_none());
    }

    #[tokio::test]
    #[serial]
    async fn handle_chat_records_launch_ops_for_local_cache_hit() {
        crate::db::init().ok();
        reset_memory_tables();

        let state = AppState {
            llm_client: None,
            current_goal: Arc::new(Mutex::new(None)),
        };
        let request = ChatRequest {
            message: "도움말".to_string(),
            channel: Some("web".to_string()),
            chat_type: Some("direct".to_string()),
            sender: Some("launch-ops-user".to_string()),
            mentioned: None,
        };

        let Json(first) = handle_chat(State(state.clone()), Json(request.clone())).await;
        let Json(second) = handle_chat(State(state), Json(request)).await;

        assert_eq!(first.command.as_deref(), Some("help_local"));
        assert_eq!(second.command.as_deref(), Some("help_local"));

        let events = crate::db::list_launch_ops_events(4).expect("launch ops events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].route_kind, "request_memory");
        assert!(events[0].request_memory_hit);
        assert!(events[0].local_only);
        assert_eq!(events[1].route_kind, "local_command");
        assert_eq!(
            second
                .route_meta
                .as_ref()
                .map(|meta| meta.route_kind.as_str()),
            Some("request_memory")
        );
        assert_eq!(
            second
                .route_meta
                .as_ref()
                .map(|meta| meta.request_memory_hit),
            Some(true)
        );
    }

    #[tokio::test]
    #[serial]
    async fn memory_records_handler_returns_recent_admin_events() {
        crate::db::init().ok();
        reset_memory_tables();

        crate::db::upsert_request_memory(
            "오늘 일정 보여줘",
            Some(&serde_json::json!({
                "command": "calendar_today",
                "params": {},
                "confidence": 0.95
            })),
            Some("📅 일정 2건"),
            "ttl_response_signature",
            "unit.test",
            0.95,
        )
        .expect("seed request memory");

        let (status, Json(response)) =
            suppress_request_memory_handler(Json(RequestMemoryAdminActionRequest {
                request_text: "오늘 일정 보여줘".to_string(),
                memory_scope: None,
                reason: Some("launch cache issue".to_string()),
                actor: Some("settings_test".to_string()),
            }))
            .await;

        assert_eq!(status, StatusCode::OK);
        assert!(response.ok);

        let Json(records) = list_memory_records_handler(Query(MemoryRecordsQuery {
            limit: Some(10),
            include_suppressed: Some(true),
        }))
        .await;

        assert!(!records.recent_admin_events.is_empty());
        let event = &records.recent_admin_events[0];
        assert_eq!(event.kind, "request_memory");
        assert_eq!(event.action, "suppress");
        assert_eq!(event.actor.as_deref(), Some("settings_test"));
        assert_eq!(event.reason.as_deref(), Some("launch cache issue"));
        assert!(event.ok);
    }

    #[tokio::test]
    #[serial]
    async fn handle_chat_records_release_nl_run_for_chat_request_but_skips_local_help_noise() {
        crate::db::init().ok();
        crate::db::clear_recommendations_for_tests();
        reset_memory_tables();

        let state = AppState {
            llm_client: None,
            current_goal: Arc::new(Mutex::new(None)),
        };

        let help_request = ChatRequest {
            message: "도움말".to_string(),
            channel: Some("web".to_string()),
            chat_type: Some("direct".to_string()),
            sender: Some("nl-run-help".to_string()),
            mentioned: None,
        };
        let workflow_request = ChatRequest {
            message: "노션에 회의록 정리하는 워크플로우 만들어줘".to_string(),
            channel: Some("web".to_string()),
            chat_type: Some("direct".to_string()),
            sender: Some("nl-run-workflow".to_string()),
            mentioned: None,
        };

        let _ = handle_chat(State(state.clone()), Json(help_request)).await;
        let Json(response) = handle_chat(State(state), Json(workflow_request)).await;

        assert_eq!(response.command.as_deref(), Some("build_workflow"));

        let runs = crate::db::list_nl_runs(10).expect("list nl runs");
        assert_eq!(
            runs.len(),
            1,
            "local help should not create a release nl_run"
        );
        assert_eq!(runs[0].intent, "build_workflow");
        assert_eq!(runs[0].status, "approval_required");
        assert!(runs[0].prompt.contains("워크플로우 만들어줘"));
    }

    #[tokio::test]
    #[serial]
    async fn get_launch_ops_handler_reports_recent_metrics() {
        crate::db::init().ok();
        reset_memory_tables();

        let state = AppState {
            llm_client: None,
            current_goal: Arc::new(Mutex::new(None)),
        };
        let help_request = ChatRequest {
            message: "도움말".to_string(),
            channel: Some("web".to_string()),
            chat_type: Some("direct".to_string()),
            sender: Some("launch-ops-metrics".to_string()),
            mentioned: None,
        };
        let digest_request = ChatRequest {
            message: "AI 뉴스 5개 요약해서 노션에 정리해줘".to_string(),
            channel: Some("web".to_string()),
            chat_type: Some("direct".to_string()),
            sender: Some("launch-ops-metrics".to_string()),
            mentioned: None,
        };

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr = listener.local_addr().expect("listener addr");
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(|| async {
                axum::Json(json!({
                    "status": "ok",
                    "notion_url": "https://www.notion.so/test-launch-ops",
                    "top_headlines_text": "1. 헤드라인 A"
                }))
            }),
        );
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        std::env::remove_var("ALLVIA_AI_DIGEST_AUTO_ROUTE_CHANNELS");
        std::env::set_var(
            "STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL",
            format!("http://{addr}/"),
        );

        let _ = handle_chat(State(state.clone()), Json(help_request.clone())).await;
        let _ = handle_chat(State(state.clone()), Json(help_request)).await;
        let _ = handle_chat(State(state), Json(digest_request)).await;

        let Json(payload) = get_launch_ops_handler(Query(LaunchOpsQuery { limit: Some(50) })).await;

        assert_eq!(payload.chat_metrics.total_requests, 3);
        assert_eq!(payload.chat_metrics.request_memory_hits, 1);
        assert_eq!(payload.chat_metrics.ai_digest_auto_routes, 1);
        assert_eq!(payload.recommendation_metrics.pending >= 0, true);
        assert_eq!(
            payload.recommendation_review_metrics.total_events >= 0,
            true
        );
        assert_eq!(payload.recent_events[0].route_kind, "ai_digest_auto");

        std::env::remove_var("STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL");
    }

    #[tokio::test]
    #[serial]
    async fn http_e2e_handlers_run_and_load_latest_report() {
        let workdir = tempfile::tempdir().expect("temp workdir");
        let workdir_str = workdir.path().to_string_lossy().to_string();
        std::env::set_var("ALLVIA_API_ALLOW_WORKDIR_OVERRIDE", "1");

        let Json(run_report) = run_http_e2e_handler(Json(HttpE2ERequest {
            workdir: Some(workdir_str.clone()),
        }))
        .await
        .expect("run http e2e");
        assert!(run_report.ok);
        assert_eq!(run_report.passed, run_report.total);

        let Json(latest) = get_latest_http_e2e_handler(Query(HttpE2ERequest {
            workdir: Some(workdir_str),
        }))
        .await
        .expect("latest http e2e");
        let latest = latest.expect("http e2e report");
        assert_eq!(latest.passed, run_report.passed);
        assert_eq!(latest.total, run_report.total);

        std::env::remove_var("ALLVIA_API_ALLOW_WORKDIR_OVERRIDE");
    }

    #[tokio::test]
    #[serial]
    async fn run_release_readiness_handler_rejects_client_baseline_override_by_default() {
        std::env::remove_var("ALLVIA_API_ALLOW_PATH_OVERRIDE");
        let result = run_release_readiness_handler(Json(ReleaseReadinessRequest {
            workdir: None,
            config_path: None,
            candidate_limit: None,
            snapshot_output_path: None,
            save_baseline: Some(true),
        }))
        .await;
        assert!(matches!(result, Err(StatusCode::BAD_REQUEST)));
    }

    #[tokio::test]
    #[serial]
    async fn set_release_baseline_handler_rejects_override_payload_by_default() {
        std::env::remove_var("ALLVIA_API_ALLOW_PATH_OVERRIDE");
        let result = set_release_baseline_handler(Json(release_gate::ReleaseBaselineRequest {
            workdir: Some("/tmp/override".to_string()),
            max_files: Some(10),
            consistency: None,
            semantic: None,
            performance: None,
            quality: None,
            perf_regression_pct: None,
            quality_drop: None,
            launch_error_rate_pct: None,
            launch_low_confidence_rate_pct: None,
            launch_cache_hit_rate_drop_pct: None,
            recommendation_approval_rate_min: None,
            launch_eval_config_path: None,
            launch_eval_report_path: None,
            refresh_launch_eval_candidates: Some(false),
            launch_eval_candidate_limit: Some(1),
            launch_eval_snapshot_output_path: Some("/tmp/override.json".to_string()),
        }))
        .await;
        assert!(matches!(result, Err(StatusCode::BAD_REQUEST)));
    }

    #[tokio::test]
    #[serial]
    async fn run_release_readiness_handler_rejects_client_path_overrides_by_default() {
        std::env::remove_var("ALLVIA_API_ALLOW_PATH_OVERRIDE");
        let result = run_release_readiness_handler(Json(ReleaseReadinessRequest {
            workdir: None,
            config_path: Some("/tmp/custom-launch-eval.yaml".to_string()),
            candidate_limit: None,
            snapshot_output_path: Some("/tmp/custom-snapshot.yaml".to_string()),
            save_baseline: Some(false),
        }))
        .await;
        assert!(matches!(result, Err(StatusCode::BAD_REQUEST)));
    }

    #[tokio::test]
    #[serial]
    async fn launch_eval_snapshot_handlers_reject_output_path_override_by_default() {
        std::env::remove_var("ALLVIA_API_ALLOW_PATH_OVERRIDE");

        let info_result =
            get_launch_eval_candidate_snapshot_info_handler(Query(LaunchEvalSnapshotInfoQuery {
                output_path: Some("/tmp/custom-launch-eval.generated.yaml".to_string()),
                workdir: None,
                provenance: Some("real".to_string()),
            }))
            .await;
        assert!(matches!(info_result, Err(StatusCode::BAD_REQUEST)));

        let write_result =
            write_launch_eval_candidate_snapshot_handler(Json(LaunchEvalSnapshotRequest {
                limit: Some(5),
                output_path: Some("/tmp/custom-launch-eval.generated.yaml".to_string()),
                workdir: None,
                provenance: Some("real".to_string()),
            }))
            .await;
        assert!(matches!(write_result, Err(StatusCode::BAD_REQUEST)));
    }

    #[test]
    #[serial]
    fn resolve_operational_workdir_ignores_client_override_by_default() {
        std::env::remove_var("ALLVIA_API_ALLOW_WORKDIR_OVERRIDE");
        let resolved = resolve_operational_workdir(Some("/tmp/should-not-be-used"));
        assert_eq!(resolved, default_server_workdir());
    }

    #[test]
    #[serial]
    fn resolve_operational_workdir_allows_override_with_env_flag() {
        std::env::set_var("ALLVIA_API_ALLOW_WORKDIR_OVERRIDE", "1");
        let resolved = resolve_operational_workdir(Some("/tmp/allvia-override"));
        assert_eq!(resolved, PathBuf::from("/tmp/allvia-override"));
        std::env::remove_var("ALLVIA_API_ALLOW_WORKDIR_OVERRIDE");
    }

    #[test]
    #[serial]
    fn resolve_operational_file_override_requires_explicit_env_flag() {
        std::env::remove_var("ALLVIA_API_ALLOW_PATH_OVERRIDE");
        std::env::remove_var("ALLVIA_API_ALLOW_WORKDIR_OVERRIDE");
        assert!(resolve_operational_file_override(Some("/tmp/blocked")).is_none());

        std::env::set_var("ALLVIA_API_ALLOW_PATH_OVERRIDE", "1");
        assert_eq!(
            resolve_operational_file_override(Some("/tmp/allowed")),
            Some(PathBuf::from("/tmp/allowed"))
        );
        std::env::remove_var("ALLVIA_API_ALLOW_PATH_OVERRIDE");
    }

    #[test]
    fn default_release_baseline_request_uses_fixed_server_defaults() {
        let request = default_release_baseline_request();
        assert_eq!(request.refresh_launch_eval_candidates, Some(true));
        assert_eq!(request.launch_eval_candidate_limit, Some(20));
        assert!(request.workdir.is_none());
        assert!(request.launch_eval_config_path.is_none());
        assert!(request.launch_eval_snapshot_output_path.is_none());
    }

    #[test]
    fn business_contract_rejects_missing_mail_notion_telegram_evidence() {
        let plan = test_plan_with_descriptions(
            IntentType::GenericTask,
            &[
                "Mail에서 qed4950@gmail.com으로 결과를 보내세요.",
                "Notion에 요약을 작성하세요.",
                "텔레그램으로 전송하세요.",
            ],
        );
        let logs = vec![
            "Step 1: Open app".to_string(),
            "Summary: requested integrations done".to_string(),
        ];

        let (ok, detail) = evaluate_business_evidence(&plan, &logs);
        assert!(!ok);
        assert!(detail.contains("contract_missing_mail_send_confirmation"));
        assert!(detail.contains("contract_missing_mail_recipient=qed4950@gmail.com"));
        assert!(detail.contains("contract_missing_notion_write_confirmation"));
        assert!(detail.contains("contract_missing_telegram_send_confirmation"));
    }

    #[test]
    fn business_contract_accepts_mail_notion_telegram_evidence_markers() {
        let plan = test_plan_with_descriptions(
            IntentType::GenericTask,
            &[
                "Mail에서 qed4950@gmail.com으로 결과를 보내세요.",
                "Notion에 요약을 작성하세요.",
                "텔레그램으로 전송하세요.",
            ],
        );
        let logs = vec![
            "MAIL_SEND_PROOF|status=sent_confirmed|recipient=qed4950@gmail.com|subject=Digest"
                .to_string(),
            "Notion: https://www.notion.so/abcd1234".to_string(),
            "telegram: sent".to_string(),
            "Summary: integrations completed with artifacts".to_string(),
        ];

        let (ok, detail) = evaluate_business_evidence(&plan, &logs);
        assert!(ok, "expected ok but got {}", detail);
    }

    #[test]
    fn business_contract_accepts_structured_evidence_schema() {
        let plan = test_plan_with_descriptions(
            IntentType::GenericTask,
            &[
                "Mail에서 qed4950@gmail.com으로 결과를 보내세요.",
                "Notion에 요약을 작성하세요.",
                "텔레그램으로 전송하세요.",
                "TextEdit에 저장하세요.",
            ],
        );
        let logs = vec![
            "EVIDENCE|target=mail|event=send|status=sent_confirmed|recipient=qed4950@gmail.com|subject=Digest|body_len=120".to_string(),
            "EVIDENCE|target=notion|event=write|status=confirmed|page_id=abcd1234".to_string(),
            "EVIDENCE|target=telegram|event=send|status=sent|message_id=123".to_string(),
            "EVIDENCE|target=textedit|event=save|status=confirmed|doc_id=doc-1".to_string(),
            "Summary: integrations completed with structured evidence".to_string(),
        ];

        let (ok, detail) = evaluate_business_evidence(&plan, &logs);
        assert!(ok, "expected ok but got {}", detail);
    }

    #[test]
    fn business_contract_rejects_structured_mail_without_recipient() {
        let plan = test_plan_with_descriptions(
            IntentType::GenericTask,
            &["Mail에서 qed4950@gmail.com으로 결과를 보내세요."],
        );
        let logs = vec![
            "EVIDENCE|target=mail|event=send|status=sent_confirmed|recipient=|subject=Digest|body_len=120".to_string(),
            "Summary: mail send done".to_string(),
        ];

        let (ok, detail) = evaluate_business_evidence(&plan, &logs);
        assert!(!ok);
        assert!(detail.contains("contract_missing_mail_recipient_evidence"));
    }

    #[test]
    fn business_contract_rejects_structured_notion_telegram_without_ids() {
        let plan = test_plan_with_descriptions(
            IntentType::GenericTask,
            &["Notion에 요약을 작성하고 텔레그램으로 전송하세요."],
        );
        let logs = vec![
            "EVIDENCE|target=notion|event=write|status=confirmed|page_id=".to_string(),
            "EVIDENCE|target=telegram|event=send|status=sent|message_id=".to_string(),
            "Summary: integrations completed".to_string(),
        ];

        let (ok, detail) = evaluate_business_evidence(&plan, &logs);
        assert!(!ok);
        assert!(detail.contains("contract_missing_notion_page_id"));
        assert!(detail.contains("contract_missing_telegram_message_id"));
    }

    #[test]
    fn business_contract_rejects_structured_notes_without_note_id() {
        let plan =
            test_plan_with_descriptions(IntentType::GenericTask, &["Notes에 TODO를 작성하세요."]);
        let logs = vec![
            "EVIDENCE|target=notes|event=write|status=confirmed|note_id=|body_len=20".to_string(),
            "Summary: notes write completed".to_string(),
        ];
        let (ok, detail) = evaluate_business_evidence(&plan, &logs);
        assert!(!ok);
        assert!(detail.contains("contract_missing_notes_note_id"));
    }

    #[test]
    fn business_contract_rejects_structured_textedit_without_doc_id() {
        let plan = test_plan_with_descriptions(
            IntentType::GenericTask,
            &["TextEdit에 결과를 작성하세요."],
        );
        let logs = vec![
            "EVIDENCE|target=textedit|event=write|status=confirmed|doc_id=|body_len=42".to_string(),
            "Summary: textedit write completed".to_string(),
        ];
        let (ok, detail) = evaluate_business_evidence(&plan, &logs);
        assert!(!ok);
        assert!(detail.contains("contract_missing_textedit_doc_id"));
    }

    #[test]
    fn business_contract_respects_explicit_semantic_assertions() {
        let plan = test_plan_with_descriptions(
            IntentType::GenericTask,
            &["semantic_assertions: [artifact.mail_sent_confirmed]"],
        );
        let logs = vec!["Summary: generic task completed".to_string()];
        let (ok, detail) = evaluate_business_evidence(&plan, &logs);
        assert!(!ok);
        assert!(detail.contains("contract_required_assertion_failed=artifact.mail_sent_confirmed"));
    }

    #[test]
    fn business_contract_rejects_mail_send_with_empty_body_len() {
        let plan = test_plan_with_descriptions(
            IntentType::GenericTask,
            &["Mail에서 qed4950@gmail.com으로 결과를 보내세요."],
        );
        let logs = vec![
            "MAIL_SEND_PROOF|status=sent_confirmed|recipient=qed4950@gmail.com|subject=Digest|body_len=0".to_string(),
            "Summary: mail send done".to_string(),
        ];

        let (ok, detail) = evaluate_business_evidence(&plan, &logs);
        assert!(!ok);
        assert!(detail.contains("contract_mail_body_empty"));
    }

    #[test]
    fn business_contract_rejects_textedit_save_missing() {
        let plan = test_plan_with_descriptions(
            IntentType::GenericTask,
            &["TextEdit에 결과를 작성하고 저장하세요."],
        );
        let logs = vec![
            "TEXTEDIT_WRITE_CONFIRMED|len=42".to_string(),
            "Summary: textedit write done".to_string(),
        ];

        let (ok, detail) = evaluate_business_evidence(&plan, &logs);
        assert!(!ok);
        assert!(detail.contains("contract_missing_textedit_save_confirmation"));
    }

    #[test]
    fn completion_score_is_high_on_clean_success() {
        let score = compute_completion_score("completed", true, true, true, true, true, 0, 0);
        assert!(score.score >= 90, "score={}", score.score);
        assert!(score.pass);
        assert_eq!(score.label, "Excellent");
    }

    #[test]
    fn completion_score_drops_on_failed_execution() {
        let score = compute_completion_score("error", true, false, false, false, false, 4, 3);
        assert!(score.score < 60, "score={}", score.score);
        assert!(!score.pass);
        assert_eq!(score.label, "Risky");
        assert!(score
            .reasons
            .iter()
            .any(|r| r.contains("final_status=error")));
    }

    #[test]
    fn parse_resume_token_accepts_valid_shape() {
        let parsed = parse_resume_token("resume:plan-123:2:user_approved:1700000000")
            .expect("resume token should parse");
        assert_eq!(parsed.plan_id, "plan-123");
        assert_eq!(parsed.step_index, 2);
        assert_eq!(parsed.reason, "user_approved");
    }

    #[test]
    fn parse_resume_token_rejects_invalid_prefix() {
        let err = parse_resume_token("token:plan-123:2:user_approved:1700000000")
            .expect_err("invalid prefix must fail");
        assert!(err.contains("prefix invalid"));
    }

    #[test]
    fn parse_resume_token_rejects_invalid_step() {
        let err = parse_resume_token("resume:plan-123:x:user_approved:1700000000")
            .expect_err("non-numeric step must fail");
        assert!(err.contains("step index invalid"));
    }

    #[test]
    fn parse_resume_token_rejects_empty_reason() {
        let err = parse_resume_token("resume:plan-123:2::1700000000")
            .expect_err("empty reason must fail");
        assert!(err.contains("reason is empty"));
    }
}
