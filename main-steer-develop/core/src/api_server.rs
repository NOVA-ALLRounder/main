use axum::{
    extract::{State, Query, Path},
    http::StatusCode,
    routing::{get, post},
    response::IntoResponse,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};

use crate::{consistency_check, db, llm_gateway, monitor, pattern_detector, feedback_collector, integrations, n8n_api, chat_sanitize, context_pruning, project_scanner, runtime_verification, quality_scorer, visual_verification, semantic_verification, performance_verification, judgment, release_gate, tool_result_guard, intent_router, slot_filler, plan_builder, execution_controller, verification_engine, approval_gate, nl_store, collector_bridge};
use sysinfo::System;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone)]
pub struct AppState {
    pub llm_client: Option<std::sync::Arc<dyn llm_gateway::LLMClient>>,
    pub current_goal: Arc<Mutex<Option<String>>>,
    pub runtime_control: Arc<Mutex<crate::runtime_mode::RuntimeControl>>,
}

// Request/Response types
#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub channel: Option<String>,
    pub chat_type: Option<String>,
    pub sender: Option<String>,
    pub mentioned: Option<bool>,
}

#[derive(Serialize)]
pub struct VersionResponse {
    pub core_version: String,
    pub build_profile: String,
    pub git_sha: Option<String>,
}

#[derive(Serialize)]
pub struct RuntimeModeResponse {
    pub mode: String,
    pub emergency_stop: bool,
    pub allow_automation: bool,
}

#[derive(Serialize)]
pub struct PreflightResponse {
    pub ok: bool,
    pub api_port: u16,
    pub api_reachable: bool,
    pub env_present: bool,
    pub steer_home_writable: bool,
    pub release_dir_writable: bool,
    pub gmail_credentials_set: bool,
    pub notion_ready: bool,
    pub operation_mode: String,
    pub emergency_stop: bool,
    pub allow_automation: bool,
    pub notes: Vec<String>,
}

#[derive(Deserialize)]
pub struct SetRuntimeModeRequest {
    pub mode: String,
}

#[derive(Deserialize)]
pub struct EmergencyStopRequest {
    pub enabled: bool,
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
}

#[derive(Serialize)]
pub struct AgentExecuteResponse {
    pub status: String,
    pub logs: Vec<String>,
    pub approval: Option<crate::nl_automation::ApprovalContext>,
    #[serde(default)]
    pub manual_steps: Vec<String>,
    pub resume_from: Option<usize>,
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
pub struct ChatResponse {
    pub response: String,
    pub command: Option<String>,
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
    pub evidence: Vec<String>, // [NEW] Explainability field
    pub last_error: Option<String>,
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
    let api_key = std::env::var("STEER_API_KEY").unwrap_or_default();
    
    // If no key configured, allow all (Localhost Dev Mode)
    if api_key.is_empty() {
        return Ok(next.run(req).await);
    }

    // Check Header
    let auth_header = req.headers().get("Authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.replace("Bearer ", ""));
        
    match auth_header {
        Some(key) if key == api_key => Ok(next.run(req).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Start the HTTP API server for desktop GUI
pub async fn start_api_server(
    llm_client: Option<std::sync::Arc<dyn llm_gateway::LLMClient>>,
) -> anyhow::Result<()> {
    let state = AppState {
        llm_client,
        current_goal: Arc::new(Mutex::new(None)),
        runtime_control: Arc::new(Mutex::new(crate::runtime_mode::RuntimeControl::from_env())),
    };
    
    // NOTE: Tauri WebView origin can vary by runtime/build. Use permissive local CORS to avoid false failures.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/api/health", get(health_check))
        .route("/api/version", get(get_version))
        // Open endpoints (Status, Logs)
        .route("/api/status", get(get_system_status))
        .route("/api/logs", get(get_recent_logs))
        .route("/api/system/health", get(get_system_health))
        .route("/api/system/preflight", get(get_system_preflight))
        .route("/api/system/mode", get(get_runtime_mode).post(set_runtime_mode))
        .route("/api/system/emergency-stop", post(set_emergency_stop))
        // Protected Endpoints (Chat, Execute, Plan, Verify)
        .route("/events", post(ingest_events))
        .route("/api/chat", post(handle_chat))
        .route("/api/recommendations", get(list_recommendations))
        .route("/api/recommendations/:id/approve", post(approve_recommendation))
        .route("/api/recommendations/:id/reject", post(reject_recommendation))
        .route("/api/recommendations/:id/later", post(later_recommendation))
        .route("/api/recommendations/:id/restore", post(restore_recommendation))
        .route("/api/exec-approvals", get(list_exec_approvals))
        .route("/api/exec-approvals/:id/approve", post(approve_exec_approval))
        .route("/api/exec-approvals/:id/reject", post(reject_exec_approval))
        .route("/api/exec-allowlist", get(list_exec_allowlist).post(add_exec_allowlist))
        .route("/api/exec-allowlist/:id", axum::routing::delete(remove_exec_allowlist))
        .route("/api/exec-results", get(list_exec_results))
        .route("/api/project/scan", get(scan_project_handler))
        .route("/api/verify/runtime", post(run_runtime_verification_handler))
        .route("/api/verify/visual", post(run_visual_verification_handler))
        .route("/api/verify/semantic", post(run_semantic_verification_handler))
        .route("/api/verify/performance", post(run_performance_verification_handler))
        .route("/api/verify/consistency", post(run_consistency_verification_handler))
        .route("/api/verify/runs", get(list_verification_runs))
        .route("/api/judgment", post(run_judgment_handler))
        .route("/api/release/baseline", post(set_release_baseline_handler))
        .route("/api/release/gate", post(run_release_gate_handler))
        .route("/api/exec-results/guard", post(run_exec_results_guard_handler))
        .route("/api/quality/score", post(score_quality_handler))
        .route("/api/quality/latest", get(latest_quality_handler))
        .route("/api/patterns/analyze", post(analyze_patterns))
        .route("/api/quality", get(get_quality_metrics))
        .route("/api/recommendations/metrics", get(get_recommendation_metrics))
        .route("/api/routines", get(list_routines).post(create_routine_handler))
        .route("/api/routines/:id", axum::routing::patch(toggle_routine_handler))
        .route("/api/routine-runs", get(list_routine_runs))
        .route("/api/agent/intent", post(agent_intent_handler))
        .route("/api/agent/plan", post(agent_plan_handler))
        .route("/api/agent/execute", post(agent_execute_handler))
        .route("/api/agent/verify", post(agent_verify_handler))
        .route("/api/agent/approve", post(agent_approve_handler))
        .route("/api/agent/nl-runs", get(list_nl_runs_handler))
        .route("/api/agent/nl-metrics", get(nl_run_metrics_handler))
        .route(
            "/api/agent/approval-policies",
            get(list_nl_approval_policies).post(set_nl_approval_policy),
        )
        .route(
            "/api/agent/approval-policies/:key",
            axum::routing::delete(remove_nl_approval_policy),
        )
        .route("/api/agent/goal", post(execute_goal_handler))
        .route("/api/agent/goal/current", get(get_current_goal))
        .route("/api/agent/feedback", post(handle_feedback))
        .route("/api/context/selection", get(get_selection_context))
        // Session Management (Clawdbot-ported)
        .route("/api/sessions", get(list_sessions_handler))
        .route("/api/sessions/:id", get(get_session_handler).delete(delete_session_handler))
        .route("/api/sessions/:id/resume", post(resume_session_handler))
        .layer(axum::middleware::from_fn(auth_middleware)) // Apply Auth Middleware
        .layer(cors)
        .with_state(state);

    let port = std::env::var("STEER_API_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(5680);
    println!("???Desktop API server running on http://localhost:{}", port);
    
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to bind port {}: {}", port, e))?;
        
    axum::serve(listener, app).await.map_err(|e| anyhow::anyhow!("Server error: {}", e))?;
    Ok(())
}

async fn root_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "online",
        "service": "Steer OS Core API",
        "version": "v0.1.0",
        "ui_url": "http://localhost:5174",
        "docs": "/api/health"
    }))
}

async fn health_check() -> &'static str {
    "ok"
}

async fn get_version() -> Json<VersionResponse> {
    let build_profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    let git_sha = std::env::var("STEER_GIT_SHA").ok().and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });

    Json(VersionResponse {
        core_version: env!("CARGO_PKG_VERSION").to_string(),
        build_profile: build_profile.to_string(),
        git_sha,
    })
}

