use crate::launch_eval::*;

pub(super) fn synthetic_candidate_from_scenario(
    scenario: &LaunchEvalScenario,
) -> Option<LaunchEvalCandidate> {
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
