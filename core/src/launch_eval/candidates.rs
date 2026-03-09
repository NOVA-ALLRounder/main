use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use super::support::{
    generate_launch_eval_candidates_from_sources,
    generate_synthetic_launch_eval_candidates_from_config, resolve_candidate_snapshot_path,
    scenario_from_candidate_yaml, task_run_business_contract_candidate,
};
use super::types::*;

pub fn generate_launch_eval_candidates(limit: usize) -> Vec<LaunchEvalCandidate> {
    let workdir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
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

pub(crate) fn render_scenario_yaml(scenario: &LaunchEvalScenario) -> String {
    serde_yaml::to_string(&LaunchEvalScenarioSnippet {
        scenarios: vec![scenario.clone()],
    })
    .unwrap_or_else(|_| "scenarios: []\n".to_string())
}

pub(crate) fn scenario_id(scenario: &LaunchEvalScenario) -> String {
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

pub(crate) fn candidate_id(prefix: &str, command: &str, message: &str) -> String {
    format!("{}-{}-{}", prefix, slug_id(command), slug_id(message))
}

pub(crate) fn slug_id(value: &str) -> String {
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

pub(crate) fn summarize_message(value: &str) -> String {
    let trimmed = value.trim();
    let summary = trimmed.chars().take(48).collect::<String>();
    if trimmed.chars().count() > 48 {
        format!("{}...", summary)
    } else {
        summary
    }
}

pub(crate) fn normalize_request(value: &str) -> String {
    crate::request_memory::normalize_request_text(value)
}

pub(crate) fn request_memory_params(intent_json: Option<&str>) -> Value {
    let Some(raw) = intent_json.map(str::trim).filter(|value| !value.is_empty()) else {
        return json!({});
    };
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|value| value.get("params").cloned())
        .filter(|value| value.is_object())
        .unwrap_or_else(default_json_object)
}

pub(crate) fn parse_json_object(raw: Option<&str>) -> Value {
    raw.and_then(|value| serde_json::from_str::<Value>(value).ok())
        .filter(|value| value.is_object())
        .unwrap_or_else(default_json_object)
}

pub(crate) fn candidate_scope(memory_scope: &str, command: &str) -> LaunchEvalScope {
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

pub(crate) fn parse_memory_scope(memory_scope: &str) -> LaunchEvalScope {
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

pub(crate) fn request_memory_score(record: &crate::db::RequestMemoryRecord) -> f64 {
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

pub(crate) fn execution_memory_confidence(record: &crate::db::ExecutionMemoryRecord) -> f64 {
    if record.request_signature.is_some() {
        0.95
    } else {
        0.9
    }
}

pub(crate) fn execution_memory_score(record: &crate::db::ExecutionMemoryRecord) -> f64 {
    80.0 + (record.use_count as f64 * 2.5) + (record.positive_feedback_count as f64 * 6.0)
        - (record.negative_feedback_count as f64 * 8.0)
        + record.freshness_ttl_seconds.min(120) as f64 / 20.0
}

pub(crate) fn launch_event_score(event: &crate::db::LaunchOpsEventRecord) -> f64 {
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

pub(crate) fn execution_request_message(
    record: &crate::db::ExecutionMemoryRecord,
) -> Option<String> {
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

pub(crate) fn parse_execution_count(params_json: Option<&str>) -> Option<u64> {
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