async fn get_runtime_mode(
    State(state): State<AppState>,
) -> Json<RuntimeModeResponse> {
    let ctl = state
        .runtime_control
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| crate::runtime_mode::RuntimeControl::from_env());

    Json(RuntimeModeResponse {
        mode: ctl.mode.as_str().to_string(),
        emergency_stop: ctl.emergency_stop,
        allow_automation: ctl.allow_automation(),
    })
}

async fn set_runtime_mode(
    State(state): State<AppState>,
    Json(payload): Json<SetRuntimeModeRequest>,
) -> Json<RuntimeModeResponse> {
    let mut ctl = state
        .runtime_control
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| crate::runtime_mode::RuntimeControl::from_env());
    ctl.mode = crate::runtime_mode::OperationMode::from_str(&payload.mode);
    if let Ok(mut guard) = state.runtime_control.lock() {
        *guard = ctl.clone();
    }

    Json(RuntimeModeResponse {
        mode: ctl.mode.as_str().to_string(),
        emergency_stop: ctl.emergency_stop,
        allow_automation: ctl.allow_automation(),
    })
}

async fn set_emergency_stop(
    State(state): State<AppState>,
    Json(payload): Json<EmergencyStopRequest>,
) -> Json<RuntimeModeResponse> {
    let mut ctl = state
        .runtime_control
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| crate::runtime_mode::RuntimeControl::from_env());
    ctl.emergency_stop = payload.enabled;
    if let Ok(mut guard) = state.runtime_control.lock() {
        *guard = ctl.clone();
    }

    Json(RuntimeModeResponse {
        mode: ctl.mode.as_str().to_string(),
        emergency_stop: ctl.emergency_stop,
        allow_automation: ctl.allow_automation(),
    })
}

fn list_files_human(path: &std::path::Path, limit: usize) -> String {
    let mut entries: Vec<String> = match std::fs::read_dir(path) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| {
                let ty = e.file_type().ok();
                let name = e.file_name().to_string_lossy().to_string();
                if ty.map(|t| t.is_dir()).unwrap_or(false) {
                    format!("[D] {}", name)
                } else {
                    format!("[F] {}", name)
                }
            })
            .collect(),
        Err(e) => return format!("Failed to read folder '{}': {}", path.display(), e),
    };

    entries.sort();
    let total = entries.len();
    let shown = entries.into_iter().take(limit).collect::<Vec<_>>();
    let mut out = format!("Files in {} (showing {}/{}):\n", path.display(), shown.len(), total);
    for item in shown {
        out.push_str(&format!("- {}\n", item));
    }
    out
}

fn open_url_windows(url: &str) -> Result<(), String> {
    let status = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("Start-Process \"{}\"", url.replace('"', "")),
        ])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("Start-Process failed".to_string())
    }
}

fn open_path_windows(path: &std::path::Path) -> Result<(), String> {
    let p = path.to_string_lossy().replace('"', "");
    let status = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("Start-Process \"{}\"", p),
        ])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("Start-Process failed".to_string())
    }
}

fn is_valid_notion_database_id(value: &str) -> bool {
    let trimmed = value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches('{')
        .trim_matches('}')
        .trim();
    let lowered = trimmed.to_lowercase();
    if trimmed.is_empty() || lowered.contains("your_database_id_here") {
        return false;
    }
    let normalized = trimmed.replace('-', "");
    normalized.len() == 32 && normalized.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_screen_summary_request(lower: &str) -> bool {
    let screen = lower.contains("screen")
        || lower.contains("current screen")
        || lower.contains("window")
        || lower.contains("\u{D654}\u{BA74}")
        || lower.contains("\u{CC3D}");
    let summarize = lower.contains("summary")
        || lower.contains("summarize")
        || lower.contains("\u{C694}\u{C57D}")
        || lower.contains("\u{C815}\u{B9AC}");
    screen && summarize
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScreenSummaryTarget {
    Notepad,
    Clipboard,
    File,
    Notion,
    Word,
}

fn parse_screen_summary_target(lower: &str) -> ScreenSummaryTarget {
    if lower.contains("clipboard") || lower.contains("\u{D074}\u{B9BD}\u{BCF4}\u{B4DC}") {
        return ScreenSummaryTarget::Clipboard;
    }
    if lower.contains("file")
        || lower.contains("save")
        || lower.contains("\u{D30C}\u{C77C}")
        || lower.contains("\u{C800}\u{C7A5}")
    {
        return ScreenSummaryTarget::File;
    }
    if lower.contains("notion") || lower.contains("\u{B178}\u{C158}") {
        return ScreenSummaryTarget::Notion;
    }
    if lower.contains("word") || lower.contains("docx") || lower.contains("\u{C6CC}\u{B4DC}") {
        return ScreenSummaryTarget::Word;
    }
    ScreenSummaryTarget::Notepad
}

async fn summarize_current_screen_text(
    state: &AppState,
    user_message: &str,
) -> Result<String, String> {
    let (b64, _scale) = crate::visual_driver::VisualDriver::capture_screen()
        .map_err(|e| format!("Screen capture failed: {}", e))?;

    let prompt = format!(
        "You are a desktop assistant. Read the current screen and summarize it in Korean.\n\
User request: {}\n\
Output requirements:\n\
- 5~10 bullet points maximum\n\
- Focus on actionable items, deadlines, errors, and next steps\n\
- Plain text only (no markdown code fences)\n\
- If the screen is unclear, state what is unclear and what to open next.",
        user_message
    );

    let provider = std::env::var("SCREEN_SUMMARY_PROVIDER")
        .unwrap_or_else(|_| {
            if std::env::var("GLM_OCR_API_KEY").is_ok() || std::env::var("GLM_API_KEY").is_ok() {
                "glm".to_string()
            } else {
                "openai".to_string()
            }
        })
        .to_lowercase();

    let raw = if provider == "glm" || provider == "glm_ocr" {
        summarize_current_screen_text_glm(&prompt, &b64)
            .await
            .map_err(|e| format!("Vision summary failed: {}", e))?
    } else {
        let Some(brain) = &state.llm_client else {
            return Err("LLM client unavailable".to_string());
        };
        brain
            .analyze_screen(&prompt, &b64)
            .await
            .map_err(|e| format!("Vision summary failed: {}", e))?
    };

    let cleaned = raw
        .replace("```", "")
        .trim()
        .to_string();
    if cleaned.is_empty() {
        return Err("Vision summary returned empty text".to_string());
    }
    Ok(cleaned)
}

fn extract_json_text_field(v: &serde_json::Value) -> Option<String> {
    let pointers = [
        "/result/markdown",
        "/result/text",
        "/result/content",
        "/data/markdown",
        "/data/text",
        "/data/content",
        "/output/markdown",
        "/output/text",
        "/output/content",
        "/text",
        "/content",
        "/choices/0/message/content",
    ];
    for p in pointers {
        if let Some(node) = v.pointer(p) {
            if let Some(s) = node.as_str() {
                let t = s.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
            if let Some(arr) = node.as_array() {
                let mut combined = String::new();
                for item in arr {
                    if let Some(s) = item.as_str() {
                        if !s.trim().is_empty() {
                            if !combined.is_empty() {
                                combined.push('\n');
                            }
                            combined.push_str(s.trim());
                        }
                        continue;
                    }
                    if let Some(s) = item.get("text").and_then(|x| x.as_str()) {
                        if !s.trim().is_empty() {
                            if !combined.is_empty() {
                                combined.push('\n');
                            }
                            combined.push_str(s.trim());
                        }
                    }
                }
                if !combined.trim().is_empty() {
                    return Some(combined.trim().to_string());
                }
            }
        }
    }
    None
}

fn collect_layout_text(node: &serde_json::Value, out: &mut Vec<String>) {
    match node {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                if let Some(s) = v.as_str() {
                    let key = k.to_lowercase();
                    let t = s.trim();
                    if t.is_empty() {
                        continue;
                    }
                    if t.starts_with("data:image/") || t.len() > 2000 {
                        continue;
                    }
                    let preferred = matches!(
                        key.as_str(),
                        "text"
                            | "content"
                            | "ocr_text"
                            | "recognized_text"
                            | "value"
                            | "markdown"
                            | "caption"
                            | "title"
                            | "line"
                            | "lines"
                            | "word"
                            | "words"
                    );
                    let looks_sentence =
                        t.contains(' ') || t.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c));
                    if preferred || looks_sentence {
                        if !out.iter().any(|x| x == t) {
                            out.push(t.to_string());
                        }
                    }
                } else {
                    collect_layout_text(v, out);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_layout_text(v, out);
            }
        }
        _ => {}
    }
}

