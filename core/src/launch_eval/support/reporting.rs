use super::super::*;
use crate::recommendation::AutomationProposal;
use anyhow::{anyhow, Context, Result};
use axum::{routing::post, Json, Router};
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn load_launch_eval_config(config_path: &Path) -> Result<LaunchEvalConfig> {
    let mut visited = HashSet::new();
    load_launch_eval_config_recursive(config_path, &mut visited)
}

pub(super) fn load_launch_eval_config_shallow(config_path: &Path) -> Result<LaunchEvalConfig> {
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

pub(super) fn resolve_relative_path(base: Option<&Path>, raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() {
        candidate
    } else {
        base.unwrap_or_else(|| Path::new(".")).join(candidate)
    }
}

pub(crate) fn resolve_candidate_snapshot_path(
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

pub(crate) fn scenario_from_candidate_yaml(raw: &str) -> Option<LaunchEvalScenario> {
    let snippet: LaunchEvalScenarioSnippet = serde_yaml::from_str(raw).ok()?;
    snippet.scenarios.into_iter().next()
}

pub(crate) fn uniquify_proposal(
    proposal: &AutomationProposal,
    case_id: &str,
) -> AutomationProposal {
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

pub(crate) fn truncate_preview(input: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for ch in input.chars().take(max_chars) {
        out.push(ch);
    }
    if input.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

pub(crate) fn render_markdown_report(report: &LaunchEvalReport) -> String {
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

pub(crate) struct ScopedEnvGuard {
    previous: Vec<(String, Option<String>)>,
}

impl ScopedEnvGuard {
    pub(crate) fn set(vars: Vec<(&str, Option<String>)>) -> Self {
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

pub(crate) struct AiDigestMockGuard {
    _env: ScopedEnvGuard,
    server: tokio::task::JoinHandle<()>,
}

impl AiDigestMockGuard {
    pub(crate) async fn start(mock: &AiDigestMock) -> Result<Self> {
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
