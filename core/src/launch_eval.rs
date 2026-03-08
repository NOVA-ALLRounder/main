use anyhow::{anyhow, Context, Result};
use axum::{routing::post, Json, Router};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::api_server::{AppState, ChatRequest, ChatResponse};
use crate::recommendation::AutomationProposal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalConfig {
    #[serde(default = "default_report_dir")]
    pub report_dir: String,
    #[serde(default)]
    pub include_paths: Vec<String>,
    #[serde(default)]
    pub scenarios: Vec<LaunchEvalScenario>,
}

fn default_report_dir() -> String {
    "reports/launch_eval".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LaunchEvalScenario {
    Chat {
        id: String,
        #[serde(default)]
        description: Option<String>,
        request: LaunchEvalChatInput,
        #[serde(default)]
        ai_digest_mock: Option<AiDigestMock>,
        expect: LaunchEvalChatExpectation,
    },
    RequestMemoryReuse {
        id: String,
        #[serde(default)]
        description: Option<String>,
        seed: RequestMemorySeed,
        request: LaunchEvalChatInput,
        expect: LaunchEvalChatExpectation,
    },
    ExecutionMemoryReuse {
        id: String,
        #[serde(default)]
        description: Option<String>,
        seed: ExecutionMemorySeed,
        request: LaunchEvalChatInput,
        expect: LaunchEvalChatExpectation,
    },
    RequestMemoryPolicy {
        id: String,
        #[serde(default)]
        description: Option<String>,
        seed: RequestMemorySeed,
        request: LaunchEvalChatInput,
        #[serde(default = "default_request_memory_policy_mode")]
        mode: RequestMemoryPolicyMode,
        #[serde(default)]
        feedback: Option<String>,
        #[serde(default)]
        suppress: bool,
        expect_cached: bool,
        #[serde(default)]
        expect_command: Option<String>,
        #[serde(default)]
        expect_response_contains: Vec<String>,
    },
    ExecutionMemoryPolicy {
        id: String,
        #[serde(default)]
        description: Option<String>,
        seed: ExecutionMemorySeed,
        request: LaunchEvalChatInput,
        #[serde(default)]
        lookup_params: Option<Value>,
        #[serde(default)]
        feedback: Option<String>,
        #[serde(default)]
        suppress: bool,
        expect_cached: bool,
        #[serde(default)]
        expect_response_contains: Vec<String>,
    },
    BusinessContract {
        id: String,
        #[serde(default)]
        description: Option<String>,
        plan: LaunchEvalBusinessPlan,
        logs: Vec<String>,
        expect_ok: bool,
        #[serde(default)]
        expect_detail_contains: Vec<String>,
        #[serde(default)]
        expect_assertions: Vec<LaunchEvalAssertionExpectation>,
    },
    MemoryScopeIsolation {
        id: String,
        #[serde(default)]
        description: Option<String>,
        seed: ScopedRequestMemorySeed,
        request: LaunchEvalChatInput,
        expect: LaunchEvalChatExpectation,
    },
    RecommendationGate {
        id: String,
        #[serde(default)]
        description: Option<String>,
        stage: RecommendationGateStage,
        proposal: AutomationProposal,
        expect_ready: bool,
        #[serde(default)]
        expect_reasons_contains: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalChatInput {
    pub message: String,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub chat_type: Option<String>,
    #[serde(default)]
    pub sender: Option<String>,
    #[serde(default)]
    pub mentioned: Option<bool>,
}

impl LaunchEvalChatInput {
    fn as_request(&self) -> ChatRequest {
        ChatRequest {
            message: self.message.clone(),
            channel: self.channel.clone(),
            chat_type: self.chat_type.clone(),
            sender: self.sender.clone(),
            mentioned: self.mentioned,
        }
    }

    fn memory_scope(&self) -> Option<String> {
        build_memory_scope(
            self.channel.as_deref(),
            self.chat_type.as_deref(),
            self.sender.as_deref(),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalChatExpectation {
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub response_contains: Vec<String>,
    #[serde(default)]
    pub response_contains_any: Vec<String>,
    #[serde(default)]
    pub response_not_contains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDigestMock {
    #[serde(default = "default_ai_digest_status")]
    pub status: String,
    #[serde(default)]
    pub notion_url: Option<String>,
    #[serde(default)]
    pub top_headlines_text: Option<String>,
}

fn default_ai_digest_status() -> String {
    "ok".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMemorySeed {
    pub request_text: String,
    pub command: String,
    #[serde(default = "default_json_object")]
    pub params: Value,
    pub response_text: String,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    #[serde(default = "default_request_memory_source")]
    pub source: String,
    #[serde(default)]
    pub response_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMemorySeed {
    pub original_request: String,
    pub command: String,
    #[serde(default = "default_json_object")]
    pub params: Value,
    pub response_text: String,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    #[serde(default = "default_execution_memory_source")]
    pub source: String,
    #[serde(default)]
    pub ttl_seconds: Option<i64>,
    #[serde(default)]
    pub tool_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedRequestMemorySeed {
    pub request_text: String,
    pub command: String,
    #[serde(default = "default_json_object")]
    pub params: Value,
    pub response_text: String,
    pub scope: LaunchEvalScope,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    #[serde(default = "default_request_memory_source")]
    pub source: String,
    #[serde(default)]
    pub response_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalScope {
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub chat_type: Option<String>,
    #[serde(default)]
    pub sender: Option<String>,
}

impl LaunchEvalScope {
    fn memory_scope(&self) -> Option<String> {
        build_memory_scope(
            self.channel.as_deref(),
            self.chat_type.as_deref(),
            self.sender.as_deref(),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationGateStage {
    AutoQueue,
    Approval,
}

impl RecommendationGateStage {
    fn as_str(&self) -> &'static str {
        match self {
            Self::AutoQueue => "auto_queue",
            Self::Approval => "approval",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalBusinessPlan {
    pub intent: crate::nl_automation::IntentType,
    #[serde(default)]
    pub descriptions: Vec<String>,
    #[serde(default)]
    pub slots: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalAssertionExpectation {
    pub key: String,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestMemoryPolicyMode {
    Response,
    Intent,
}

impl RequestMemoryPolicyMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Response => "response",
            Self::Intent => "intent",
        }
    }
}

fn default_request_memory_policy_mode() -> RequestMemoryPolicyMode {
    RequestMemoryPolicyMode::Response
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalReport {
    pub generated_at: String,
    pub config_path: Option<String>,
    pub db_path: String,
    pub report_json_path: String,
    pub report_markdown_path: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<LaunchEvalCaseResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalCaseResult {
    pub id: String,
    pub kind: String,
    pub description: Option<String>,
    pub passed: bool,
    pub errors: Vec<String>,
    pub notes: Vec<String>,
    pub command: Option<String>,
    pub response_preview: Option<String>,
    pub readiness: Option<LaunchEvalReadinessReport>,
    pub admission: Option<LaunchEvalAdmissionReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalReadinessReport {
    pub ready: bool,
    pub reasons: Vec<String>,
    pub category: String,
    pub business_score: f64,
    pub confidence: f64,
    pub occurrences: Option<u32>,
    pub distinct_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalAdmissionReport {
    pub accepted: bool,
    pub reasons: Vec<String>,
    pub priority_score: f64,
    pub pending_same_category: usize,
    pub pending_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalCandidate {
    pub id: String,
    #[serde(default = "default_real_provenance")]
    pub provenance: String,
    pub source_kind: String,
    pub scenario_kind: String,
    pub title: String,
    pub score: f64,
    pub command: Option<String>,
    pub request_message: String,
    pub rationale: Vec<String>,
    pub yaml: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalCandidateSnapshot {
    pub generated_at: String,
    pub output_path: String,
    #[serde(default = "default_real_provenance")]
    pub provenance_filter: String,
    pub scenario_count: usize,
    pub candidate_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalCandidateSnapshotInfo {
    pub output_path: String,
    pub exists: bool,
    #[serde(default = "default_real_provenance")]
    pub provenance_filter: String,
    pub scenario_count: usize,
    pub updated_at: Option<String>,
    pub scenario_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchEvalCandidateProvenanceFilter {
    Real,
    Synthetic,
    All,
}

impl LaunchEvalCandidateProvenanceFilter {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Real => "real",
            Self::Synthetic => "synthetic",
            Self::All => "all",
        }
    }
}

pub fn parse_launch_eval_candidate_provenance_filter(
    raw: Option<&str>,
) -> LaunchEvalCandidateProvenanceFilter {
    match raw.unwrap_or("real").trim().to_ascii_lowercase().as_str() {
        "synthetic" | "dogfood" => LaunchEvalCandidateProvenanceFilter::Synthetic,
        "all" | "mixed" => LaunchEvalCandidateProvenanceFilter::All,
        _ => LaunchEvalCandidateProvenanceFilter::Real,
    }
}

fn default_json_object() -> Value {
    json!({})
}

fn default_real_provenance() -> String {
    "real".to_string()
}

fn default_confidence() -> f64 {
    0.95
}

fn default_request_memory_source() -> String {
    "launch.eval.request_seed".to_string()
}

fn default_execution_memory_source() -> String {
    "launch.eval.execution_seed".to_string()
}

pub async fn run_launch_eval_from_path(config_path: &Path) -> Result<LaunchEvalReport> {
    let config = load_launch_eval_config(config_path)?;
    run_launch_eval(config, Some(config_path)).await
}

pub async fn run_launch_eval(
    config: LaunchEvalConfig,
    config_path: Option<&Path>,
) -> Result<LaunchEvalReport> {
    crate::load_env_with_fallback();

    let report_dir = PathBuf::from(config.report_dir.trim());
    fs::create_dir_all(&report_dir)
        .with_context(|| format!("failed to create report dir {}", report_dir.display()))?;

    let db_path =
        std::env::temp_dir().join(format!("allvia-launch-eval-{}.db", uuid::Uuid::new_v4()));
    let json_path = report_dir.join("latest.json");
    let markdown_path = report_dir.join("latest.md");

    let report = {
        let _env_guard = ScopedEnvGuard::set(vec![
            ("STEER_DB_PATH", Some(db_path.to_string_lossy().to_string())),
            ("CHAT_GATE_ENABLED", Some("0".to_string())),
            (
                "ALLVIA_ENABLE_RECOMMENDATION_CATEGORY_BROWSING",
                Some("0".to_string()),
            ),
            (
                "ALLVIA_AUTO_APPROVAL_MIN_BUSINESS_SCORE",
                Some("0.68".to_string()),
            ),
            (
                "ALLVIA_AUTO_APPROVAL_MIN_CONFIDENCE",
                Some("0.80".to_string()),
            ),
            (
                "ALLVIA_AUTO_APPROVAL_MIN_DISTINCT_DAYS",
                Some("3".to_string()),
            ),
            (
                "ALLVIA_AUTO_APPROVAL_MIN_OCCURRENCES",
                Some("4".to_string()),
            ),
            ("ALLVIA_WORK_PATTERN_CONTEXT_MIN", Some("0.6".to_string())),
            ("ALLVIA_AUTO_PENDING_WORK_LIMIT", Some("5".to_string())),
            ("ALLVIA_AUTO_PENDING_OTHER_LIMIT", Some("0".to_string())),
            (
                "ALLVIA_PENDING_RECOMMENDATION_DISPLAY_LIMIT",
                Some("5".to_string()),
            ),
            (
                "ALLVIA_RESPONSE_CACHE_TTL_GMAIL_LIST",
                Some("20".to_string()),
            ),
            (
                "ALLVIA_RESPONSE_CACHE_TTL_CALENDAR_TODAY",
                Some("30".to_string()),
            ),
            (
                "ALLVIA_RESPONSE_CACHE_TTL_CALENDAR_WEEK",
                Some("120".to_string()),
            ),
            (
                "ALLVIA_RESPONSE_CACHE_TTL_SYSTEM_STATUS",
                Some("10".to_string()),
            ),
            (
                "ALLVIA_REPEAT_REQUEST_FRESH_WINDOW_SECONDS",
                Some("15".to_string()),
            ),
        ]);

        crate::db::reset_connection();
        crate::db::init().context("failed to initialize launch eval DB")?;

        let mut results = Vec::new();
        for scenario in config.scenarios {
            let result = run_scenario(&scenario).await;
            results.push(result);
        }

        let total = results.len();
        let passed = results.iter().filter(|result| result.passed).count();
        let failed = total.saturating_sub(passed);

        LaunchEvalReport {
            generated_at: chrono::Utc::now().to_rfc3339(),
            config_path: config_path.map(|path| path.display().to_string()),
            db_path: db_path.display().to_string(),
            report_json_path: json_path.display().to_string(),
            report_markdown_path: markdown_path.display().to_string(),
            total,
            passed,
            failed,
            results,
        }
    };
    crate::db::reset_connection();

    fs::write(
        &json_path,
        serde_json::to_string_pretty(&report).context("failed to serialize launch eval JSON")?,
    )
    .with_context(|| format!("failed to write {}", json_path.display()))?;
    fs::write(&markdown_path, render_markdown_report(&report))
        .with_context(|| format!("failed to write {}", markdown_path.display()))?;

    Ok(report)
}

pub fn generate_launch_eval_candidates(limit: usize) -> Vec<LaunchEvalCandidate> {
    let workdir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    generate_launch_eval_candidates_with_filter(
        &workdir,
        limit,
        LaunchEvalCandidateProvenanceFilter::Real,
    )
}

pub fn generate_launch_eval_candidates_with_filter(
    workdir: &Path,
    limit: usize,
    provenance_filter: LaunchEvalCandidateProvenanceFilter,
) -> Vec<LaunchEvalCandidate> {
    crate::load_env_with_fallback();
    let _ = crate::db::init();

    let candidate_limit = limit.clamp(1, 50);
    let sample_limit = ((candidate_limit as i64) * 4).clamp(12, 200);
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    if matches!(
        provenance_filter,
        LaunchEvalCandidateProvenanceFilter::Real | LaunchEvalCandidateProvenanceFilter::All
    ) {
        let request_records =
            crate::db::list_request_memory_records(sample_limit, false).unwrap_or_default();
        let execution_records =
            crate::db::list_execution_memory_records(sample_limit, false).unwrap_or_default();
        let launch_events = crate::db::list_launch_ops_events(sample_limit).unwrap_or_default();
        let task_runs = crate::db::list_task_runs(sample_limit, None).unwrap_or_default();
        for candidate in generate_launch_eval_candidates_from_sources(
            &request_records,
            &execution_records,
            &launch_events,
            candidate_limit,
        ) {
            if seen.insert(candidate.id.clone()) {
                candidates.push(candidate);
            }
        }
        for run in task_runs {
            let Some(candidate) = task_run_business_contract_candidate(&run) else {
                continue;
            };
            if seen.insert(candidate.id.clone()) {
                candidates.push(candidate);
            }
        }
    }

    if matches!(
        provenance_filter,
        LaunchEvalCandidateProvenanceFilter::Synthetic | LaunchEvalCandidateProvenanceFilter::All
    ) {
        for candidate in
            generate_synthetic_launch_eval_candidates_from_config(workdir, candidate_limit)
        {
            if seen.insert(candidate.id.clone()) {
                candidates.push(candidate);
            }
        }
    }

    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    candidates.truncate(candidate_limit);
    candidates
}

pub fn write_launch_eval_candidate_snapshot(
    workdir: &Path,
    output_path: Option<&str>,
    limit: usize,
) -> Result<LaunchEvalCandidateSnapshot> {
    write_launch_eval_candidate_snapshot_with_filter(
        workdir,
        output_path,
        limit,
        LaunchEvalCandidateProvenanceFilter::Real,
    )
}

pub fn write_launch_eval_candidate_snapshot_with_filter(
    workdir: &Path,
    output_path: Option<&str>,
    limit: usize,
    provenance_filter: LaunchEvalCandidateProvenanceFilter,
) -> Result<LaunchEvalCandidateSnapshot> {
    let candidates = generate_launch_eval_candidates_with_filter(workdir, limit, provenance_filter);
    let scenarios = candidates
        .iter()
        .filter_map(|candidate| scenario_from_candidate_yaml(&candidate.yaml))
        .collect::<Vec<_>>();
    let output_path = resolve_candidate_snapshot_path(workdir, output_path, provenance_filter);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create snapshot dir {}", parent.display()))?;
    }

    let snapshot_config = LaunchEvalConfig {
        report_dir: default_report_dir(),
        include_paths: Vec::new(),
        scenarios,
    };
    fs::write(
        &output_path,
        serde_yaml::to_string(&snapshot_config)
            .context("failed to serialize launch eval candidate snapshot")?,
    )
    .with_context(|| format!("failed to write {}", output_path.display()))?;

    Ok(LaunchEvalCandidateSnapshot {
        generated_at: chrono::Utc::now().to_rfc3339(),
        output_path: output_path.display().to_string(),
        provenance_filter: provenance_filter.as_str().to_string(),
        scenario_count: snapshot_config.scenarios.len(),
        candidate_ids: candidates
            .into_iter()
            .map(|candidate| candidate.id)
            .collect(),
    })
}

pub fn read_launch_eval_candidate_snapshot_info(
    workdir: &Path,
    output_path: Option<&str>,
) -> LaunchEvalCandidateSnapshotInfo {
    read_launch_eval_candidate_snapshot_info_with_filter(
        workdir,
        output_path,
        LaunchEvalCandidateProvenanceFilter::Real,
    )
}

pub fn read_launch_eval_candidate_snapshot_info_with_filter(
    workdir: &Path,
    output_path: Option<&str>,
    provenance_filter: LaunchEvalCandidateProvenanceFilter,
) -> LaunchEvalCandidateSnapshotInfo {
    let output_path = resolve_candidate_snapshot_path(workdir, output_path, provenance_filter);
    let metadata = fs::metadata(&output_path).ok();
    let updated_at = metadata
        .as_ref()
        .and_then(|meta| meta.modified().ok())
        .map(chrono::DateTime::<chrono::Utc>::from)
        .map(|dt| dt.to_rfc3339());
    let config = fs::read_to_string(&output_path)
        .ok()
        .and_then(|raw| serde_yaml::from_str::<LaunchEvalConfig>(&raw).ok());
    let scenarios = config
        .as_ref()
        .map(|value| value.scenarios.as_slice())
        .unwrap_or(&[]);

    LaunchEvalCandidateSnapshotInfo {
        output_path: output_path.display().to_string(),
        exists: metadata.is_some(),
        provenance_filter: provenance_filter.as_str().to_string(),
        scenario_count: scenarios.len(),
        updated_at,
        scenario_ids: scenarios.iter().map(scenario_id).collect(),
    }
}

fn generate_launch_eval_candidates_from_sources(
    request_records: &[crate::db::RequestMemoryRecord],
    execution_records: &[crate::db::ExecutionMemoryRecord],
    launch_events: &[crate::db::LaunchOpsEventRecord],
    limit: usize,
) -> Vec<LaunchEvalCandidate> {
    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for record in request_records {
        let Some(candidate) = request_memory_candidate(record) else {
            continue;
        };
        let dedupe_key = format!(
            "{}::{}::{}",
            candidate.scenario_kind,
            candidate.command.clone().unwrap_or_default(),
            normalize_request(&candidate.request_message)
        );
        if seen.insert(dedupe_key) {
            candidates.push(candidate);
        }
    }

    for record in execution_records {
        let Some(candidate) = execution_memory_candidate(record) else {
            continue;
        };
        let dedupe_key = format!(
            "{}::{}::{}",
            candidate.scenario_kind,
            candidate.command.clone().unwrap_or_default(),
            normalize_request(&candidate.request_message)
        );
        if seen.insert(dedupe_key) {
            candidates.push(candidate);
        }
    }

    for event in launch_events {
        let Some(candidate) = launch_event_candidate(event) else {
            continue;
        };
        let dedupe_key = format!(
            "{}::{}::{}",
            candidate.scenario_kind,
            candidate.command.clone().unwrap_or_default(),
            normalize_request(&candidate.request_message)
        );
        if seen.insert(dedupe_key) {
            candidates.push(candidate);
        }
    }

    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    candidates.truncate(limit);
    candidates
}

fn load_launch_eval_config(config_path: &Path) -> Result<LaunchEvalConfig> {
    let mut visited = HashSet::new();
    load_launch_eval_config_recursive(config_path, &mut visited)
}

fn load_launch_eval_config_shallow(config_path: &Path) -> Result<LaunchEvalConfig> {
    let canonical = fs::canonicalize(config_path)
        .with_context(|| format!("failed to resolve config {}", config_path.display()))?;
    let raw = fs::read_to_string(&canonical)
        .with_context(|| format!("failed to read config {}", canonical.display()))?;
    serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse config {}", canonical.display()))
}

fn load_launch_eval_config_recursive(
    config_path: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<LaunchEvalConfig> {
    let canonical = fs::canonicalize(config_path)
        .with_context(|| format!("failed to resolve config {}", config_path.display()))?;
    if !visited.insert(canonical.clone()) {
        return Err(anyhow!(
            "launch eval config include loop detected: {}",
            canonical.display()
        ));
    }

    let raw = fs::read_to_string(&canonical)
        .with_context(|| format!("failed to read config {}", canonical.display()))?;
    let mut config: LaunchEvalConfig = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse config {}", canonical.display()))?;

    let include_paths = config.include_paths.clone();
    for include in include_paths {
        let include_path = resolve_relative_path(canonical.parent(), &include);
        let included = load_launch_eval_config_recursive(&include_path, visited)?;
        config.scenarios.extend(included.scenarios);
    }

    visited.remove(&canonical);
    Ok(config)
}

fn resolve_relative_path(base: Option<&Path>, raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() {
        candidate
    } else {
        base.unwrap_or_else(|| Path::new(".")).join(candidate)
    }
}

fn resolve_candidate_snapshot_path(
    workdir: &Path,
    output_path: Option<&str>,
    provenance_filter: LaunchEvalCandidateProvenanceFilter,
) -> PathBuf {
    let raw = output_path.unwrap_or(match provenance_filter {
        LaunchEvalCandidateProvenanceFilter::Real => "configs/launch_eval.generated.yaml",
        LaunchEvalCandidateProvenanceFilter::Synthetic => {
            "configs/launch_eval.dogfood.generated.yaml"
        }
        LaunchEvalCandidateProvenanceFilter::All => "configs/launch_eval.mixed.generated.yaml",
    });
    resolve_relative_path(Some(workdir), raw)
}

fn scenario_from_candidate_yaml(raw: &str) -> Option<LaunchEvalScenario> {
    let snippet: LaunchEvalScenarioSnippet = serde_yaml::from_str(raw).ok()?;
    snippet.scenarios.into_iter().next()
}

fn generate_synthetic_launch_eval_candidates_from_config(
    workdir: &Path,
    limit: usize,
) -> Vec<LaunchEvalCandidate> {
    let config_path = resolve_relative_path(Some(workdir), "configs/launch_eval.yaml");
    let Ok(config) = load_launch_eval_config_shallow(&config_path) else {
        return Vec::new();
    };

    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for scenario in &config.scenarios {
        let Some(candidate) = synthetic_candidate_from_scenario(scenario) else {
            continue;
        };
        if seen.insert(candidate.id.clone()) {
            candidates.push(candidate);
        }
    }

    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    candidates.truncate(limit.clamp(1, 50));
    candidates
}

fn synthetic_candidate_from_scenario(scenario: &LaunchEvalScenario) -> Option<LaunchEvalCandidate> {
    let (title, score, command, request_message, mut rationale) = match scenario {
        LaunchEvalScenario::Chat {
            request,
            expect,
            ai_digest_mock,
            ..
        } => (
            format!("Dogfood chat: {}", summarize_message(&request.message)),
            if ai_digest_mock.is_some() { 93.0 } else { 89.0 },
            expect.command.clone(),
            request.message.clone(),
            vec![
                "synthetic dogfood".to_string(),
                "curated launch_eval scenario".to_string(),
            ],
        ),
        LaunchEvalScenario::RequestMemoryReuse {
            request,
            seed,
            expect,
            ..
        } => (
            format!(
                "Dogfood request memory: {}",
                summarize_message(&request.message)
            ),
            86.0,
            expect
                .command
                .clone()
                .or_else(|| Some(seed.command.clone())),
            request.message.clone(),
            vec![
                "synthetic dogfood".to_string(),
                "request memory reuse".to_string(),
            ],
        ),
        LaunchEvalScenario::ExecutionMemoryReuse {
            request,
            seed,
            expect,
            ..
        } => (
            format!(
                "Dogfood execution memory: {}",
                summarize_message(&request.message)
            ),
            87.0,
            expect
                .command
                .clone()
                .or_else(|| Some(seed.command.clone())),
            request.message.clone(),
            vec![
                "synthetic dogfood".to_string(),
                "execution memory reuse".to_string(),
            ],
        ),
        LaunchEvalScenario::RequestMemoryPolicy {
            request,
            seed,
            mode,
            expect_cached,
            ..
        } => (
            format!(
                "Dogfood request cache policy: {}",
                summarize_message(&request.message)
            ),
            if *expect_cached { 82.0 } else { 90.0 },
            Some(seed.command.clone()),
            request.message.clone(),
            vec![
                "synthetic dogfood".to_string(),
                format!("request_memory_policy={}", mode.as_str()),
            ],
        ),
        LaunchEvalScenario::ExecutionMemoryPolicy {
            request,
            seed,
            expect_cached,
            ..
        } => (
            format!(
                "Dogfood execution cache policy: {}",
                summarize_message(&request.message)
            ),
            if *expect_cached { 82.0 } else { 90.0 },
            Some(seed.command.clone()),
            request.message.clone(),
            vec![
                "synthetic dogfood".to_string(),
                "execution memory policy".to_string(),
            ],
        ),
        LaunchEvalScenario::BusinessContract { id, plan, .. } => (
            format!("Dogfood write contract: {}", summarize_message(id)),
            92.0,
            None,
            summarize_business_contract(plan),
            vec![
                "synthetic dogfood".to_string(),
                "write-action business contract".to_string(),
            ],
        ),
        LaunchEvalScenario::MemoryScopeIsolation { request, seed, .. } => (
            format!(
                "Dogfood scope isolation: {}",
                summarize_message(&request.message)
            ),
            88.0,
            Some(seed.command.clone()),
            request.message.clone(),
            vec![
                "synthetic dogfood".to_string(),
                "memory scope isolation".to_string(),
            ],
        ),
        LaunchEvalScenario::RecommendationGate {
            proposal,
            stage,
            expect_ready,
            ..
        } => (
            format!(
                "Dogfood recommendation gate: {}",
                summarize_message(&proposal.title)
            ),
            if *expect_ready { 88.0 } else { 85.0 },
            None,
            proposal.title.clone(),
            vec![
                "synthetic dogfood".to_string(),
                format!("recommendation_gate={}", stage.as_str()),
            ],
        ),
    };

    rationale.push(format!("scenario_id={}", scenario_id(scenario)));

    Some(LaunchEvalCandidate {
        id: scenario_id(scenario),
        provenance: "synthetic".to_string(),
        source_kind: "synthetic_config".to_string(),
        scenario_kind: scenario_kind_label(scenario).to_string(),
        title,
        score,
        command,
        request_message,
        rationale,
        yaml: render_scenario_yaml(scenario),
    })
}

fn scenario_kind_label(scenario: &LaunchEvalScenario) -> &'static str {
    match scenario {
        LaunchEvalScenario::Chat { .. } => "chat",
        LaunchEvalScenario::RequestMemoryReuse { .. } => "request_memory_reuse",
        LaunchEvalScenario::ExecutionMemoryReuse { .. } => "execution_memory_reuse",
        LaunchEvalScenario::RequestMemoryPolicy { .. } => "request_memory_policy",
        LaunchEvalScenario::ExecutionMemoryPolicy { .. } => "execution_memory_policy",
        LaunchEvalScenario::BusinessContract { .. } => "business_contract",
        LaunchEvalScenario::MemoryScopeIsolation { .. } => "memory_scope_isolation",
        LaunchEvalScenario::RecommendationGate { .. } => "recommendation_gate",
    }
}

fn summarize_business_contract(plan: &LaunchEvalBusinessPlan) -> String {
    let joined = plan
        .descriptions
        .iter()
        .take(2)
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    if joined.trim().is_empty() {
        "Write-action business contract".to_string()
    } else {
        summarize_message(&joined)
    }
}

fn is_synthetic_source(source: &str) -> bool {
    let normalized = source.trim().to_ascii_lowercase();
    normalized.starts_with("unit.test")
        || normalized.starts_with("launch.eval")
        || normalized.starts_with("launch.dogfood.synthetic")
}

fn is_synthetic_launch_event(event: &crate::db::LaunchOpsEventRecord) -> bool {
    event
        .channel
        .as_deref()
        .map(|value| value.eq_ignore_ascii_case("dogfood"))
        .unwrap_or(false)
        || event
            .note
            .as_deref()
            .map(|value| {
                let normalized = value.to_ascii_lowercase();
                normalized.contains("synthetic dogfood")
                    || normalized.contains("launch.eval")
                    || normalized.contains("launch.dogfood.synthetic")
            })
            .unwrap_or(false)
}

fn request_memory_candidate(
    record: &crate::db::RequestMemoryRecord,
) -> Option<LaunchEvalCandidate> {
    let command = record.intent_command.as_deref()?.trim();
    if !matches!(command, "calendar_today" | "calendar_week" | "gmail_list") {
        return None;
    }
    if is_synthetic_source(&record.source) {
        return None;
    }

    let params = request_memory_params(record.intent_json.as_deref());
    let scope = candidate_scope(&record.memory_scope, command);
    let synthetic_marker = format!(
        "LAUNCH_EVAL_REQUEST_CACHE_{}",
        slug_id(&record.original_request)
    );
    let scenario = LaunchEvalScenario::RequestMemoryReuse {
        id: candidate_id("request-memory", command, &record.original_request),
        description: Some(format!(
            "Generated from request memory (use_count={}, confidence={:.2}).",
            record.use_count, record.confidence
        )),
        seed: RequestMemorySeed {
            request_text: record.original_request.clone(),
            command: command.to_string(),
            params,
            response_text: synthetic_marker.clone(),
            confidence: record.confidence.max(0.8),
            source: "launch.eval.candidate.request_memory".to_string(),
            response_mode: Some(record.response_mode.clone()),
        },
        request: LaunchEvalChatInput {
            message: record.original_request.clone(),
            channel: scope.channel.clone(),
            chat_type: scope.chat_type.clone(),
            sender: scope.sender.clone(),
            mentioned: Some(false),
        },
        expect: LaunchEvalChatExpectation {
            command: Some(command.to_string()),
            response_contains: vec![synthetic_marker.clone()],
            response_contains_any: vec![],
            response_not_contains: vec![],
        },
    };

    Some(LaunchEvalCandidate {
        id: scenario_id(&scenario),
        provenance: "real".to_string(),
        source_kind: "request_memory".to_string(),
        scenario_kind: "request_memory_reuse".to_string(),
        title: format!(
            "Request memory: {}",
            summarize_message(&record.original_request)
        ),
        score: request_memory_score(record),
        command: Some(command.to_string()),
        request_message: record.original_request.clone(),
        rationale: vec![
            format!("use_count={}", record.use_count),
            format!("confidence={:.2}", record.confidence),
            format!(
                "feedback=+{} -{}",
                record.positive_feedback_count, record.negative_feedback_count
            ),
        ],
        yaml: render_scenario_yaml(&scenario),
    })
}

fn execution_memory_candidate(
    record: &crate::db::ExecutionMemoryRecord,
) -> Option<LaunchEvalCandidate> {
    if !record.success || record.suppressed {
        return None;
    }
    if !matches!(
        record.intent_command.as_str(),
        "calendar_today" | "calendar_week" | "gmail_list"
    ) {
        return None;
    }
    if is_synthetic_source(&record.source) {
        return None;
    }

    let request_message = execution_request_message(record)?;
    let params = parse_json_object(record.params_json.as_deref());
    let scope = candidate_scope(&record.memory_scope, &record.intent_command);
    let synthetic_marker = format!(
        "LAUNCH_EVAL_EXECUTION_CACHE_{}",
        slug_id(&format!("{}-{}", record.intent_command, request_message))
    );
    let scenario = LaunchEvalScenario::ExecutionMemoryReuse {
        id: candidate_id("execution-memory", &record.intent_command, &request_message),
        description: Some(format!(
            "Generated from execution memory (use_count={}, ttl={}s).",
            record.use_count, record.freshness_ttl_seconds
        )),
        seed: ExecutionMemorySeed {
            original_request: request_message.clone(),
            command: record.intent_command.clone(),
            params,
            response_text: synthetic_marker.clone(),
            confidence: execution_memory_confidence(record),
            source: "launch.eval.candidate.execution_memory".to_string(),
            ttl_seconds: Some(record.freshness_ttl_seconds.max(1)),
            tool_path: Some(record.tool_path.clone()),
        },
        request: LaunchEvalChatInput {
            message: request_message.clone(),
            channel: scope.channel.clone(),
            chat_type: scope.chat_type.clone(),
            sender: scope.sender.clone(),
            mentioned: Some(false),
        },
        expect: LaunchEvalChatExpectation {
            command: Some(record.intent_command.clone()),
            response_contains: vec![synthetic_marker.clone()],
            response_contains_any: vec![],
            response_not_contains: vec![],
        },
    };

    Some(LaunchEvalCandidate {
        id: scenario_id(&scenario),
        provenance: "real".to_string(),
        source_kind: "execution_memory".to_string(),
        scenario_kind: "execution_memory_reuse".to_string(),
        title: format!("Execution memory: {}", summarize_message(&request_message)),
        score: execution_memory_score(record),
        command: Some(record.intent_command.clone()),
        request_message,
        rationale: vec![
            format!("use_count={}", record.use_count),
            format!("ttl={}s", record.freshness_ttl_seconds),
            format!(
                "feedback=+{} -{}",
                record.positive_feedback_count, record.negative_feedback_count
            ),
        ],
        yaml: render_scenario_yaml(&scenario),
    })
}

fn launch_event_candidate(event: &crate::db::LaunchOpsEventRecord) -> Option<LaunchEvalCandidate> {
    if event.outcome != "success" {
        return None;
    }
    if is_synthetic_launch_event(event) {
        return None;
    }
    let command = event.command.as_deref()?.trim();
    if !matches!(
        command,
        "calendar_today" | "calendar_week" | "gmail_list" | "build_workflow" | "ai_digest_program"
    ) {
        return None;
    }
    if event.message_preview.trim().is_empty() {
        return None;
    }

    let scope = candidate_scope(event.memory_scope.as_deref().unwrap_or("global"), command);
    let ai_digest_mock = if command == "ai_digest_program" {
        Some(AiDigestMock {
            status: "ok".to_string(),
            notion_url: Some(format!(
                "https://www.notion.so/launch-eval-{}",
                slug_id(&event.message_preview)
            )),
            top_headlines_text: Some(
                "1. Launch candidate headline A\n2. Launch candidate headline B".to_string(),
            ),
        })
    } else {
        None
    };

    let expect = if let Some(mock) = ai_digest_mock.as_ref() {
        LaunchEvalChatExpectation {
            command: Some(command.to_string()),
            response_contains: vec![
                mock.notion_url.clone().unwrap_or_default(),
                "Launch candidate headline A".to_string(),
            ],
            response_contains_any: vec![],
            response_not_contains: vec![],
        }
    } else {
        LaunchEvalChatExpectation {
            command: Some(command.to_string()),
            response_contains: vec![],
            response_contains_any: vec![],
            response_not_contains: vec![],
        }
    };

    let scenario = LaunchEvalScenario::Chat {
        id: candidate_id("chat", command, &event.message_preview),
        description: Some(format!(
            "Generated from launch ops route '{}' (channel={}).",
            event.route_kind,
            event.channel.as_deref().unwrap_or("unknown")
        )),
        request: LaunchEvalChatInput {
            message: event.message_preview.clone(),
            channel: scope.channel.clone(),
            chat_type: scope.chat_type.clone(),
            sender: scope.sender.clone(),
            mentioned: Some(false),
        },
        ai_digest_mock,
        expect,
    };

    Some(LaunchEvalCandidate {
        id: scenario_id(&scenario),
        provenance: "real".to_string(),
        source_kind: "launch_ops".to_string(),
        scenario_kind: "chat".to_string(),
        title: format!("Chat route: {}", summarize_message(&event.message_preview)),
        score: launch_event_score(event),
        command: Some(command.to_string()),
        request_message: event.message_preview.clone(),
        rationale: vec![
            format!("route_kind={}", event.route_kind),
            format!(
                "memory_hits=intent:{} request:{} execution:{}",
                event.intent_memory_hit, event.request_memory_hit, event.execution_memory_hit
            ),
            format!("confidence={:.2}", event.confidence.unwrap_or(0.0)),
        ],
        yaml: render_scenario_yaml(&scenario),
    })
}

fn task_run_business_contract_candidate(
    run: &crate::db::TaskRunRecord,
) -> Option<LaunchEvalCandidate> {
    if !run.business_complete || !run.execution_complete || !run.planner_complete {
        return None;
    }
    if run.prompt.trim().is_empty() || is_synthetic_task_run(run) {
        return None;
    }

    let artifacts = crate::db::list_task_run_artifacts(&run.run_id).ok()?;
    let logs = sanitize_task_run_logs(task_run_logs(run, &artifacts));
    if logs.is_empty() {
        return None;
    }

    let prompt = sanitize_candidate_text(&run.prompt);
    let summary = run
        .summary
        .as_deref()
        .map(sanitize_candidate_text)
        .unwrap_or_default();
    let passed_artifacts = artifacts
        .iter()
        .filter(|artifact| artifact.value.eq_ignore_ascii_case("true"))
        .count();
    let expect_assertions = artifacts
        .iter()
        .filter(|artifact| artifact.artifact_type == "artifact_assertion")
        .filter_map(|artifact| {
            if !artifact.artifact_key.starts_with("artifact.") {
                return None;
            }
            Some(LaunchEvalAssertionExpectation {
                key: artifact.artifact_key.clone(),
                passed: parse_boolish(&artifact.value),
            })
        })
        .collect::<Vec<_>>();

    let scenario = LaunchEvalScenario::BusinessContract {
        id: candidate_id("task-run-business", &run.intent, &prompt),
        description: Some(format!(
            "Generated from completed task run {} (status={}).",
            run.run_id, run.status
        )),
        plan: LaunchEvalBusinessPlan {
            intent: infer_task_run_intent(&run.intent, &prompt),
            descriptions: vec![prompt.clone()],
            slots: std::collections::HashMap::new(),
        },
        logs,
        expect_ok: true,
        expect_detail_contains: summary
            .trim()
            .is_empty()
            .then(Vec::new)
            .unwrap_or_else(|| vec![summary.clone()]),
        expect_assertions,
    };

    Some(LaunchEvalCandidate {
        id: scenario_id(&scenario),
        provenance: "real".to_string(),
        source_kind: "task_run".to_string(),
        scenario_kind: "business_contract".to_string(),
        title: format!("Task run contract: {}", summarize_message(&prompt)),
        score: task_run_candidate_score(run, artifacts.len(), passed_artifacts),
        command: None,
        request_message: prompt,
        rationale: vec![
            format!("run_id={}", run.run_id),
            format!("status={}", run.status),
            format!("artifacts={}", artifacts.len()),
            format!("assertions_passed={}", passed_artifacts),
        ],
        yaml: render_scenario_yaml(&scenario),
    })
}

fn task_run_logs(
    run: &crate::db::TaskRunRecord,
    artifacts: &[crate::db::TaskRunArtifactRecord],
) -> Vec<String> {
    let mut logs = run
        .details
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
        .unwrap_or_default();
    if logs.is_empty() {
        if let Some(details) = run
            .details
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            logs.push(details.to_string());
        }
    }
    if let Some(summary) = run
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !logs.iter().any(|line| line.starts_with("Summary: ")) {
            logs.insert(0, format!("Summary: {}", summary));
        }
    }
    for artifact in artifacts {
        if artifact.artifact_type == "artifact_assertion" {
            continue;
        }
        if artifact.artifact_key.starts_with("artifact.") {
            logs.push(format!(
                "artifact_snapshot|type={}|key={}|value={}",
                artifact.artifact_type, artifact.artifact_key, artifact.value
            ));
        }
    }
    logs
}

fn is_synthetic_task_run(run: &crate::db::TaskRunRecord) -> bool {
    let prompt = run.prompt.to_ascii_lowercase();
    let summary = run
        .summary
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let details = run
        .details
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    run.run_id.starts_with("test")
        || run.run_id.starts_with("launch-eval")
        || prompt.contains("launch.eval")
        || prompt.contains("launch eval")
        || summary.contains("launch.eval")
        || details.contains("launch.eval")
        || prompt.contains("artifact upsert")
}

fn infer_task_run_intent(raw_intent: &str, prompt: &str) -> crate::nl_automation::IntentType {
    let haystack = format!(
        "{} {}",
        raw_intent.to_ascii_lowercase(),
        prompt.to_ascii_lowercase()
    );
    if haystack.contains("flight") || haystack.contains("항공") {
        crate::nl_automation::IntentType::FlightSearch
    } else if haystack.contains("shop")
        || haystack.contains("shopping")
        || haystack.contains("compare")
        || haystack.contains("상품")
    {
        crate::nl_automation::IntentType::ShoppingCompare
    } else if haystack.contains("form") || haystack.contains("신청서") || haystack.contains("폼")
    {
        crate::nl_automation::IntentType::FormFill
    } else {
        crate::nl_automation::IntentType::GenericTask
    }
}

fn task_run_candidate_score(
    run: &crate::db::TaskRunRecord,
    artifact_count: usize,
    passed_assertions: usize,
) -> f64 {
    let mut score = 80.0;
    if run.business_complete {
        score += 8.0;
    }
    if run.execution_complete {
        score += 4.0;
    }
    if run.planner_complete {
        score += 3.0;
    }
    score += (artifact_count.min(6) as f64) * 1.2;
    score += (passed_assertions.min(6) as f64) * 0.8;
    score.min(99.0)
}

fn parse_boolish(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "ok" | "passed"
    )
}

fn sanitize_task_run_logs(logs: Vec<String>) -> Vec<String> {
    logs.into_iter()
        .map(|line| sanitize_candidate_text(&line))
        .filter(|line| !line.trim().is_empty())
        .collect()
}

fn sanitize_candidate_text(input: &str) -> String {
    let mut value = input.to_string();
    if let Ok(re) = Regex::new(r#"(?i)[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}"#) {
        value = re
            .replace_all(&value, "launch-eval@example.com")
            .to_string();
    }
    if let Ok(re) = Regex::new(r#"https?://[^\s"\\]+notion\.so[^\s"\\]*"#) {
        value = re
            .replace_all(&value, "https://www.notion.so/launch-eval-page")
            .to_string();
    }
    if let Ok(re) = Regex::new(r#"(?i)\b(page_id|message_id|doc_id|note_id|recipient)=([^\s|]+)"#) {
        value = re.replace_all(&value, "$1=launch_eval_id").to_string();
    }
    if let Ok(re) = Regex::new(r#"(?i)\b(run_scope_[a-z0-9_]+)\b"#) {
        value = re.replace_all(&value, "RUN_SCOPE_LAUNCH_EVAL").to_string();
    }
    value
}

async fn run_scenario(scenario: &LaunchEvalScenario) -> LaunchEvalCaseResult {
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

async fn evaluate_chat_case(
    id: &str,
    kind: &str,
    description: &Option<String>,
    request: &LaunchEvalChatInput,
    expect: &LaunchEvalChatExpectation,
) -> LaunchEvalCaseResult {
    let response =
        crate::api_server::process_chat_request(default_state(), request.as_request()).await;
    let errors = evaluate_chat_expectation(expect, &response);
    LaunchEvalCaseResult {
        id: id.to_string(),
        kind: kind.to_string(),
        description: description.clone(),
        passed: errors.is_empty(),
        errors,
        notes: vec![format!(
            "scope={}",
            request
                .memory_scope()
                .unwrap_or_else(|| "global".to_string())
        )],
        command: response.command.clone(),
        response_preview: Some(truncate_preview(&response.response, 220)),
        readiness: None,
        admission: None,
    }
}

fn evaluate_request_memory_policy_case(
    id: &str,
    description: &Option<String>,
    seed: &RequestMemorySeed,
    request: &LaunchEvalChatInput,
    mode: &RequestMemoryPolicyMode,
    feedback: Option<&str>,
    suppress: bool,
    expect_cached: bool,
    expect_command: Option<&str>,
    expect_response_contains: &[String],
) -> LaunchEvalCaseResult {
    let memory_scope = request.memory_scope();
    let memory_scope_ref = memory_scope.as_deref();
    let mut errors = Vec::new();
    let mut notes = vec![format!(
        "scope={}",
        memory_scope.clone().unwrap_or_else(|| "global".to_string())
    )];

    if let Err(error) = seed_request_memory(seed, memory_scope_ref) {
        return LaunchEvalCaseResult {
            id: id.to_string(),
            kind: "request_memory_policy".to_string(),
            description: description.clone(),
            passed: false,
            errors: vec![error.to_string()],
            notes,
            command: None,
            response_preview: None,
            readiness: None,
            admission: None,
        };
    }

    if let Some(sentiment) = feedback {
        notes.push(format!("feedback={}", sentiment));
        if let Err(error) = crate::db::record_request_memory_feedback_scoped(
            memory_scope_ref,
            &seed.request_text,
            &seed.response_text,
            sentiment,
        ) {
            errors.push(format!(
                "failed to apply request memory feedback: {}",
                error
            ));
        }
    }
    if suppress {
        notes.push("suppressed=true".to_string());
        if let Err(error) = crate::db::suppress_request_memory_scoped(
            memory_scope_ref,
            &seed.request_text,
            Some("launch eval policy suppression"),
        ) {
            errors.push(format!("failed to suppress request memory: {}", error));
        }
    }

    let (command, response_preview) = match mode {
        RequestMemoryPolicyMode::Response => {
            let cached = crate::api_server::load_cached_request_response(
                memory_scope_ref,
                &request.message,
                &seed.command,
            );
            if cached.is_some() != expect_cached {
                errors.push(format!(
                    "expected cached response={}, got {}",
                    expect_cached,
                    cached.is_some()
                ));
            }
            if let Some(response) = cached.as_ref() {
                for needle in expect_response_contains {
                    if !response.contains(needle) {
                        errors.push(format!("cached response missing '{}'", needle));
                    }
                }
            }
            notes.push("mode=response".to_string());
            (
                Some(seed.command.clone()),
                cached.map(|value| truncate_preview(&value, 220)),
            )
        }
        RequestMemoryPolicyMode::Intent => {
            let cached =
                crate::api_server::load_cached_request_intent(memory_scope_ref, &request.message);
            if cached.is_some() != expect_cached {
                errors.push(format!(
                    "expected cached intent={}, got {}",
                    expect_cached,
                    cached.is_some()
                ));
            }
            if let Some((intent, confidence)) = cached.as_ref() {
                if let Some(expected) = expect_command {
                    if intent["command"].as_str() != Some(expected) {
                        errors.push(format!(
                            "expected cached intent command {:?}, got {:?}",
                            expected,
                            intent["command"].as_str()
                        ));
                    }
                }
                notes.push(format!("mode=intent confidence={:.2}", confidence));
                (
                    intent["command"].as_str().map(|value| value.to_string()),
                    Some(truncate_preview(&intent.to_string(), 220)),
                )
            } else {
                notes.push("mode=intent".to_string());
                (None, None)
            }
        }
    };

    LaunchEvalCaseResult {
        id: id.to_string(),
        kind: "request_memory_policy".to_string(),
        description: description.clone(),
        passed: errors.is_empty(),
        errors,
        notes,
        command,
        response_preview,
        readiness: None,
        admission: None,
    }
}

fn evaluate_execution_memory_policy_case(
    id: &str,
    description: &Option<String>,
    seed: &ExecutionMemorySeed,
    request: &LaunchEvalChatInput,
    lookup_params: Option<&Value>,
    feedback: Option<&str>,
    suppress: bool,
    expect_cached: bool,
    expect_response_contains: &[String],
) -> LaunchEvalCaseResult {
    let memory_scope = request.memory_scope();
    let memory_scope_ref = memory_scope.as_deref();
    let mut errors = Vec::new();
    let mut notes = vec![format!(
        "scope={}",
        memory_scope.clone().unwrap_or_else(|| "global".to_string())
    )];

    if let Err(error) = seed_execution_memory(seed, memory_scope_ref) {
        return LaunchEvalCaseResult {
            id: id.to_string(),
            kind: "execution_memory_policy".to_string(),
            description: description.clone(),
            passed: false,
            errors: vec![error.to_string()],
            notes,
            command: Some(seed.command.clone()),
            response_preview: None,
            readiness: None,
            admission: None,
        };
    }

    let params_key = infer_execution_params_key(&seed.command, &seed.params);
    if let Some(sentiment) = feedback {
        notes.push(format!("feedback={}", sentiment));
        match &params_key {
            Ok(params_key) => {
                if let Err(error) = crate::db::record_execution_memory_feedback_scoped(
                    memory_scope_ref,
                    &seed.command,
                    params_key,
                    &seed.response_text,
                    sentiment,
                ) {
                    errors.push(format!(
                        "failed to apply execution memory feedback: {}",
                        error
                    ));
                }
            }
            Err(error) => errors.push(error.to_string()),
        }
    }
    if suppress {
        notes.push("suppressed=true".to_string());
        match &params_key {
            Ok(params_key) => {
                if let Err(error) = crate::db::suppress_execution_memory_scoped(
                    memory_scope_ref,
                    &seed.command,
                    params_key,
                    Some("launch eval policy suppression"),
                ) {
                    errors.push(format!("failed to suppress execution memory: {}", error));
                }
            }
            Err(error) => errors.push(error.to_string()),
        }
    }

    let params = lookup_params
        .cloned()
        .unwrap_or_else(|| seed.params.clone());
    let intent = json!({
        "command": seed.command,
        "params": params,
        "confidence": seed.confidence,
    });
    let cached = crate::api_server::load_cached_execution_response(
        memory_scope_ref,
        &request.message,
        &seed.command,
        &intent,
    );
    if cached.is_some() != expect_cached {
        errors.push(format!(
            "expected cached execution response={}, got {}",
            expect_cached,
            cached.is_some()
        ));
    }
    if let Some(response) = cached.as_ref() {
        for needle in expect_response_contains {
            if !response.contains(needle) {
                errors.push(format!("cached execution response missing '{}'", needle));
            }
        }
    }

    notes.push(format!(
        "lookup_params={}",
        truncate_preview(&intent["params"].to_string(), 120)
    ));

    LaunchEvalCaseResult {
        id: id.to_string(),
        kind: "execution_memory_policy".to_string(),
        description: description.clone(),
        passed: errors.is_empty(),
        errors,
        notes,
        command: Some(seed.command.clone()),
        response_preview: cached.map(|value| truncate_preview(&value, 220)),
        readiness: None,
        admission: None,
    }
}

fn evaluate_business_contract_case(
    id: &str,
    description: &Option<String>,
    plan: &LaunchEvalBusinessPlan,
    logs: &[String],
    expect_ok: bool,
    expect_detail_contains: &[String],
    expect_assertions: &[LaunchEvalAssertionExpectation],
) -> LaunchEvalCaseResult {
    let plan = crate::nl_automation::Plan {
        plan_id: format!("launch-eval-{}", id),
        intent: plan.intent.clone(),
        slots: plan.slots.clone(),
        steps: plan
            .descriptions
            .iter()
            .enumerate()
            .map(|(idx, description)| crate::nl_automation::PlanStep {
                step_id: format!("step-{}", idx + 1),
                step_type: crate::nl_automation::StepType::Extract,
                description: description.clone(),
                data: json!({}),
            })
            .collect(),
    };
    let evaluation = crate::api_server::evaluate_business_evidence_with_assertions(&plan, logs);
    let mut errors = Vec::new();
    if evaluation.ok != expect_ok {
        errors.push(format!(
            "expected business evidence ok={}, got {}",
            expect_ok, evaluation.ok
        ));
    }
    for needle in expect_detail_contains {
        if !evaluation.detail.contains(needle) {
            errors.push(format!("business detail missing '{}'", needle));
        }
    }
    for expected in expect_assertions {
        let assertion = evaluation
            .assertions
            .iter()
            .find(|assertion| assertion.key == expected.key);
        match assertion {
            Some(assertion) if assertion.passed == expected.passed => {}
            Some(assertion) => errors.push(format!(
                "expected assertion {} passed={}, got {}",
                expected.key, expected.passed, assertion.passed
            )),
            None => errors.push(format!("missing assertion {}", expected.key)),
        }
    }

    LaunchEvalCaseResult {
        id: id.to_string(),
        kind: "business_contract".to_string(),
        description: description.clone(),
        passed: errors.is_empty(),
        errors,
        notes: evaluation
            .assertions
            .iter()
            .filter(|assertion| !assertion.passed)
            .take(4)
            .map(|assertion| format!("{}={}", assertion.key, assertion.actual))
            .collect(),
        command: None,
        response_preview: Some(truncate_preview(&evaluation.detail, 220)),
        readiness: None,
        admission: None,
    }
}

fn evaluate_recommendation_case(
    id: &str,
    description: &Option<String>,
    stage: &RecommendationGateStage,
    proposal: &AutomationProposal,
    expect_ready: bool,
    expect_reasons_contains: &[String],
) -> LaunchEvalCaseResult {
    match stage {
        RecommendationGateStage::AutoQueue => {
            let readiness =
                crate::recommendation_policy::evaluate_auto_recommendation_readiness(proposal);
            let existing =
                crate::db::get_recommendations_with_filter(Some("all")).unwrap_or_default();
            let admission =
                crate::recommendation_policy::admit_auto_recommendation(proposal, &existing);
            let mut errors = Vec::new();
            if readiness.ready != expect_ready {
                errors.push(format!(
                    "expected readiness={}, got {}",
                    expect_ready, readiness.ready
                ));
            }
            if admission.accepted != expect_ready {
                errors.push(format!(
                    "expected admission={}, got {}",
                    expect_ready, admission.accepted
                ));
            }
            for needle in expect_reasons_contains {
                let found = readiness
                    .reasons
                    .iter()
                    .any(|reason| reason.contains(needle))
                    || admission
                        .reasons
                        .iter()
                        .any(|reason| reason.contains(needle));
                if !found {
                    errors.push(format!("missing expected reason fragment '{}'", needle));
                }
            }

            LaunchEvalCaseResult {
                id: id.to_string(),
                kind: "recommendation_gate".to_string(),
                description: description.clone(),
                passed: errors.is_empty(),
                errors,
                notes: vec!["stage=auto_queue".to_string()],
                command: None,
                response_preview: None,
                readiness: Some(LaunchEvalReadinessReport::from(readiness)),
                admission: Some(LaunchEvalAdmissionReport::from(admission)),
            }
        }
        RecommendationGateStage::Approval => {
            let unique_proposal = uniquify_proposal(proposal, id);
            let inserted = crate::db::insert_recommendation(&unique_proposal)
                .context("failed to insert recommendation");
            let mut errors = Vec::new();
            if let Err(error) = inserted {
                errors.push(error.to_string());
            }

            let recommendation = crate::db::get_recommendations_with_filter(Some("all"))
                .unwrap_or_default()
                .into_iter()
                .find(|rec| rec.title == unique_proposal.title);

            let readiness = if let Some(rec) = recommendation.as_ref() {
                Some(crate::recommendation_policy::evaluate_recommendation_approval_readiness(rec))
            } else {
                errors.push("could not reload inserted recommendation".to_string());
                None
            };

            if let Some(readiness) = readiness.as_ref() {
                if readiness.ready != expect_ready {
                    errors.push(format!(
                        "expected readiness={}, got {}",
                        expect_ready, readiness.ready
                    ));
                }
                for needle in expect_reasons_contains {
                    if !readiness
                        .reasons
                        .iter()
                        .any(|reason| reason.contains(needle))
                    {
                        errors.push(format!("missing expected reason fragment '{}'", needle));
                    }
                }
            }

            LaunchEvalCaseResult {
                id: id.to_string(),
                kind: "recommendation_gate".to_string(),
                description: description.clone(),
                passed: errors.is_empty(),
                errors,
                notes: vec!["stage=approval".to_string()],
                command: None,
                response_preview: None,
                readiness: readiness.map(LaunchEvalReadinessReport::from),
                admission: None,
            }
        }
    }
}

fn seed_request_memory(seed: &RequestMemorySeed, memory_scope: Option<&str>) -> Result<()> {
    let intent = json!({
        "command": seed.command,
        "params": seed.params,
        "confidence": seed.confidence,
    });
    let response_mode = seed
        .response_mode
        .clone()
        .unwrap_or_else(|| infer_request_memory_response_mode(&seed.command).to_string());
    crate::db::upsert_request_memory_scoped(
        memory_scope,
        &seed.request_text,
        Some(&intent),
        Some(&seed.response_text),
        &response_mode,
        &seed.source,
        seed.confidence,
    )
    .context("failed to seed request memory")
}

fn seed_request_memory_with_scope(seed: &ScopedRequestMemorySeed) -> Result<()> {
    let intent = json!({
        "command": seed.command,
        "params": seed.params,
        "confidence": seed.confidence,
    });
    let response_mode = seed
        .response_mode
        .clone()
        .unwrap_or_else(|| infer_request_memory_response_mode(&seed.command).to_string());
    let scope = seed.scope.memory_scope();
    crate::db::upsert_request_memory_scoped(
        scope.as_deref(),
        &seed.request_text,
        Some(&intent),
        Some(&seed.response_text),
        &response_mode,
        &seed.source,
        seed.confidence,
    )
    .context("failed to seed scoped request memory")
}

fn seed_execution_memory(seed: &ExecutionMemorySeed, memory_scope: Option<&str>) -> Result<()> {
    let params_key = infer_execution_params_key(&seed.command, &seed.params)?;
    let tool_path = seed
        .tool_path
        .clone()
        .unwrap_or_else(|| infer_execution_tool_path(&seed.command).to_string());
    let request_signature = crate::request_memory::build_request_signature(&seed.original_request);
    let signature = if request_signature.trim().is_empty() {
        None
    } else {
        Some(request_signature.as_str())
    };
    crate::db::upsert_execution_memory_scoped(
        memory_scope,
        &seed.command,
        &params_key,
        Some(&seed.params),
        signature,
        &seed.response_text,
        seed.ttl_seconds
            .unwrap_or_else(|| infer_execution_ttl(&seed.command)),
        &seed.source,
        &tool_path,
    )
    .context("failed to seed execution memory")
}

fn infer_request_memory_response_mode(command: &str) -> &'static str {
    match command {
        "help" | "help_local" | "greeting_local" => "reusable_response",
        "calendar_today" | "calendar_week" | "gmail_list" => "ttl_response_signature",
        "system_status" => "ttl_response_exact",
        _ => "intent_only",
    }
}

fn infer_execution_tool_path(command: &str) -> &'static str {
    match command {
        "calendar_today" => "integrations.calendar.list_today",
        "calendar_week" => "integrations.calendar.list_week",
        "gmail_list" => "integrations.gmail.list_messages",
        _ => "launch.eval",
    }
}

fn infer_execution_ttl(command: &str) -> i64 {
    match command {
        "gmail_list" => 20,
        "calendar_today" => 30,
        "calendar_week" => 120,
        "system_status" => 10,
        _ => 60,
    }
}

fn infer_execution_params_key(command: &str, params: &Value) -> Result<String> {
    match command {
        "gmail_list" => {
            let count = params
                .get("count")
                .and_then(|value| value.as_u64())
                .or_else(|| {
                    params
                        .get("count")
                        .and_then(|value| value.as_str())
                        .and_then(|value| value.parse::<u64>().ok())
                })
                .unwrap_or(5)
                .clamp(1, 20);
            Ok(format!("count={}", count))
        }
        "calendar_today" | "calendar_week" => Ok("default".to_string()),
        _ => Err(anyhow!(
            "unsupported execution memory command '{}'",
            command
        )),
    }
}

fn build_memory_scope(
    channel: Option<&str>,
    chat_type: Option<&str>,
    sender: Option<&str>,
) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(channel) = normalize_scope_part(channel) {
        parts.push(format!("channel_{}", channel));
    }
    if let Some(chat_type) = normalize_scope_part(chat_type) {
        parts.push(format!("type_{}", chat_type));
    }
    if let Some(sender) = normalize_scope_part(sender) {
        parts.push(format!("sender_{}", sender));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("__"))
    }
}

fn normalize_scope_part(value: Option<&str>) -> Option<String> {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LaunchEvalScenarioSnippet {
    scenarios: Vec<LaunchEvalScenario>,
}

fn render_scenario_yaml(scenario: &LaunchEvalScenario) -> String {
    serde_yaml::to_string(&LaunchEvalScenarioSnippet {
        scenarios: vec![scenario.clone()],
    })
    .unwrap_or_else(|_| "scenarios: []\n".to_string())
}

fn scenario_id(scenario: &LaunchEvalScenario) -> String {
    match scenario {
        LaunchEvalScenario::Chat { id, .. }
        | LaunchEvalScenario::RequestMemoryReuse { id, .. }
        | LaunchEvalScenario::ExecutionMemoryReuse { id, .. }
        | LaunchEvalScenario::RequestMemoryPolicy { id, .. }
        | LaunchEvalScenario::ExecutionMemoryPolicy { id, .. }
        | LaunchEvalScenario::BusinessContract { id, .. }
        | LaunchEvalScenario::MemoryScopeIsolation { id, .. }
        | LaunchEvalScenario::RecommendationGate { id, .. } => id.clone(),
    }
}

fn candidate_id(prefix: &str, command: &str, message: &str) -> String {
    format!("{}-{}-{}", prefix, slug_id(command), slug_id(message))
}

fn slug_id(value: &str) -> String {
    let normalized = value
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("-");
    if normalized.is_empty() {
        "candidate".to_string()
    } else {
        normalized
    }
}

fn summarize_message(value: &str) -> String {
    let trimmed = value.trim();
    let summary = trimmed.chars().take(48).collect::<String>();
    if trimmed.chars().count() > 48 {
        format!("{}...", summary)
    } else {
        summary
    }
}

fn normalize_request(value: &str) -> String {
    crate::request_memory::normalize_request_text(value)
}

fn request_memory_params(intent_json: Option<&str>) -> Value {
    let Some(raw) = intent_json.map(str::trim).filter(|value| !value.is_empty()) else {
        return json!({});
    };
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|value| value.get("params").cloned())
        .filter(|value| value.is_object())
        .unwrap_or_else(default_json_object)
}

fn parse_json_object(raw: Option<&str>) -> Value {
    raw.and_then(|value| serde_json::from_str::<Value>(value).ok())
        .filter(|value| value.is_object())
        .unwrap_or_else(default_json_object)
}

fn candidate_scope(memory_scope: &str, command: &str) -> LaunchEvalScope {
    let mut scope = parse_memory_scope(memory_scope);
    if scope.channel.is_none() {
        scope.channel = Some("web".to_string());
    }
    if scope.chat_type.is_none() {
        scope.chat_type = Some("direct".to_string());
    }
    if scope.sender.is_none() {
        scope.sender = Some(format!("launch-eval-{}", slug_id(command)));
    }
    scope
}

fn parse_memory_scope(memory_scope: &str) -> LaunchEvalScope {
    let mut scope = LaunchEvalScope {
        channel: None,
        chat_type: None,
        sender: None,
    };

    for part in memory_scope.split("__") {
        if let Some(value) = part.strip_prefix("channel_") {
            if !value.trim().is_empty() {
                scope.channel = Some(value.to_string());
            }
            continue;
        }
        if let Some(value) = part.strip_prefix("type_") {
            if !value.trim().is_empty() {
                scope.chat_type = Some(value.to_string());
            }
            continue;
        }
        if let Some(value) = part.strip_prefix("sender_") {
            if !value.trim().is_empty() {
                scope.sender = Some(value.to_string());
            }
        }
    }

    scope
}

fn request_memory_score(record: &crate::db::RequestMemoryRecord) -> f64 {
    let source_bonus = if record.source.contains("deterministic") {
        12.0
    } else if record.source.contains("api.chat") {
        6.0
    } else {
        0.0
    };
    record.confidence * 100.0
        + (record.use_count as f64 * 2.0)
        + (record.positive_feedback_count as f64 * 6.0)
        - (record.negative_feedback_count as f64 * 8.0)
        + source_bonus
}

fn execution_memory_confidence(record: &crate::db::ExecutionMemoryRecord) -> f64 {
    if record.request_signature.is_some() {
        0.95
    } else {
        0.9
    }
}

fn execution_memory_score(record: &crate::db::ExecutionMemoryRecord) -> f64 {
    80.0 + (record.use_count as f64 * 2.5) + (record.positive_feedback_count as f64 * 6.0)
        - (record.negative_feedback_count as f64 * 8.0)
        + record.freshness_ttl_seconds.min(120) as f64 / 20.0
}

fn launch_event_score(event: &crate::db::LaunchOpsEventRecord) -> f64 {
    let route_bonus = match event.route_kind.as_str() {
        "ai_digest_auto_fallback" | "ai_digest_explicit" => 28.0,
        "llm" => 20.0,
        "deterministic" => 16.0,
        "local_command" => 10.0,
        _ => 8.0,
    };
    let confidence = event.confidence.unwrap_or(0.75) * 100.0;
    let freshness_bonus = if event.freshness_bypassed { 6.0 } else { 0.0 };
    confidence + route_bonus + freshness_bonus
}

fn execution_request_message(record: &crate::db::ExecutionMemoryRecord) -> Option<String> {
    if let Some(signature) = record.request_signature.as_deref() {
        let count = parse_execution_count(record.params_json.as_deref()).unwrap_or(5);
        if signature.contains("today") && signature.contains("calendar") {
            return Some("오늘 일정 보여줘".to_string());
        }
        if signature.contains("this_week") && signature.contains("calendar") {
            return Some("이번 주 일정 보여줘".to_string());
        }
        if signature.contains("recent") && signature.contains("email") {
            return Some(format!("최근 이메일 {}개 보여줘", count));
        }
    }

    match record.intent_command.as_str() {
        "calendar_today" => Some("오늘 일정 보여줘".to_string()),
        "calendar_week" => Some("이번 주 일정 보여줘".to_string()),
        "gmail_list" => Some(format!(
            "최근 이메일 {}개 보여줘",
            parse_execution_count(record.params_json.as_deref()).unwrap_or(5)
        )),
        _ => None,
    }
}

fn parse_execution_count(params_json: Option<&str>) -> Option<u64> {
    parse_json_object(params_json)
        .get("count")
        .and_then(|value| value.as_u64())
        .or_else(|| {
            parse_json_object(params_json)
                .get("count")
                .and_then(|value| value.as_str())
                .and_then(|value| value.parse::<u64>().ok())
        })
}

fn default_state() -> AppState {
    AppState {
        llm_client: None,
        current_goal: Arc::new(Mutex::new(None)),
    }
}

fn evaluate_chat_expectation(
    expect: &LaunchEvalChatExpectation,
    response: &ChatResponse,
) -> Vec<String> {
    let mut errors = Vec::new();
    if response.command != expect.command {
        errors.push(format!(
            "expected command {:?}, got {:?}",
            expect.command, response.command
        ));
    }
    for needle in &expect.response_contains {
        if !response.response.contains(needle) {
            errors.push(format!("response missing '{}'", needle));
        }
    }
    if !expect.response_contains_any.is_empty()
        && !expect
            .response_contains_any
            .iter()
            .any(|needle| response.response.contains(needle))
    {
        errors.push(format!(
            "response missing any of {:?}",
            expect.response_contains_any
        ));
    }
    for needle in &expect.response_not_contains {
        if response.response.contains(needle) {
            errors.push(format!("response unexpectedly contains '{}'", needle));
        }
    }
    errors
}

fn uniquify_proposal(proposal: &AutomationProposal, case_id: &str) -> AutomationProposal {
    let suffix = format!("-{}", case_id);
    let mut proposal = proposal.clone();
    proposal.title = format!("{} {}", proposal.title, suffix);
    proposal.trigger = format!("{}{}", proposal.trigger, suffix);
    proposal.pattern_id = proposal
        .pattern_id
        .as_ref()
        .map(|pattern_id| format!("{}{}", pattern_id, suffix));
    proposal
}

fn truncate_preview(input: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for ch in input.chars().take(max_chars) {
        out.push(ch);
    }
    if input.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

fn render_markdown_report(report: &LaunchEvalReport) -> String {
    let mut out = String::new();
    out.push_str("# Allvia Launch Eval\n\n");
    out.push_str(&format!(
        "- Generated at: `{}`\n- Passed: `{}/{}`\n- DB: `{}`\n",
        report.generated_at, report.passed, report.total, report.db_path
    ));
    if let Some(config_path) = &report.config_path {
        out.push_str(&format!("- Config: `{}`\n", config_path));
    }
    out.push_str("\n| ID | Kind | Status | Notes |\n| --- | --- | --- | --- |\n");
    for result in &report.results {
        let status = if result.passed { "PASS" } else { "FAIL" };
        let notes = if result.errors.is_empty() {
            result.notes.join("; ")
        } else {
            result.errors.join("; ")
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            result.id,
            result.kind,
            status,
            notes.replace('|', "/")
        ));
    }
    out.push('\n');
    for result in &report.results {
        out.push_str(&format!("## {}\n\n", result.id));
        if let Some(description) = &result.description {
            out.push_str(&format!("{}\n\n", description));
        }
        out.push_str(&format!("- Kind: `{}`\n", result.kind));
        out.push_str(&format!("- Passed: `{}`\n", result.passed));
        if let Some(command) = &result.command {
            out.push_str(&format!("- Command: `{}`\n", command));
        }
        if let Some(response) = &result.response_preview {
            out.push_str(&format!("- Response preview: `{}`\n", response));
        }
        if !result.notes.is_empty() {
            out.push_str(&format!("- Notes: {}\n", result.notes.join("; ")));
        }
        if !result.errors.is_empty() {
            out.push_str(&format!("- Errors: {}\n", result.errors.join("; ")));
        }
        if let Some(readiness) = &result.readiness {
            out.push_str(&format!(
                "- Readiness: ready=`{}`, business_score=`{:.2}`, confidence=`{:.2}`\n",
                readiness.ready, readiness.business_score, readiness.confidence
            ));
            if !readiness.reasons.is_empty() {
                out.push_str(&format!(
                    "- Readiness reasons: {}\n",
                    readiness.reasons.join("; ")
                ));
            }
        }
        if let Some(admission) = &result.admission {
            out.push_str(&format!(
                "- Admission: accepted=`{}`, priority_score=`{:.2}`, pending=`{}`/`{}`\n",
                admission.accepted,
                admission.priority_score,
                admission.pending_same_category,
                admission.pending_limit
            ));
            if !admission.reasons.is_empty() {
                out.push_str(&format!(
                    "- Admission reasons: {}\n",
                    admission.reasons.join("; ")
                ));
            }
        }
        out.push('\n');
    }
    out
}

impl From<crate::recommendation_policy::RecommendationApprovalReadinessDecision>
    for LaunchEvalReadinessReport
{
    fn from(value: crate::recommendation_policy::RecommendationApprovalReadinessDecision) -> Self {
        Self {
            ready: value.ready,
            reasons: value.reasons,
            category: value.category,
            business_score: value.business_score,
            confidence: value.confidence,
            occurrences: value.occurrences,
            distinct_days: value.distinct_days,
        }
    }
}

impl From<crate::recommendation_policy::AutoRecommendationAdmissionDecision>
    for LaunchEvalAdmissionReport
{
    fn from(value: crate::recommendation_policy::AutoRecommendationAdmissionDecision) -> Self {
        Self {
            accepted: value.accepted,
            reasons: value.reasons,
            priority_score: value.priority_score,
            pending_same_category: value.pending_same_category,
            pending_limit: value.pending_limit,
        }
    }
}

struct ScopedEnvGuard {
    previous: Vec<(String, Option<String>)>,
}

impl ScopedEnvGuard {
    fn set(vars: Vec<(&str, Option<String>)>) -> Self {
        let mut previous = Vec::new();
        for (key, next) in vars {
            previous.push((key.to_string(), std::env::var(key).ok()));
            match next {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
        Self { previous }
    }
}

impl Drop for ScopedEnvGuard {
    fn drop(&mut self) {
        for (key, previous) in self.previous.iter().rev() {
            match previous {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

struct AiDigestMockGuard {
    _env: ScopedEnvGuard,
    server: tokio::task::JoinHandle<()>,
}

impl AiDigestMockGuard {
    async fn start(mock: &AiDigestMock) -> Result<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .context("failed to bind ai_digest mock listener")?;
        let addr = listener
            .local_addr()
            .context("failed to read ai_digest mock addr")?;
        let payload = json!({
            "status": mock.status,
            "notion_url": mock.notion_url,
            "top_headlines_text": mock.top_headlines_text,
        });
        let app = Router::new().route(
            "/",
            post({
                let payload = payload.clone();
                move || {
                    let payload = payload.clone();
                    async move { Json(payload) }
                }
            }),
        );
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let env = ScopedEnvGuard::set(vec![
            (
                "STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL",
                Some(format!("http://{}/", addr)),
            ),
            (
                "ALLVIA_AI_DIGEST_AUTO_ROUTE_CHANNELS",
                Some("web,telegram".to_string()),
            ),
        ]);
        Ok(Self { _env: env, server })
    }
}

impl Drop for AiDigestMockGuard {
    fn drop(&mut self) {
        self.server.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn generate_launch_eval_candidates_uses_real_records_without_leaking_response_text() {
        crate::db::init().ok();
        crate::db::clear_request_memory_for_tests();
        crate::db::clear_execution_memory_for_tests();
        crate::db::clear_launch_ops_events_for_tests();

        let request_intent = json!({
            "command": "calendar_today",
            "params": {},
            "confidence": 0.94
        });
        crate::db::upsert_request_memory_scoped(
            Some("channel_web__type_direct__sender_ops"),
            "오늘 일정 보여줘",
            Some(&request_intent),
            Some("PRIVATE_REQUEST_RESPONSE"),
            "ttl_response_signature",
            "api.chat.deterministic",
            0.94,
        )
        .expect("seed request memory");

        crate::db::upsert_execution_memory_scoped(
            Some("channel_web__type_direct__sender_ops"),
            "gmail_list",
            "count=5",
            Some(&json!({ "count": 5 })),
            Some("recent email 5 read"),
            "PRIVATE_EXECUTION_RESPONSE",
            20,
            "api.chat.execution",
            "integrations.gmail.list_messages",
        )
        .expect("seed execution memory");

        crate::db::record_launch_ops_event(
            Some("web"),
            Some("channel_web__type_direct__sender_ops"),
            "AI 뉴스 5개 요약해서 노션에 정리해줘",
            "ai_digest_auto_fallback",
            Some("ai_digest_program"),
            "success",
            Some(0.88),
            false,
            false,
            false,
            false,
            false,
            true,
            true,
            false,
            Some("launch eval candidate"),
        )
        .expect("seed launch ops event");

        let candidates = generate_launch_eval_candidates(10);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.scenario_kind == "request_memory_reuse"));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.scenario_kind == "execution_memory_reuse"));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.command.as_deref() == Some("ai_digest_program")));
        assert!(candidates
            .iter()
            .all(|candidate| !candidate.yaml.contains("PRIVATE_REQUEST_RESPONSE")));
        assert!(candidates
            .iter()
            .all(|candidate| !candidate.yaml.contains("PRIVATE_EXECUTION_RESPONSE")));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.yaml.contains("LAUNCH_EVAL_REQUEST_CACHE_")));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.yaml.contains("LAUNCH_EVAL_EXECUTION_CACHE_")));
    }

    #[test]
    #[serial]
    fn generate_launch_eval_candidates_includes_task_run_business_contract_candidates() {
        crate::db::init().ok();
        crate::db::clear_request_memory_for_tests();
        crate::db::clear_execution_memory_for_tests();
        crate::db::clear_launch_ops_events_for_tests();

        let run_id = format!("launch-prod-{}", uuid::Uuid::new_v4());
        crate::db::create_task_run(
            &run_id,
            "surf_goal",
            "qed4950@gmail.com으로 요약 메일을 보내고 Notion에 저장한 뒤 텔레그램으로 알려줘",
            "running",
        )
        .expect("create task run");
        let details = serde_json::to_string(&vec![
            "Summary: qed4950@gmail.com으로 메일 전송, 노션 저장, 텔레그램 전송 완료".to_string(),
            "EVIDENCE target=mail|event=send|status=sent_confirmed|recipient=qed4950@gmail.com|body_len=42".to_string(),
            "EVIDENCE target=notion|event=write|status=confirmed|page_id=abc123".to_string(),
            "EVIDENCE target=telegram|event=send|status=sent|message_id=msg_123".to_string(),
            "notion: https://www.notion.so/private-page".to_string(),
        ])
        .expect("serialize details");
        crate::db::update_task_run_outcome(
            &run_id,
            true,
            true,
            true,
            "business_completed",
            Some("qed4950@gmail.com에게 보냈고 노션/텔레그램 완료"),
            Some(&details),
        )
        .expect("update task run");
        crate::db::upsert_task_run_artifact(
            &run_id,
            "artifact_assertion",
            "artifact.mail_sent_confirmed",
            "true",
            Some("{\"passed\":true}"),
        )
        .expect("artifact assertion");
        crate::db::upsert_task_run_artifact(
            &run_id,
            "artifact_assertion",
            "artifact.notion_write_confirmed",
            "true",
            Some("{\"passed\":true}"),
        )
        .expect("artifact assertion");

        let temp_dir = std::env::temp_dir().join(format!(
            "allvia-launch-eval-task-run-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");
        std::fs::create_dir_all(temp_dir.join("configs")).expect("create config dir");
        std::fs::write(
            temp_dir.join("configs").join("launch_eval.yaml"),
            "report_dir: reports/launch_eval\nscenarios: []\n",
        )
        .expect("write minimal config");

        let candidates = generate_launch_eval_candidates_with_filter(
            &temp_dir,
            20,
            LaunchEvalCandidateProvenanceFilter::Real,
        );
        let candidate = candidates
            .iter()
            .find(|candidate| {
                candidate.source_kind == "task_run"
                    && candidate
                        .rationale
                        .iter()
                        .any(|item| item == &format!("run_id={}", run_id))
            })
            .expect("task run candidate");
        assert_eq!(candidate.provenance, "real");
        assert!(!candidate.yaml.contains("qed4950@gmail.com"));
        assert!(!candidate.yaml.contains("private-page"));
        assert!(candidate.yaml.contains("launch-eval@example.com"));
        assert!(candidate
            .yaml
            .contains("https://www.notion.so/launch-eval-page"));
    }

    #[test]
    fn load_launch_eval_config_merges_include_paths() {
        let temp_dir = std::env::temp_dir().join(format!(
            "allvia-launch-eval-config-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");
        let base_path = temp_dir.join("base.yaml");
        let include_path = temp_dir.join("included.yaml");

        std::fs::write(
            &include_path,
            r#"
report_dir: reports/launch_eval
scenarios:
  - id: included-chat
    kind: chat
    request:
      message: "도움말"
    expect:
      command: "help_local"
"#,
        )
        .expect("write include");
        std::fs::write(
            &base_path,
            r#"
report_dir: reports/launch_eval
include_paths:
  - included.yaml
scenarios:
  - id: base-chat
    kind: chat
    request:
      message: "도움말"
    expect:
      command: "help_local"
"#,
        )
        .expect("write base");

        let config = load_launch_eval_config(&base_path).expect("load config");
        let ids = config.scenarios.iter().map(scenario_id).collect::<Vec<_>>();
        assert!(ids.contains(&"base-chat".to_string()));
        assert!(ids.contains(&"included-chat".to_string()));
    }

    #[test]
    fn synthetic_candidates_are_loaded_from_curated_config_without_generated_include() {
        let temp_dir = std::env::temp_dir().join(format!(
            "allvia-launch-eval-synthetic-{}",
            uuid::Uuid::new_v4()
        ));
        let configs_dir = temp_dir.join("configs");
        std::fs::create_dir_all(&configs_dir).expect("create configs dir");
        std::fs::write(
            configs_dir.join("launch_eval.generated.yaml"),
            r#"
report_dir: reports/launch_eval
scenarios:
  - id: generated-real
    kind: chat
    request:
      message: "generated real scenario"
    expect:
      command: "help_local"
"#,
        )
        .expect("write generated include");
        std::fs::write(
            configs_dir.join("launch_eval.yaml"),
            r#"
report_dir: reports/launch_eval
include_paths:
  - launch_eval.generated.yaml
scenarios:
  - id: curated-dogfood
    kind: chat
    request:
      message: "dogfood curated scenario"
    expect:
      command: "help_local"
"#,
        )
        .expect("write curated config");

        let candidates = generate_launch_eval_candidates_with_filter(
            &temp_dir,
            8,
            LaunchEvalCandidateProvenanceFilter::Synthetic,
        );
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].provenance, "synthetic");
        assert_eq!(candidates[0].id, "curated-dogfood");
    }

    #[test]
    #[serial]
    fn write_launch_eval_candidate_snapshot_writes_generated_yaml() {
        crate::db::init().ok();
        crate::db::clear_request_memory_for_tests();
        crate::db::clear_execution_memory_for_tests();
        crate::db::clear_launch_ops_events_for_tests();

        let request_intent = json!({
            "command": "calendar_today",
            "params": {},
            "confidence": 0.94
        });
        crate::db::upsert_request_memory_scoped(
            Some("channel_web__type_direct__sender_ops"),
            "오늘 일정 보여줘",
            Some(&request_intent),
            Some("PRIVATE_REQUEST_RESPONSE"),
            "ttl_response_signature",
            "api.chat.deterministic",
            0.94,
        )
        .expect("seed request memory");

        let temp_dir = std::env::temp_dir().join(format!(
            "allvia-launch-eval-snapshot-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");
        let snapshot = write_launch_eval_candidate_snapshot(&temp_dir, Some("generated.yaml"), 5)
            .expect("write snapshot");

        let raw = std::fs::read_to_string(&snapshot.output_path).expect("read snapshot");
        assert!(raw.contains("scenarios:"));
        assert!(raw.contains("request_memory_reuse"));
        assert!(!raw.contains("PRIVATE_REQUEST_RESPONSE"));
        assert!(snapshot.scenario_count >= 1);
    }

    #[test]
    fn synthetic_snapshot_uses_dogfood_default_path() {
        let temp_dir = std::env::temp_dir().join(format!(
            "allvia-launch-eval-dogfood-snapshot-{}",
            uuid::Uuid::new_v4()
        ));
        let configs_dir = temp_dir.join("configs");
        std::fs::create_dir_all(&configs_dir).expect("create configs dir");
        std::fs::write(
            configs_dir.join("launch_eval.yaml"),
            r#"
report_dir: reports/launch_eval
scenarios:
  - id: dogfood-chat
    kind: chat
    request:
      message: "도그푸드 일정 보여줘"
    expect:
      command: "calendar_today"
"#,
        )
        .expect("write curated config");

        let snapshot = write_launch_eval_candidate_snapshot_with_filter(
            &temp_dir,
            None,
            5,
            LaunchEvalCandidateProvenanceFilter::Synthetic,
        )
        .expect("write synthetic snapshot");

        assert!(snapshot
            .output_path
            .ends_with("configs/launch_eval.dogfood.generated.yaml"));
        assert_eq!(snapshot.provenance_filter, "synthetic");
        assert_eq!(snapshot.scenario_count, 1);
    }

    #[test]
    fn read_launch_eval_candidate_snapshot_info_reports_existing_file() {
        let temp_dir = std::env::temp_dir().join(format!(
            "allvia-launch-eval-snapshot-info-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");
        let snapshot_path = temp_dir.join("launch_eval.generated.yaml");
        std::fs::write(
            &snapshot_path,
            r#"report_dir: reports/launch_eval
scenarios:
  - kind: chat
    id: snapshot-info
    request:
      message: "도움말"
    expect:
      command: "help_local"
"#,
        )
        .expect("write snapshot info fixture");

        let info =
            read_launch_eval_candidate_snapshot_info(&temp_dir, Some("launch_eval.generated.yaml"));
        assert!(info.exists);
        assert_eq!(info.scenario_count, 1);
        assert_eq!(info.scenario_ids, vec!["snapshot-info".to_string()]);
        assert!(info.updated_at.is_some());
    }

    #[test]
    #[serial]
    fn request_memory_policy_case_detects_negative_feedback_block() {
        crate::db::init().ok();
        crate::db::clear_request_memory_for_tests();

        let scenario = LaunchEvalScenario::RequestMemoryPolicy {
            id: "request-feedback-block".to_string(),
            description: Some("negative feedback should block request cache reuse".to_string()),
            seed: RequestMemorySeed {
                request_text: "오늘 일정 보여줘".to_string(),
                command: "calendar_today".to_string(),
                params: json!({}),
                response_text: "📅 TODAY_CACHE".to_string(),
                confidence: 0.95,
                source: "launch.eval".to_string(),
                response_mode: None,
            },
            request: LaunchEvalChatInput {
                message: "오늘 일정 보여줘".to_string(),
                channel: Some("web".to_string()),
                chat_type: Some("direct".to_string()),
                sender: Some("launch-eval-request-policy".to_string()),
                mentioned: None,
            },
            mode: RequestMemoryPolicyMode::Response,
            feedback: Some("negative".to_string()),
            suppress: false,
            expect_cached: false,
            expect_command: None,
            expect_response_contains: Vec::new(),
        };

        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let result = runtime.block_on(run_scenario(&scenario));
        assert!(result.passed, "{:?}", result.errors);
    }

    #[test]
    #[serial]
    fn execution_memory_policy_case_detects_fresh_bypass() {
        crate::db::init().ok();
        crate::db::clear_execution_memory_for_tests();

        let scenario = LaunchEvalScenario::ExecutionMemoryPolicy {
            id: "execution-fresh-bypass".to_string(),
            description: Some("fresh wording should bypass execution cache".to_string()),
            seed: ExecutionMemorySeed {
                original_request: "최근 이메일 5개 보여줘".to_string(),
                command: "gmail_list".to_string(),
                params: json!({ "count": 5 }),
                response_text: "📧 EXEC_CACHE".to_string(),
                confidence: 0.95,
                source: "launch.eval".to_string(),
                ttl_seconds: Some(20),
                tool_path: None,
            },
            request: LaunchEvalChatInput {
                message: "지금 최근 이메일 5개 새로고침".to_string(),
                channel: Some("web".to_string()),
                chat_type: Some("direct".to_string()),
                sender: Some("launch-eval-exec-policy".to_string()),
                mentioned: None,
            },
            lookup_params: None,
            feedback: None,
            suppress: false,
            expect_cached: false,
            expect_response_contains: Vec::new(),
        };

        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let result = runtime.block_on(run_scenario(&scenario));
        assert!(result.passed, "{:?}", result.errors);
    }

    #[test]
    fn business_contract_case_detects_missing_write_artifacts() {
        let scenario = LaunchEvalScenario::BusinessContract {
            id: "business-contract-missing-artifacts".to_string(),
            description: Some("missing mail/notion/telegram artifacts should fail".to_string()),
            plan: LaunchEvalBusinessPlan {
                intent: crate::nl_automation::IntentType::GenericTask,
                descriptions: vec![
                    "Mail에서 qed4950@gmail.com으로 결과를 보내세요.".to_string(),
                    "Notion에 요약을 작성하세요.".to_string(),
                    "텔레그램으로 전송하세요.".to_string(),
                ],
                slots: std::collections::HashMap::new(),
            },
            logs: vec!["Summary: requested integrations done".to_string()],
            expect_ok: false,
            expect_detail_contains: vec![
                "contract_missing_mail_send_confirmation".to_string(),
                "contract_missing_notion_write_confirmation".to_string(),
                "contract_missing_telegram_send_confirmation".to_string(),
            ],
            expect_assertions: vec![
                LaunchEvalAssertionExpectation {
                    key: "artifact.mail_sent_confirmed".to_string(),
                    passed: false,
                },
                LaunchEvalAssertionExpectation {
                    key: "artifact.notion_write_confirmed".to_string(),
                    passed: false,
                },
            ],
        };

        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let result = runtime.block_on(run_scenario(&scenario));
        assert!(result.passed, "{:?}", result.errors);
    }
}