fn extract_layout_parsing_text(v: &serde_json::Value) -> Option<String> {
    let candidate_nodes = [
        "/layout_details",
        "/result/layout_details",
        "/data/layout_details",
        "/result",
        "/data",
    ];
    let mut lines = Vec::new();
    for p in candidate_nodes {
        if let Some(node) = v.pointer(p) {
            collect_layout_text(node, &mut lines);
        }
    }
    if lines.is_empty() {
        return None;
    }
    let text = lines
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .take(200)
        .collect::<Vec<_>>()
        .join("\n");
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

async fn summarize_current_screen_text_glm(prompt: &str, image_b64: &str) -> Result<String, String> {
    let api_key = std::env::var("GLM_OCR_API_KEY")
        .or_else(|_| std::env::var("GLM_API_KEY"))
        .map_err(|_| "GLM_OCR_API_KEY (or GLM_API_KEY) not set".to_string())?;

    let layout_url = std::env::var("GLM_OCR_BASE_URL")
        .unwrap_or_else(|_| "https://open.bigmodel.cn/api/paas/v4/layout_parsing".to_string());
    let layout_model = std::env::var("GLM_OCR_MODEL").unwrap_or_else(|_| "glm-ocr".to_string());
    let data_url = format!("data:image/jpeg;base64,{}", image_b64);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("GLM client init failed: {}", e))?;

    let layout_body = json!({
        "model": layout_model,
        "file": data_url,
        "prompt": prompt
    });

    let layout_resp = client
        .post(&layout_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&layout_body)
        .send()
        .await
        .map_err(|e| format!("GLM OCR request failed: {}", e))?;

    let layout_status = layout_resp.status();
    let layout_text = layout_resp
        .text()
        .await
        .map_err(|e| format!("GLM OCR read failed: {}", e))?;

    if layout_status.is_success() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&layout_text) {
            if let Some(out) = extract_json_text_field(&v) {
                return Ok(out);
            }
            if let Some(out) = extract_layout_parsing_text(&v) {
                return Ok(out);
            }
        }
    }

    let chat_url = std::env::var("GLM_VISION_CHAT_URL")
        .unwrap_or_else(|_| "https://open.bigmodel.cn/api/paas/v4/chat/completions".to_string());
    let chat_model = std::env::var("GLM_VISION_MODEL")
        .ok()
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| "glm-4.5".to_string());
    let chat_body = json!({
        "model": chat_model,
        "messages": [
            {
                "role": "user",
                "content": format!(
                    "{}\n\nOCR raw response (truncated):\n{}",
                    prompt,
                    layout_text.chars().take(12000).collect::<String>()
                )
            }
        ],
        "max_tokens": 500
    });
    let chat_resp = client
        .post(&chat_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&chat_body)
        .send()
        .await
        .map_err(|e| format!("GLM vision fallback request failed: {}", e))?;
    let chat_status = chat_resp.status();
    let chat_text = chat_resp
        .text()
        .await
        .map_err(|e| format!("GLM vision fallback read failed: {}", e))?;

    if chat_status.is_success() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&chat_text) {
            if let Some(out) = extract_json_text_field(&v) {
                return Ok(out);
            }
        }
    }

    Err(format!(
        "GLM OCR failed (status {}): {} | GLM vision fallback failed (status {}): {}",
        layout_status.as_u16(),
        layout_text.chars().take(240).collect::<String>(),
        chat_status.as_u16(),
        chat_text.chars().take(240).collect::<String>()
    ))
}

async fn deliver_screen_summary(
    state: &AppState,
    user_message: &str,
) -> Result<(String, String), String> {
    let summary = summarize_current_screen_text(state, user_message).await?;
    let lower = user_message.to_lowercase();
    let target = parse_screen_summary_target(&lower);

    let final_text = format!(
        "[Screen Summary - {}]\n\n{}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        summary
    );

    match target {
        ScreenSummaryTarget::Clipboard => {
            crate::tool_chaining::CrossAppBridge::copy_to_clipboard(&final_text)
                .map_err(|e| format!("Failed to copy summary to clipboard: {}", e))?;
            Ok(("clipboard".to_string(), final_text))
        }
        ScreenSummaryTarget::File => {
            let base = std::env::var("STEER_HOME")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
            let out_dir = base.join("artifacts").join("screen-summaries");
            std::fs::create_dir_all(&out_dir)
                .map_err(|e| format!("Failed to create output directory: {}", e))?;
            let out_path = out_dir.join(format!(
                "screen-summary-{}.txt",
                chrono::Local::now().format("%Y%m%d-%H%M%S")
            ));
            std::fs::write(&out_path, &final_text)
                .map_err(|e| format!("Failed to write summary file: {}", e))?;
            let _ = open_path_windows(&out_path);
            Ok((format!("file:{}", out_path.display()), final_text))
        }
        ScreenSummaryTarget::Notion => {
            let db = std::env::var("NOTION_DATABASE_ID")
                .map_err(|_| "NOTION_DATABASE_ID missing".to_string())?;
            if !is_valid_notion_database_id(&db) {
                return Err("Invalid NOTION_DATABASE_ID format".to_string());
            }
            let client = integrations::notion::NotionClient::from_env()
                .map_err(|e| format!("Notion client init failed: {}", e))?;
            let title = format!("Screen Summary {}", chrono::Local::now().format("%Y-%m-%d %H:%M"));
            let body: String = final_text.chars().take(1800).collect();
            let page_id = client
                .create_page(&db, &title, &body)
                .await
                .map_err(|e| format!("Notion create failed: {}", e))?;
            Ok((format!("notion:{}", page_id), final_text))
        }
        ScreenSummaryTarget::Word => {
            crate::tool_chaining::CrossAppBridge::copy_to_clipboard(&final_text)
                .map_err(|e| format!("Failed to stage clipboard for Word: {}", e))?;
            crate::windows::actions::launch_app("winword")
                .map_err(|e| format!("Failed to launch Word: {}", e))?;
            tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;
            crate::windows::actions::type_text(&final_text)
                .map_err(|e| format!("Failed to type into Word: {}", e))?;
            Ok(("word".to_string(), final_text))
        }
        ScreenSummaryTarget::Notepad => {
            crate::windows::actions::launch_app("notepad")
                .map_err(|e| format!("Failed to launch Notepad: {}", e))?;
            tokio::time::sleep(tokio::time::Duration::from_millis(700)).await;
            crate::windows::actions::type_text(&final_text)
                .map_err(|e| format!("Failed to type into Notepad: {}", e))?;
            Ok(("notepad".to_string(), final_text))
        }
    }
}

fn free_chat_system_prompt() -> &'static str {
    "You are Steer, a practical desktop copilot.
- Default to natural conversation in the user's language.
- Give concise, useful answers.
- If a request implies automation, explain what you can do now and what command or phrase to run next.
- If blocked by runtime mode or missing credentials, explain the exact missing piece briefly.
- Do not claim actions were executed unless explicit execution result was provided."
}

async fn generate_free_chat_reply(state: &AppState, message: &str) -> Result<String, String> {
    let Some(brain) = &state.llm_client else {
        return Err("LLM client unavailable".to_string());
    };

    let history_limit = context_pruning::history_fetch_limit().min(16);
    let history = db::get_recent_chat_history(history_limit).unwrap_or_default();

    let mut messages: Vec<serde_json::Value> = Vec::with_capacity(history.len() + 2);
    messages.push(json!({
        "role": "system",
        "content": free_chat_system_prompt()
    }));

    for h in history {
        if h.content.trim().is_empty() {
            continue;
        }
        let role = match h.role.as_str() {
            "user" => "user",
            "assistant" => "assistant",
            _ => continue,
        };
        messages.push(json!({
            "role": role,
            "content": h.content
        }));
    }

    messages.push(json!({
        "role": "user",
        "content": message
    }));

    match brain.chat_completion(messages).await {
        Ok(s) => Ok(s.trim().to_string()),
        Err(e) => {
            let err_text = e.to_string();
            let lower = err_text.to_lowercase();
            if lower.contains("rate limit") || lower.contains("rate_limit_exceeded") {
                // Retry once with minimal context to reduce token pressure.
                let minimal = vec![
                    json!({
                        "role": "system",
                        "content": "You are a concise assistant. Reply in the user's language in 2-4 short sentences."
                    }),
                    json!({
                        "role": "user",
                        "content": message
                    }),
                ];
                return brain
                    .chat_completion(minimal)
                    .await
                    .map(|s| s.trim().to_string())
                    .map_err(|e2| e2.to_string());
            }
            Err(err_text)
        }
    }
}

