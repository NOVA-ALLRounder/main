use super::super::*;
use super::reporting::{load_launch_eval_config_shallow, resolve_relative_path};
use std::collections::HashSet;
use std::path::Path;

mod events;
mod memory;
mod synthetic;
mod task_runs;

pub(crate) use task_runs::task_run_business_contract_candidate;

pub(crate) fn generate_launch_eval_candidates_from_sources(
    request_records: &[crate::db::RequestMemoryRecord],
    execution_records: &[crate::db::ExecutionMemoryRecord],
    launch_events: &[crate::db::LaunchOpsEventRecord],
    limit: usize,
) -> Vec<LaunchEvalCandidate> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    for record in request_records {
        let Some(candidate) = memory::request_memory_candidate(record) else {
            continue;
        };
        if seen.insert(dedupe_key(&candidate)) {
            candidates.push(candidate);
        }
    }

    for record in execution_records {
        let Some(candidate) = memory::execution_memory_candidate(record) else {
            continue;
        };
        if seen.insert(dedupe_key(&candidate)) {
            candidates.push(candidate);
        }
    }

    for event in launch_events {
        let Some(candidate) = events::launch_event_candidate(event) else {
            continue;
        };
        if seen.insert(dedupe_key(&candidate)) {
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

pub(crate) fn generate_synthetic_launch_eval_candidates_from_config(
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
        let Some(candidate) = synthetic::synthetic_candidate_from_scenario(scenario) else {
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

fn dedupe_key(candidate: &LaunchEvalCandidate) -> String {
    format!(
        "{}::{}::{}",
        candidate.scenario_kind,
        candidate.command.clone().unwrap_or_default(),
        normalize_request(&candidate.request_message)
    )
}
