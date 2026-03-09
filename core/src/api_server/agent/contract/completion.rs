use serde::Serialize;

#[derive(Serialize, Clone)]
pub(crate) struct AgentCompletionScore {
    pub score: u8,
    pub label: String,
    pub pass: bool,
    pub reasons: Vec<String>,
}

#[derive(Serialize, Clone)]
pub(crate) struct AgentStageDodCheck {
    pub stage: String,
    pub key: String,
    pub expected: String,
    pub actual: String,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

pub(crate) fn completion_score_pass_threshold() -> u8 {
    std::env::var("STEER_COMPLETION_SCORE_PASS")
        .ok()
        .and_then(|v| v.trim().parse::<u8>().ok())
        .map(|v| v.min(100))
        .unwrap_or(75)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_completion_score(
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