fn write_email_ops_artifacts(
    summary: &str,
    tasks: &[(String, String)],
) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    use std::io::Write;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();

    let base = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join("artifacts")
        .join("email_ops")
        .join(format!("{}", ts));
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;

    let summary_path = base.join("summary.md");
    let mut summary_file = std::fs::File::create(&summary_path).map_err(|e| e.to_string())?;
    summary_file
        .write_all(summary.as_bytes())
        .map_err(|e| e.to_string())?;

    let tasks_path = base.join("tasks.csv");
    let mut tasks_file = std::fs::File::create(&tasks_path).map_err(|e| e.to_string())?;
    tasks_file
        .write_all(b"priority,subject,from,action,owner,status\n")
        .map_err(|e| e.to_string())?;
    for (subject, from) in tasks {
        let line = format!(
            "P2,\"{}\",\"{}\",\"review-and-reply\",\"me\",\"todo\"\n",
            subject.replace('"', "'"),
            from.replace('"', "'")
        );
        tasks_file
            .write_all(line.as_bytes())
            .map_err(|e| e.to_string())?;
    }

    Ok((summary_path, tasks_path))
}

async fn get_system_status() -> Json<SystemStatus> {
    let mut sys = System::new_all();
    sys.refresh_cpu(); // First refresh just gathers data
    std::thread::sleep(std::time::Duration::from_millis(200)); // Sleep minimal amount for CPU calculation
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

async fn get_recent_logs() -> Json<Vec<LogEntry>> {
    // Fetch routines from DB and convert last_run to LogEntry
    match crate::db::get_all_routines() {
        Ok(routines) => {
            let mut logs = Vec::new();
            for r in routines {
                if let Some(last) = r.last_run {
                    logs.push(LogEntry {
                        timestamp: last,
                        level: "INFO".to_string(),
                        message: format!("Routine Executed: {}", r.name),
                    });
                }
            }
            // Sort by timestamp desc
            logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            Json(logs)
        },
        Err(_) => Json(vec![]),
    }
}

async fn get_system_health(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let health = crate::dependency_check::SystemHealth::check_all();
    let ctl = state
        .runtime_control
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| crate::runtime_mode::RuntimeControl::from_env());
    let mut value = serde_json::to_value(health).unwrap_or_else(|_| json!({}));
    if let Some(obj) = value.as_object_mut() {
        obj.insert("operation_mode".to_string(), json!(ctl.mode.as_str()));
        obj.insert("emergency_stop".to_string(), json!(ctl.emergency_stop));
        obj.insert("allow_automation".to_string(), json!(ctl.allow_automation()));
    }
    Json(value)
}

fn check_writable_dir(path: &std::path::Path) -> bool {
    if std::fs::create_dir_all(path).is_err() {
        return false;
    }
    let probe = path.join(".steer_write_probe");
    match std::fs::write(&probe, b"ok") {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

async fn get_system_preflight(
    State(state): State<AppState>,
) -> Json<PreflightResponse> {
    let health = crate::dependency_check::SystemHealth::check_all();
    let ctl = state
        .runtime_control
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| crate::runtime_mode::RuntimeControl::from_env());

    let env_present = std::path::Path::new(".env").exists()
        || std::path::Path::new("core/.env").exists()
        || std::path::Path::new("web/src-tauri/target/release/.env").exists();

    let steer_home = std::env::var("STEER_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join(".steer"));
    let steer_home_writable = check_writable_dir(&steer_home);

    let release_dir = std::env::current_dir()
        .unwrap_or_default()
        .join("web")
        .join("src-tauri")
        .join("target")
        .join("release");
    let release_dir_writable = check_writable_dir(&release_dir);

    let mut notes = Vec::new();
    if !health.api_reachable {
        notes.push("API not reachable on expected port".to_string());
    }
    if !env_present {
        notes.push(".env file not found in known runtime paths".to_string());
    }
    if !steer_home_writable {
        notes.push("STEER_HOME is not writable".to_string());
    }
    if !release_dir_writable {
        notes.push("Release runtime directory is not writable".to_string());
    }
    if !health.gmail_credentials_set {
        notes.push("Gmail credentials missing (core/credentials.json)".to_string());
    }
    if !health.notion_ready {
        notes.push("Notion integration not ready (NOTION_API_KEY/NOTION_DATABASE_ID)".to_string());
    }

    let ok = health.api_reachable
        && env_present
        && steer_home_writable
        && release_dir_writable;

    Json(PreflightResponse {
        ok,
        api_port: health.api_port,
        api_reachable: health.api_reachable,
        env_present,
        steer_home_writable,
        release_dir_writable,
        gmail_credentials_set: health.gmail_credentials_set,
        notion_ready: health.notion_ready,
        operation_mode: ctl.mode.as_str().to_string(),
        emergency_stop: ctl.emergency_stop,
        allow_automation: ctl.allow_automation(),
        notes,
    })
}

async fn scan_project_handler(
    Query(query): Query<ProjectScanQuery>,
) -> Json<ProjectScanResponse> {
    let workdir = query
        .workdir
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().to_string_lossy().to_string());
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
        Some(json!({ "issues": result.issues, "backend_health": result.backend_health, "frontend_health": result.frontend_health })),
    );
    Json(result)
}

async fn run_visual_verification_handler(
    State(state): State<AppState>,
    Json(payload): Json<visual_verification::VisualVerifyRequest>,
) -> Json<visual_verification::VisualVerifyResult> {
    let Some(llm) = &state.llm_client else {
        return Json(visual_verification::VisualVerifyResult { ok: false, verdicts: vec![] });
    };
    match visual_verification::verify_screen(llm.as_ref(), payload).await {
        Ok(result) => {
            let summary = if result.ok { "Visual verification passed" } else { "Visual verification failed" };
            let details = json!({
                "verdicts": result.verdicts.iter().map(|v| json!({ "prompt": v.prompt, "ok": v.ok })).collect::<Vec<_>>()
            });
            log_verification_run("visual", result.ok, summary, Some(details));
            Json(result)
        }
        Err(_) => Json(visual_verification::VisualVerifyResult { ok: false, verdicts: vec![] }),
    }
}

async fn run_semantic_verification_handler(
    Json(payload): Json<SemanticVerifyRequest>,
) -> Json<semantic_verification::SemanticVerificationResult> {
    let workdir = payload
        .workdir
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().to_string_lossy().to_string());
    let max_files = payload.max_files.unwrap_or(200);
    let result = semantic_verification::semantic_consistency(std::path::Path::new(&workdir), max_files);
    let details = json!({
        "issues": result.issues.iter().take(10).map(|i| json!({"file": i.file, "severity": i.severity, "reason": i.reason})).collect::<Vec<_>>()
    });
    log_verification_run("semantic", result.ok, &result.reason, Some(details));
    Json(result)
}

async fn run_performance_verification_handler(
    Json(payload): Json<PerformanceVerifyRequest>,
) -> Json<performance_verification::PerformanceVerificationResult> {
    let workdir = payload
        .workdir
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().to_string_lossy().to_string());
    let max_files = payload.max_files.unwrap_or(300);
    let result = performance_verification::performance_baseline(std::path::Path::new(&workdir), max_files);
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
) -> Json<release_gate::ReleaseBaseline> {
    let baseline = release_gate::build_baseline(payload);
    release_gate::save_baseline(&baseline);
    Json(baseline)
}

