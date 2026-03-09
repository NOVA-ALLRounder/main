use super::super::support::truncate_preview;
use super::super::support::uniquify_proposal;
use super::super::*;
use crate::recommendation::AutomationProposal;

pub(super) fn evaluate_business_contract_case(
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
                data: serde_json::json!({}),
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

pub(super) fn evaluate_recommendation_case(
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
