use super::super::evidence::detect_artifact_evidence_assertions;
use super::summary::is_meaningful_summary;
use super::validation::validate_intent_business_contract;

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

pub(crate) fn evaluate_business_evidence(
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