async fn run_release_gate_handler(
    Json(payload): Json<release_gate::ReleaseGateRequest>,
) -> Json<release_gate::ReleaseGateResult> {
    let result = release_gate::run_release_gate(payload);
    let summary = if result.ok { "Release gate passed" } else { "Release gate failed" };
    let details = json!({
        "regressions": result.regressions.iter().take(10).cloned().collect::<Vec<_>>(),
        "warnings": result.warnings.iter().take(10).cloned().collect::<Vec<_>>()
    });
    log_verification_run("release_gate", result.ok, summary, Some(details));
    Json(result)
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
                Err(_) => quality_scorer::score_quality(Some(&runtime), payload.code_review.as_ref()),
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
async fn ingest_events(
    Json(payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    // 1. Normalize
    let events: Vec<crate::schema::EventEnvelope> = if let Some(arr) = payload.as_array() {
        arr.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect()
    } else if let Ok(single) = serde_json::from_value(payload.clone()) {
        vec![single]
    } else {
        return Json(serde_json::json!({ "error": "Invalid Event Format", "count": 0 }));
    };

    let count = events.len();
    let use_dcp = collector_bridge::is_dcp_mode();
    let mut success = 0;
    if use_dcp {
        match collector_bridge::send_events(&events).await {
            Ok(sent) => success = sent,
            Err(e) => {
                eprintln!("Ingest Error (DCP): {}", e);
            }
        }
    } else {
        // 2. Process & Insert
        // In a real high-perf scenario, we would push to a channel (EventBus).
        // For now, direct DB insert is fast enough for direct migration.
        
        // Initialize Privacy Guard (Salt should come from env in prod)
        let salt = std::env::var("PRIVACY_SALT").unwrap_or_else(|_| "default_salt".to_string());
        let guard = crate::privacy::PrivacyGuard::new(salt);
        
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
    }

    Json(serde_json::json!({
        "status": if use_dcp { "forwarded" } else { "queued" },
        "received": count,
        "processed": success
    }))
}

async fn handle_chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Json<ChatResponse> {
    let gate = crate::chat_gate::ChatGateConfig::from_env();
    let gate_ctx = crate::chat_gate::ChatGateContext {
        channel: req.channel.clone(),
        chat_type: req.chat_type.clone(),
        sender: req.sender.clone(),
        mentioned: req.mentioned,
    };
    if !gate.is_allowed(&gate_ctx) {
        return Json(ChatResponse {
            response: "Chat is currently blocked by gate policy for this channel/type.".to_string(),
            command: None,
        });
    }

    let sanitized = chat_sanitize::sanitize_chat_input(&req.message);
    if !sanitized.flags.is_empty() {
        eprintln!("Chat sanitize flags: {:?}", sanitized.flags);
    }
    let message = sanitized.text.trim().to_string();
    if message.is_empty() {
        return Json(ChatResponse {
            response: "Please enter a message.".to_string(),
            command: None,
        });
    }

    if let Err(e) = db::insert_chat_message("user", &message) {
        eprintln!("Failed to save user chat: {}", e);
    }

    let lower = message.to_lowercase();
    let ko_hello = "\u{C548}\u{B155}";
    let ko_hello_formal = "\u{C548}\u{B155}\u{D558}\u{C138}\u{C694}";
    let ko_calendar = "\u{C77C}\u{C815}";
    let ko_week = "\u{C774}\u{BC88}\u{C8FC}";
    let ko_mail = "\u{BA54}\u{C77C}";
    let ko_status = "\u{C0C1}\u{D0DC}";
    let ko_notepad = "\u{BA54}\u{BAA8}\u{C7A5}";
    let ko_calc = "\u{ACC4}\u{C0B0}\u{AE30}";
    let ko_explorer = "\u{D0D0}\u{C0C9}\u{AE30}";
    let ko_open = "\u{C5F4}\u{C5B4}";
    let ko_search = "\u{AC80}\u{C0C9}";
    let ko_mail_workflow = "\u{BA54}\u{C77C} \u{C6CC}\u{D06C}\u{D50C}\u{B85C}\u{C6B0}";
    let ko_mail_work = "\u{BA54}\u{C77C} \u{C5C5}\u{BB34}";
    let ko_execute = "\u{C2E4}\u{D589}";
    let ko_template = "\u{C0D8}\u{D50C}";

    if lower.contains(ko_hello)
        || lower.contains(ko_hello_formal)
        || lower == "hi"
        || lower == "hello"
        || lower == "hey"
    {
        return Json(ChatResponse {
            response: "Hello. Try: system status, today calendar, recent email, open notepad".to_string(),
            command: Some("smalltalk_greeting".to_string()),
        });
    }

    if message == "analyze_patterns" {
        let results = run_analysis_internal();
        let response = if results.is_empty() {
            "No repetitive pattern detected yet.".to_string()
        } else {
            format!("Detected {} pattern(s):\n{}", results.len(), results.join("\n"))
        };
        return Json(ChatResponse {
            response,
            command: Some("analyze_patterns".to_string()),
        });
    }

    if lower.contains(ko_calendar) || lower.contains("calendar") {
        let week_mode = lower.contains(ko_week) || lower.contains("week");
        let response = match integrations::calendar::CalendarClient::new().await {
            Ok(client) => {
                let events = if week_mode { client.list_week().await } else { client.list_today().await };
                match events {
                    Ok(items) if items.is_empty() => {
                        if week_mode { "No events this week.".to_string() } else { "No events today.".to_string() }
                    }
                    Ok(items) => {
                        let mut out = if week_mode {
                            String::from("This week's events:\n")
                        } else {
                            String::from("Today's events:\n")
                        };
                        for (_, summary, start_time) in items {
                            out.push_str(&format!("- {} ({})\n", summary, start_time));
                        }
                        out
                    }
                    Err(e) => format!("Calendar query failed: {}", e),
                }
            }
            Err(e) => format!("Calendar connection failed: {}", e),
        };
        return Json(ChatResponse {
            response,
            command: Some(if week_mode { "calendar_week" } else { "calendar_today" }.to_string()),
        });
    }

    if (lower.contains(ko_mail) || lower.contains("gmail") || lower.contains("email") || lower.contains("mail"))
        && !lower.contains("email workflow")
        && !lower.contains("mail workflow")
        && !lower.contains(ko_mail_workflow)
        && !lower.contains(ko_mail_work)
    {
        let response = match integrations::gmail::GmailClient::new().await {
            Ok(client) => match client.list_messages(5).await {
                Ok(items) if items.is_empty() => "No recent emails.".to_string(),
                Ok(items) => {
                    let mut out = String::from("Recent 5 emails:\n");
                    for (_, subject, from) in items {
                        out.push_str(&format!("- {} ({})\n", subject, from));
                    }
                    out
                }
                Err(e) => format!("Email query failed: {}", e),
            },
            Err(e) => format!("Gmail connection failed: {}", e),
        };
        return Json(ChatResponse {
            response,
            command: Some("gmail_list".to_string()),
        });
    }

    if lower.contains(ko_status) || lower == "status" || lower.contains("system status") {
        let mut rm = monitor::ResourceMonitor::new();
        return Json(ChatResponse {
            response: format!("System status: {}", rm.get_status()),
            command: Some("system_status".to_string()),
        });
    }

    if cfg!(target_os = "windows") {
        let runtime_ctl = state
            .runtime_control
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| crate::runtime_mode::RuntimeControl::from_env());
        let automation_allowed = runtime_ctl.allow_automation();
        let mode_name = runtime_ctl.mode.as_str().to_string();
        let emergency_stop = runtime_ctl.emergency_stop;

        let blocked_automation = |feature: &str| -> Json<ChatResponse> {
            Json(ChatResponse {
                response: format!(
                    "Automation '{}' blocked (mode={}, emergency_stop={}). Enable copilot/autopilot and clear emergency stop.",
                    feature, mode_name, emergency_stop
                ),
                command: Some("error_action_denied".to_string()),
            })
        };

        let run_app = |name: &str, alias: &str| -> Json<ChatResponse> {
            if !automation_allowed {
                return blocked_automation(alias);
            }
            match crate::windows::actions::launch_app(name) {
                Ok(_) => Json(ChatResponse {
                    response: format!("Launched {}.", alias),
                    command: Some("open_app".to_string()),
                }),
                Err(e) => Json(ChatResponse {
                    response: format!("Failed to launch {}: {}", alias, e),
                    command: None,
                }),
            }
        };

        if is_screen_summary_request(&lower) {
            if !automation_allowed {
                return blocked_automation("screen_summary");
            }
            return match deliver_screen_summary(&state, &message).await {
                Ok((target, final_text)) => {
                    let preview: String = final_text.chars().take(280).collect();
                    Json(ChatResponse {
                        response: format!(
                            "Screen summary delivered ({target}).\nPreview:\n{}{}",
                            preview,
                            if final_text.chars().count() > 280 { "\n..." } else { "" }
                        ),
                        command: Some("screen_summary_deliver".to_string()),
                    })
                }
                Err(e) => Json(ChatResponse {
                    response: format!("Screen summary automation failed: {}", e),
                    command: Some("error_exec_failed".to_string()),
                }),
            };
        }

        if lower.contains("email workflow")
            || lower.contains("mail workflow")
            || lower.contains(ko_mail_workflow)
            || lower.contains(ko_mail_work)
        {
            if !automation_allowed {
                return blocked_automation("email_ops_workflow");
            }

            let template_mode = lower.contains("template") || lower.contains(ko_template);
            let emails = if template_mode {
                vec![
                    ("Q1 budget review request".to_string(), "finance@company.com".to_string()),
                    ("Client escalation: delivery timeline".to_string(), "sales@company.com".to_string()),
                    ("Weekly project sync agenda".to_string(), "pm@company.com".to_string()),
                ]
            } else {
                match integrations::gmail::GmailClient::new().await {
                    Ok(client) => match client.list_messages(10).await {
                        Ok(items) => items
                            .into_iter()
                            .map(|(_, subject, from)| (subject, from))
                            .collect::<Vec<_>>(),
                        Err(e) => {
                            return Json(ChatResponse {
                                response: format!(
                                    "Email fetch failed: {}. You can still test with 'email workflow template run'.",
                                    e
                                ),
                                command: Some("error_missing_credentials".to_string()),
                            });
                        }
                    },
                    Err(e) => {
                        return Json(ChatResponse {
                            response: format!(
                                "Gmail connection failed: {}. You can still test with 'email workflow template run'.",
                                e
                            ),
                            command: Some("error_missing_credentials".to_string()),
                        });
                    }
                }
            };

            if emails.is_empty() {
                return Json(ChatResponse {
                    response: "No recent emails found. Workflow artifacts were not created.".to_string(),
                    command: Some("email_workflow_prepare".to_string()),
                });
            }

            let mut summary = String::from("# Email Ops Summary\n\n");
            summary.push_str("Generated automatically from recent inbox messages.\n\n");
            summary.push_str("## Top Messages\n");
            for (idx, (subject, from)) in emails.iter().take(10).enumerate() {
                summary.push_str(&format!("{}. {} ({})\n", idx + 1, subject, from));
            }
            summary.push_str("\n## Suggested Next Steps\n");
            summary.push_str("- Review high-priority items and draft replies.\n");
            summary.push_str("- Update task tracker from tasks.csv.\n");
            summary.push_str("- Push final report to Notion/Word if needed.\n");

            let execute_mode = lower.contains(" run")
                || lower.ends_with("run")
                || lower.contains(ko_execute);

            return match write_email_ops_artifacts(&summary, &emails) {
                Ok((summary_path, tasks_path)) => Json(ChatResponse {
                    response: {
                        if execute_mode {
                            let mut out = format!(
                                "Email workflow executed.\n- Summary: {}\n- Tasks CSV: {}",
                                summary_path.display(),
                                tasks_path.display()
                            );

                            let db_id = std::env::var("NOTION_DATABASE_ID").ok();
                            if let (Some(db), Ok(client)) = (db_id, integrations::notion::NotionClient::from_env()) {
                                if !is_valid_notion_database_id(&db) {
                                    out.push_str("\n- Notion skipped (invalid NOTION_DATABASE_ID format)");
                                } else {
                                    let title = format!("Email Ops {}", chrono::Utc::now().format("%Y-%m-%d %H:%M"));
                                    let page_content: String = summary.chars().take(1800).collect();
                                    match client.create_page(&db, &title, &page_content).await {
                                        Ok(page_id) => out.push_str(&format!("\n- Notion page created: {}", page_id)),
                                        Err(e) => out.push_str(&format!("\n- Notion create failed: {}", e)),
                                    }
                                }
                            } else {
                                out.push_str("\n- Notion skipped (NOTION_API_KEY/NOTION_DATABASE_ID missing)");
                            }

                            let _ = open_path_windows(&summary_path);
                            let _ = open_path_windows(&tasks_path);
                            if let Some(folder) = summary_path.parent() {
                                let _ = open_path_windows(folder);
                            }

                            out.push_str("\n- Local files opened. Continue in Excel/Word/Notion as needed.");
                            out
                        } else {
                            format!(
                                "Email workflow prepared.\n- Summary: {}\n- Tasks CSV: {}\nNext: run 'email workflow run' to execute open/export actions.",
                                summary_path.display(),
                                tasks_path.display()
                            )
                        }
                    },
                    command: Some(if execute_mode { "email_workflow_execute".to_string() } else { "email_workflow_prepare".to_string() }),
                }),
                Err(e) => Json(ChatResponse {
                    response: format!("Email workflow artifact write failed: {}", e),
                    command: Some("error_exec_failed".to_string()),
                }),
            };
        }

        if lower.contains("notepad") || lower.contains(ko_notepad) { return run_app("notepad", "Notepad"); }
        if lower.contains("calculator") || lower == "calc" || lower.contains(ko_calc) { return run_app("calc", "Calculator"); }
        if lower.contains("task manager") || lower == "taskmgr" { return run_app("taskmgr", "Task Manager"); }
        if lower.contains("windows settings") || lower == "settings" { return run_app("ms-settings:", "Windows Settings"); }
        if lower.contains("open chrome") || lower == "chrome" { return run_app("chrome", "Chrome"); }
        if lower.contains("open edge") || lower == "edge" { return run_app("msedge", "Edge"); }

        if lower.contains("explorer") || lower.contains("file explorer") || lower.contains(ko_explorer) {
            if !automation_allowed {
                return blocked_automation("explorer");
            }
            let status = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", "Start-Process explorer.exe"])
                .status();
            return match status {
                Ok(s) if s.success() => Json(ChatResponse {
                    response: "Opened File Explorer.".to_string(),
                    command: Some("open_explorer".to_string()),
                }),
                Ok(_) | Err(_) => Json(ChatResponse {
                    response: "Failed to open File Explorer.".to_string(),
                    command: None,
                }),
            };
        }

        if lower.contains("list files") {
            let target = if lower.contains("downloads") {
                dirs::download_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
            } else if lower.contains("desktop") {
                dirs::desktop_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
            } else {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            };
            return Json(ChatResponse {
                response: list_files_human(&target, 30),
                command: Some("list_files".to_string()),
            });
        }

        if let Some(rest) = lower.strip_prefix("open url ") {
            if !automation_allowed {
                return blocked_automation("open_url");
            }
            let url = rest.trim();
            if url.starts_with("http://") || url.starts_with("https://") {
                return match open_url_windows(url) {
                    Ok(_) => Json(ChatResponse {
                        response: format!("Opened URL: {}", url),
                        command: Some("open_url".to_string()),
                    }),
                    Err(e) => Json(ChatResponse {
                        response: format!("Failed to open URL: {}", e),
                        command: None,
                    }),
                };
            }
        }

        if let Some(rest) = lower.strip_prefix("search ") {
            if !automation_allowed {
                return blocked_automation("web_search");
            }
            let query = rest.trim().replace(' ', "+");
            if !query.is_empty() {
                let url = format!("https://www.google.com/search?q={}", query);
                return match open_url_windows(&url) {
                    Ok(_) => Json(ChatResponse {
                        response: format!("Searching web for '{}'.", rest.trim()),
                        command: Some("web_search".to_string()),
                    }),
                    Err(e) => Json(ChatResponse {
                        response: format!("Failed to open browser: {}", e),
                        command: None,
                    }),
                };
            }
        }
        if lower.contains(ko_search) && lower.contains(ko_open) {
            if !automation_allowed {
                return blocked_automation("web_search");
            }
            let query = lower.replace(ko_search, "").replace(ko_open, "").trim().replace(' ', "+");
            if !query.is_empty() {
                let url = format!("https://www.google.com/search?q={}", query);
                return match open_url_windows(&url) {
                    Ok(_) => Json(ChatResponse {
                        response: "Searching web.".to_string(),
                        command: Some("web_search".to_string()),
                    }),
                    Err(e) => Json(ChatResponse {
                        response: format!("Failed to open browser: {}", e),
                        command: None,
                    }),
                };
            }
        }

        let ko_run = "\u{C2E4}\u{D589} ";
        let raw_cmd = if let Some(c) = message.strip_prefix(ko_run) {
            Some(c.trim())
        } else if let Some(c) = message.strip_prefix("run ") {
            Some(c.trim())
        } else if let Some(c) = message.strip_prefix("cmd ") {
            Some(c.trim())
        } else {
            None
        };

        if let Some(cmd) = raw_cmd {
            if !automation_allowed {
                return blocked_automation("run_command");
            }

            let deny_patterns = [
                "format ",
                "diskpart",
                "bcdedit",
                "cipher /w",
                "remove-item -recurse -force c:\\",
                "del /s /q c:\\",
                "rd /s /q c:\\",
                "shutdown /s",
                "shutdown -s",
                "reg delete hk",
            ];
            let cmd_l = cmd.to_lowercase();
            if deny_patterns.iter().any(|p| cmd_l.contains(p)) {
                return Json(ChatResponse {
                    response: "Blocked potentially destructive command.".to_string(),
                    command: None,
                });
            }

            let cfg = crate::bash_executor::BashExecConfig {
                timeout_ms: 20_000,
                working_dir: None,
                env_vars: std::collections::HashMap::new(),
                background: false,
                approval_required: true,
            };

            return match crate::bash_executor::execute_bash(cmd, &cfg) {
                Ok(res) => {
                    let mut out = String::new();
                    if !res.stdout.trim().is_empty() {
                        out.push_str("STDOUT:\n");
                        out.push_str(res.stdout.trim());
                        out.push('\n');
                    }
                    if !res.stderr.trim().is_empty() {
                        out.push_str("STDERR:\n");
                        out.push_str(res.stderr.trim());
                        out.push('\n');
                    }
                    if out.is_empty() {
                        out = "Command executed with no output.".to_string();
                    }
                    Json(ChatResponse {
                        response: format!(
                            "[exit:{} | {}ms]\n{}",
                            res.exit_code,
                            res.duration_ms,
                            out.chars().take(2000).collect::<String>()
                        ),
                        command: Some("run_command".to_string()),
                    })
                }
                Err(e) => Json(ChatResponse {
                    response: format!("Command execution failed: {}", e),
                    command: None,
                }),
            };
        }
    }

    if state.llm_client.is_some() {
        match generate_free_chat_reply(&state, &message).await {
            Ok(response) if !response.is_empty() => {
                if let Err(e) = db::insert_chat_message("assistant", &response) {
                    eprintln!("Failed to save assistant chat: {}", e);
                }
                return Json(ChatResponse {
                    response,
                    command: Some("chat_reply".to_string()),
                });
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Free chat generation failed: {}", e);
                let lower = e.to_lowercase();
                if lower.contains("rate limit") || lower.contains("rate_limit_exceeded") {
                    return Json(ChatResponse {
                        response: "LLM rate limit reached. Please retry in a few seconds.".to_string(),
                        command: Some("error_rate_limited".to_string()),
                    });
                }
            }
        }
    }

    Json(ChatResponse {
        response: "I could not determine the request. Try: system status, calendar, email, open notepad, list files, search <query>.".to_string(),
        command: None,
    })
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

async fn create_routine_handler(Json(payload): Json<CreateRoutineRequest>) -> Json<serde_json::Value> {
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
}

async fn list_recommendations(
    Query(params): Query<RecQueryParams>,
) -> Json<Vec<RecommendationItem>> {
    // Treat empty string as None; default to "all" for history view.
    let filter = params
        .status
        .as_deref()
        .filter(|s| !s.is_empty())
        .or(Some("all"));

    match db::get_recommendations_with_filter(filter) {
        Ok(recs) => Json(
            recs.into_iter()
                .map(|r| RecommendationItem {
                    id: r.id,
                    status: r.status,
                    title: r.title,
                    summary: r.summary,
                    confidence: r.confidence,
                    evidence: r.evidence, // [NEW] Pass evidence
                    last_error: r.last_error,
                })
                .collect()
        ),
        Err(_) => Json(vec![]),
    }
}

async fn approve_recommendation(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("???Received approval request for Recommendation ID: {}", id);

    // 1. Get recommendation from DB
    let rec = match db::get_recommendation(id) {
        Ok(Some(r)) => r,
        _ => {
            eprintln!("??Recommendation #{} not found in DB", id);
            return Err((StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Recommendation not found"}))));
        }
    };

    let n8n_client = match n8n_api::N8nApi::from_env() {
        Ok(c) => c,
        Err(e) => {
             return Err((
                 StatusCode::INTERNAL_SERVER_ERROR, 
                 Json(serde_json::json!({ "error": "n8n Client Init Failed", "details": e.to_string() }))
             ));
        }
    };
    
    // Ensure n8n is running first
    if let Err(e) = n8n_client.ensure_server_running().await {
        eprintln!("??Failed to start n8n: {}", e);
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "n8n Server Unavailable", "details": e.to_string() }))
        ));
    }

    // [NEW] Fetch Credentials to inform LLM
    let credentials = n8n_client.list_credentials().await.unwrap_or_default();
    let cred_context = if credentials.is_empty() {
        "NOTE: No credentials found in n8n. Do NOT use nodes requiring authentication (like Gmail, Slack) unless you are sure.".to_string()
    } else {
        let list = credentials.iter()
            .map(|c| format!("- Name: '{}', ID: '{}', Type: '{}'", c.name, c.id, c.type_name))
            .collect::<Vec<_>>()
            .join("\n");
        format!("IMPORTANT: You MUST use these exact Credential IDs for authentication:\n{}\nIf a required credential is missing, do not hallucinate an ID. Use a placeholder and add a comment.", list)
    };

    let mut current_json = if let Some(json) = &rec.workflow_json {
        json.clone()
    } else {
        // Initial Generation with Credential Context
        if let Some(llm) = &state.llm_client {
             let full_prompt = format!("{}\n\n{}", rec.n8n_prompt, cred_context);
             println!("?壤?Generating workflow with context: {} credentials", credentials.len());
             
             match llm.build_n8n_workflow(&full_prompt).await {
                Ok(json) => json,
                Err(e) => return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "LLM Generation Failed", "details": e.to_string() }))
                )),
             }
        } else {
             return Err((
                 StatusCode::SERVICE_UNAVAILABLE,
                 Json(serde_json::json!({ "error": "LLM Client Unavailable" }))
             ));
        }
    };

    let mut attempts = 0;
    let max_attempts = 3;
    let mut last_error = String::new();

    while attempts < max_attempts {
        attempts += 1;
        
        // Repair JSON if this is a retry
        if attempts > 1 {
            if let Some(llm) = &state.llm_client {
                println!("???Attempting to fix workflow JSON (Try {}/{})", attempts, max_attempts);
                // Also pass credential context during fix
                let fix_prompt = format!("{}\n\n{}", rec.n8n_prompt, cred_context);
                match llm.fix_n8n_workflow(&fix_prompt, &current_json, &last_error).await {
                    Ok(fixed) => current_json = fixed,
                    Err(e) => println!("Failed to fix JSON: {}", e),
                }
            }
        }

        // Parse JSON to Value
        let workflow_data: serde_json::Value = match serde_json::from_str(&current_json)
            .or_else(|_| {
                let cleaned = extract_json_object(&current_json);
                serde_json::from_str(&cleaned)
            }) {
            Ok(v) => v,
            Err(e) => {
                last_error = format!("Invalid JSON Syntax: {}", e);
                continue; 
            }
        };

        // Extract name
        let name = workflow_data["name"].as_str().unwrap_or(&rec.title).to_string();
        
        // Try create
        // SAFETY: Created as inactive (false) to prevent broken loops. User must enable manually.
        match n8n_client.create_workflow(&name, &workflow_data, false).await {
            Ok(workflow_id) => {
                // Success!
                println!("??Workflow created successfully on attempt {}", attempts);
                if let Err(e) = db::mark_recommendation_approved(id, &workflow_id, &current_json) {
                    eprintln!("Failed to update DB: {}", e);
                }
                return Ok(Json(serde_json::json!({
                    "status": "success",
                    "id": workflow_id,
                    "message": "Workflow created successfully"
                })));
            },
            Err(e) => {
                last_error = e.to_string();
                println!("??Creation failed: {}", last_error);
            }
        }
    }
    
    // If we get here, all attempts failed
    let error_msg = format!("??All {} attempts to create workflow failed. Last Error: {}", max_attempts, last_error);
    eprintln!("{}", error_msg);
    if let Err(db_err) = db::mark_recommendation_failed(id, &last_error) {
        eprintln!("Failed to mark recommendation as failed: {}", db_err);
    }
    
    Err((
        StatusCode::BAD_GATEWAY,
        Json(serde_json::json!({ "error": error_msg, "details": last_error }))
    ))
}

