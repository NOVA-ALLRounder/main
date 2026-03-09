use crate::launch_eval::*;
use regex::Regex;

pub(crate) fn task_run_business_contract_candidate(
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