fn extract_json_object(input: &str) -> String {
    let start = input.find('{');
    let end = input.rfind('}');
    match (start, end) {
        (Some(s), Some(e)) if e > s => input[s..=e].to_string(),
        _ => input.to_string(),
    }
}

async fn reject_recommendation(
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> StatusCode {
    match db::update_recommendation_status(id, "rejected") {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn later_recommendation(
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> StatusCode {
    match db::update_recommendation_status(id, "later") {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn restore_recommendation(
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> StatusCode {
    match db::update_recommendation_status(id, "pending") {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
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

async fn list_routine_runs(
    Query(query): Query<RoutineRunsQuery>,
) -> Json<Vec<db::RoutineRun>> {
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

async fn list_exec_results(
    Query(query): Query<ExecResultsQuery>,
) -> Json<Vec<db::ExecResult>> {
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

async fn list_nl_runs_handler(
    Query(query): Query<NLRunQuery>,
) -> Json<Vec<db::NLRun>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let runs = db::list_nl_runs(limit).unwrap_or_default();
    Json(runs)
}

async fn nl_run_metrics_handler(
    Query(query): Query<NLRunMetricsQuery>,
) -> Json<db::NLRunMetrics> {
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

async fn set_nl_approval_policy(
    Json(payload): Json<ApprovalPolicyRequest>,
) -> StatusCode {
    if payload.policy_key.trim().is_empty() || payload.decision.trim().is_empty() {
        return StatusCode::BAD_REQUEST;
    }
    match db::upsert_approval_policy(&payload.policy_key, &payload.decision) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn remove_nl_approval_policy(
    Path(key): Path<String>,
) -> StatusCode {
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

async fn add_exec_allowlist(
    Json(payload): Json<ExecAllowlistRequest>,
) -> StatusCode {
    if payload.pattern.trim().is_empty() {
        return StatusCode::BAD_REQUEST;
    }
    match db::add_exec_allowlist(&payload.pattern, payload.cwd.as_deref()) {
        Ok(_) => StatusCode::CREATED,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn remove_exec_allowlist(
    Path(id): Path<i64>,
) -> StatusCode {
    match db::remove_exec_allowlist(id) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[derive(Serialize)]
struct RecommendationMetricsResponse {
    total: i64,
    approved: i64,
    rejected: i64,
    failed: i64,
    pending: i64,
    later: i64,
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
        last_created_at: None,
    });

    let approval_rate = if metrics.total > 0 {
        (metrics.approved as f64 / metrics.total as f64) * 100.0
    } else {
        0.0
    };

    Json(RecommendationMetricsResponse {
        total: metrics.total,
        approved: metrics.approved,
        rejected: metrics.rejected,
        failed: metrics.failed,
        pending: metrics.pending,
        later: metrics.later,
        approval_rate,
        last_created_at: metrics.last_created_at,
    })
}

// Add at top: use crate::recommendation::AutomationProposal; 

async fn analyze_patterns() -> Json<Vec<String>> {
    Json(run_analysis_internal())
}

fn run_analysis_internal() -> Vec<String> {
    let detector = pattern_detector::PatternDetector::new();
    let patterns = detector.analyze();
    
    // 1. Save detected patterns to DB
    for p in &patterns {
        let proposal = crate::recommendation::AutomationProposal {
            title: format!("New Pattern: {}", p.description),
            summary: format!("Detected {} repeats. AI suggests automating this.", p.occurrences),
            trigger: format!("Pattern Type: {:?}", p.pattern_type),
            actions: vec!["Analyze".to_string(), "Automate".to_string()],
            n8n_prompt: format!("Create an automation for: {}", p.description),
            confidence: p.similarity_score,
            evidence: vec![format!("Pattern: {}", p.description)],
            pattern_id: Some(p.pattern_id.clone()),
        };
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
        };
        if let Err(e) = db::insert_recommendation(&proposal) {
            eprintln!("??ル쵑??Failed to save recommendation analysis: {}", e);
        }
        return vec![];
    }
    */
    
    patterns.into_iter()
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
            let executor = crate::executor::AgentExecutor::new(llm);
            match executor.execute_goal(&payload.goal).await {
                Ok(res) => println!("??Goal Execution Success: {}", res),
                Err(e) => println!("??Goal Execution Failed: {}", e),
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

async fn get_current_goal(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let goal = state
        .current_goal
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_default();
    Json(serde_json::json!({ "goal": goal }))
}

async fn agent_intent_handler(
    Json(payload): Json<AgentIntentRequest>,
) -> impl IntoResponse {
    let intent_result = intent_router::classify_intent(&payload.text);
    let fill = slot_filler::fill_slots(&intent_result.intent, intent_result.slots.clone());
    let session = nl_store::create_session(intent_result.clone(), fill.slots.clone(), payload.text.clone());

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

async fn agent_plan_handler(
    Json(payload): Json<AgentPlanRequest>,
) -> impl IntoResponse {
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

async fn agent_execute_handler(
    Json(payload): Json<AgentExecuteRequest>,
) -> impl IntoResponse {
    let Some(plan) = nl_store::get_plan(&payload.plan_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "plan_not_found" })),
        )
            .into_response();
    };

    let mut resume_from = nl_store::get_plan_progress(&plan.plan_id).unwrap_or(0);
    if resume_from >= plan.steps.len() {
        nl_store::clear_plan_progress(&plan.plan_id);
        resume_from = 0;
    }
    let mut result = execution_controller::execute_plan(&plan, resume_from).await;
    if resume_from > 0 {
        result
            .logs
            .insert(0, format!("Resuming from step {}", resume_from + 1));
    }
    let mut verify = verification_engine::verify_execution(&plan, &result.logs);
    if !verify.ok {
        result.logs.push(format!("Verification failed: {}", verify.issues.join("; ")));
    } else {
        result.logs.push("Verification passed".to_string());
    }

    // Simple auto-replan: one retry on failure or verification issues
    let auto_replan = std::env::var("STEER_AUTO_REPLAN")
        .ok()
        .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(true);
    let allow_replan = !matches!(result.status.as_str(), "manual_required" | "approval_required");
    if auto_replan && allow_replan && (result.status == "error" || !verify.ok) {
        result.logs.push("Auto-replan: retrying once after short wait".to_string());
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let retry = execution_controller::execute_plan(&plan, 0).await;
        result.logs = retry.logs;
        result.status = retry.status;
        result.approval = retry.approval;
        result.manual_steps = retry.manual_steps;
        result.resume_from = retry.resume_from;
        verify = verification_engine::verify_execution(&plan, &result.logs);
        if !verify.ok {
            result
                .logs
                .push(format!("Verification failed: {}", verify.issues.join("; ")));
        } else {
            result.logs.push("Verification passed".to_string());
        }
    }

    if matches!(result.status.as_str(), "manual_required" | "approval_required") {
        if let Some(next_step) = result.resume_from {
            nl_store::set_plan_progress(&plan.plan_id, next_step);
        }
    } else {
        nl_store::clear_plan_progress(&plan.plan_id);
    }
    let session = nl_store::find_session_by_plan(&payload.plan_id);
    let summary = extract_summary(&result.logs);
    if let Some(state) = session {
        let _ = db::insert_nl_run(
            state.intent.intent.as_str(),
            &state.prompt,
            &result.status,
            summary.as_deref(),
            Some(&serde_json::to_string(&result.logs).unwrap_or_default()),
        );
    }
    let response = AgentExecuteResponse {
        status: result.status,
        logs: result.logs,
        approval: result.approval,
        manual_steps: result.manual_steps,
        resume_from: result.resume_from,
    };

    (StatusCode::OK, Json(response)).into_response()
}

async fn agent_verify_handler(
    Json(payload): Json<AgentVerifyRequest>,
) -> impl IntoResponse {
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

async fn agent_approve_handler(
    Json(payload): Json<AgentApproveRequest>,
) -> impl IntoResponse {
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

fn extract_summary(logs: &[String]) -> Option<String> {
    logs.iter()
        .find_map(|line| line.strip_prefix("Summary: ").map(|s| s.to_string()))
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
    let history = req.history_summary.unwrap_or_else(|| format!("Goal: {}", goal));
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
                    "??怨뺢덧?꾩룄????꾩룇瑗???嶺뚮ㅄ維싷쭗?밸ご????낆몥??袁⑤콦???곗꽑?? ???곕뻣 ???덈뺄???????곗꽑??".to_string()
                } else {
                    "??怨뺢덧?꾩룄????筌먦끉逾???곗꽑?? ??얜?????熬곣뫁?룟슖???戮?뻣??紐껊퉵??".to_string()
                };
                return Json(FeedbackResponse { action, new_goal, message });
            }
            Err(e) => {
                eprintln!("Feedback analysis failed: {}", e);
            }
        }
    }

    Json(FeedbackResponse {
        action: "complete".to_string(),
        new_goal: None,
        message: "??怨뺢덧?꾩룄????筌먦끉逾???곗꽑?? ??얜?????熬곣뫁?룟슖???戮?뻣??紐껊퉵??".to_string(),
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
                let sessions: Vec<serde_json::Value> = store.list_active()
                    .iter()
                    .map(|s| serde_json::json!({
                        "id": s.id,
                        "key": s.key,
                        "goal": s.goal,
                        "status": format!("{:?}", s.status),
                        "created_at": s.created_at.to_rfc3339(),
                        "updated_at": s.updated_at.to_rfc3339(),
                        "steps_count": s.steps.len(),
                        "can_resume": s.can_resume(),
                    }))
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
        Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() }))
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
        Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() }))
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
                    Ok(false) => Json(serde_json::json!({ "success": false, "error": "Session not found" })),
                    Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() }))
                }
            } else {
                Json(serde_json::json!({ "success": false, "error": "Store not initialized" }))
            }
        }
        Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() }))
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
                        session.add_message("system", &format!("Resuming from step {}", resume_point));
                        
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
        Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() }))
    }
}

