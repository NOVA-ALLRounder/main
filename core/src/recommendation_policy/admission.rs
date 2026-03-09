use super::*;

fn normalized_review_status(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn recommendation_is_snoozed(rec: &crate::db::Recommendation) -> bool {
    let Some(snoozed_until) = rec
        .snoozed_until
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return false;
    };
    chrono::DateTime::parse_from_rfc3339(snoozed_until)
        .map(|ts| ts.with_timezone(&chrono::Utc) > chrono::Utc::now())
        .unwrap_or(false)
}

pub fn recommendation_requires_approval_readiness(rec: &crate::db::Recommendation) -> bool {
    recommendation_auto_generated(rec)
}

pub fn evaluate_recommendation_approval_readiness(
    rec: &crate::db::Recommendation,
) -> RecommendationApprovalReadinessDecision {
    let category = effective_recommendation_category(rec);
    let business_score = effective_recommendation_business_score(rec);
    let confidence = rec.confidence.clamp(0.0, 1.0);
    let occurrences = extract_occurrences_from_recommendation(rec);
    let distinct_days = extract_distinct_days_from_recommendation(rec);

    if !recommendation_requires_approval_readiness(rec) {
        return RecommendationApprovalReadinessDecision {
            ready: true,
            reasons: Vec::new(),
            category,
            business_score,
            confidence,
            occurrences,
            distinct_days,
        };
    }

    let mut reasons = Vec::new();
    if !category.eq_ignore_ascii_case(CATEGORY_WORK) {
        reasons.push("Only work auto recommendations can be approved in MVP.".to_string());
    }

    let min_business_score = min_auto_approval_business_score();
    if business_score < min_business_score {
        reasons.push(format!(
            "Business score {:.0}% is below the approval threshold {:.0}%.",
            business_score * 100.0,
            min_business_score * 100.0
        ));
    }

    let min_confidence = min_auto_approval_confidence();
    if confidence < min_confidence {
        reasons.push(format!(
            "Confidence {:.0}% is below the approval threshold {:.0}%.",
            confidence * 100.0,
            min_confidence * 100.0
        ));
    }

    let min_occurrences = min_auto_approval_occurrences();
    match occurrences {
        Some(value) if value >= min_occurrences => {}
        Some(value) => reasons.push(format!(
            "Pattern frequency is too weak for approval ({} < {} occurrences).",
            value, min_occurrences
        )),
        None => reasons.push("Missing pattern frequency evidence.".to_string()),
    }

    let min_distinct_days = min_auto_approval_distinct_days();
    match distinct_days {
        Some(value) if value >= min_distinct_days => {}
        Some(value) => reasons.push(format!(
            "Pattern span is too short for approval ({} < {} distinct days).",
            value, min_distinct_days
        )),
        None => reasons.push("Missing distinct-day evidence.".to_string()),
    }

    let has_strong_work_signals =
        evidence_has_marker(&rec.evidence, "policy.reason=strong_work_signals=");
    let has_work_context_reason =
        evidence_has_marker(&rec.evidence, "policy.reason=pattern.work_context=strong")
            || evidence_has_marker(
                &rec.evidence,
                "policy.reason=pattern.work_context=promoted_support_only",
            );
    let context_score = extract_policy_context_score(rec).unwrap_or(0.0);
    if !has_strong_work_signals
        && !has_work_context_reason
        && context_score < min_pattern_work_context_score()
    {
        reasons.push("Missing strong work evidence or work-hour context.".to_string());
    }

    RecommendationApprovalReadinessDecision {
        ready: reasons.is_empty(),
        reasons,
        category,
        business_score,
        confidence,
        occurrences,
        distinct_days,
    }
}

pub fn evaluate_auto_recommendation_readiness(
    proposal: &AutomationProposal,
) -> RecommendationApprovalReadinessDecision {
    let category = proposal.category.trim().to_lowercase();
    let business_score = proposal.business_score.clamp(0.0, 1.0);
    let confidence = proposal.confidence.clamp(0.0, 1.0);
    let occurrences = extract_occurrences_from_texts(&proposal.evidence, &proposal.summary);
    let distinct_days = extract_distinct_days_from_texts(&proposal.evidence, &proposal.summary);

    if !proposal_auto_generated(proposal) {
        return RecommendationApprovalReadinessDecision {
            ready: true,
            reasons: Vec::new(),
            category,
            business_score,
            confidence,
            occurrences,
            distinct_days,
        };
    }

    let mut reasons = Vec::new();
    if !category.eq_ignore_ascii_case(CATEGORY_WORK) {
        reasons.push("Only work auto recommendations can enter the MVP queue.".to_string());
    }

    let min_business_score = min_auto_approval_business_score();
    if business_score < min_business_score {
        reasons.push(format!(
            "Business score {:.0}% is below the queue threshold {:.0}%.",
            business_score * 100.0,
            min_business_score * 100.0
        ));
    }

    let min_confidence = min_auto_approval_confidence();
    if confidence < min_confidence {
        reasons.push(format!(
            "Confidence {:.0}% is below the queue threshold {:.0}%.",
            confidence * 100.0,
            min_confidence * 100.0
        ));
    }

    let min_occurrences = min_auto_approval_occurrences();
    match occurrences {
        Some(value) if value >= min_occurrences => {}
        Some(value) => reasons.push(format!(
            "Pattern frequency is too weak for queueing ({} < {} occurrences).",
            value, min_occurrences
        )),
        None => reasons.push("Missing pattern frequency evidence.".to_string()),
    }

    let min_distinct_days = min_auto_approval_distinct_days();
    match distinct_days {
        Some(value) if value >= min_distinct_days => {}
        Some(value) => reasons.push(format!(
            "Pattern span is too short for queueing ({} < {} distinct days).",
            value, min_distinct_days
        )),
        None => reasons.push("Missing distinct-day evidence.".to_string()),
    }

    let has_strong_work_signals =
        evidence_has_marker(&proposal.evidence, "policy.reason=strong_work_signals=");
    let has_work_context_reason = evidence_has_marker(
        &proposal.evidence,
        "policy.reason=pattern.work_context=strong",
    ) || evidence_has_marker(
        &proposal.evidence,
        "policy.reason=pattern.work_context=promoted_support_only",
    );
    let context_score =
        extract_policy_context_score_from_evidence(&proposal.evidence).unwrap_or(0.0);
    if !has_strong_work_signals
        && !has_work_context_reason
        && context_score < min_pattern_work_context_score()
    {
        reasons.push("Missing strong work evidence or work-hour context.".to_string());
    }

    RecommendationApprovalReadinessDecision {
        ready: reasons.is_empty(),
        reasons,
        category,
        business_score,
        confidence,
        occurrences,
        distinct_days,
    }
}

pub fn admit_auto_recommendation(
    proposal: &AutomationProposal,
    existing_recommendations: &[crate::db::Recommendation],
) -> AutoRecommendationAdmissionDecision {
    let category = proposal.category.trim().to_lowercase();
    let pending_limit = auto_pending_limit_for_category(&category);
    let priority_score = proposal_priority_score(proposal);
    let proposal_fingerprint = proposal.fingerprint();
    let title_key = normalize_title_key(&proposal.title);
    let same_category = existing_recommendations
        .iter()
        .filter(|rec| effective_recommendation_category(rec).eq_ignore_ascii_case(&category))
        .collect::<Vec<_>>();
    let pending_same_category = same_category
        .iter()
        .copied()
        .filter(|rec| normalized_review_status(&rec.status) == "pending")
        .filter(|rec| !recommendation_is_snoozed(rec))
        .filter(|rec| rec.pattern_id.is_some())
        .collect::<Vec<_>>();
    let snoozed_same_category = same_category
        .iter()
        .copied()
        .filter(|rec| normalized_review_status(&rec.status) == "pending")
        .filter(|rec| recommendation_is_snoozed(rec))
        .collect::<Vec<_>>();
    let historical_same_category = same_category
        .iter()
        .copied()
        .filter(|rec| {
            let status = normalized_review_status(&rec.status);
            status == "approved" || status == "rejected"
        })
        .collect::<Vec<_>>();

    let mut reasons = Vec::new();
    let readiness = evaluate_auto_recommendation_readiness(proposal);
    if !readiness.ready {
        for reason in readiness.reasons {
            reasons.push(format!("launch_gate.{}", reason));
        }
    }

    if let Some(pattern_id) = proposal.pattern_id.as_deref() {
        if pending_same_category
            .iter()
            .any(|rec| rec.pattern_id.as_deref() == Some(pattern_id))
        {
            reasons.push(format!("auto_queue.duplicate_pattern_id={}", pattern_id));
        }
        if snoozed_same_category
            .iter()
            .any(|rec| rec.pattern_id.as_deref() == Some(pattern_id))
        {
            reasons.push(format!("auto_queue.snoozed_pattern_id={}", pattern_id));
        }
        for rec in &historical_same_category {
            if rec.pattern_id.as_deref() == Some(pattern_id) {
                reasons.push(format!(
                    "history.{}_pattern_id={}",
                    normalized_review_status(&rec.status),
                    pattern_id
                ));
                break;
            }
        }
    }

    if !title_key.is_empty()
        && pending_same_category
            .iter()
            .any(|rec| normalize_title_key(&rec.title) == title_key)
    {
        reasons.push("auto_queue.duplicate_title_pending".to_string());
    }
    if !title_key.is_empty()
        && snoozed_same_category
            .iter()
            .any(|rec| normalize_title_key(&rec.title) == title_key)
    {
        reasons.push("auto_queue.snoozed_title_pending".to_string());
    }

    if !title_key.is_empty() {
        for rec in &historical_same_category {
            if normalize_title_key(&rec.title) == title_key {
                reasons.push(format!(
                    "history.{}_title_pending",
                    normalized_review_status(&rec.status)
                ));
                break;
            }
        }
    }

    if pending_same_category.iter().any(|rec| {
        format!(
            "{}::{}",
            rec.title.trim().to_lowercase(),
            rec.trigger.trim().to_lowercase()
        ) == proposal_fingerprint
    }) {
        reasons.push("auto_queue.duplicate_fingerprint_pending".to_string());
    }
    if snoozed_same_category.iter().any(|rec| {
        format!(
            "{}::{}",
            rec.title.trim().to_lowercase(),
            rec.trigger.trim().to_lowercase()
        ) == proposal_fingerprint
    }) {
        reasons.push("auto_queue.snoozed_fingerprint_pending".to_string());
    }

    for rec in &historical_same_category {
        let rec_fingerprint = format!(
            "{}::{}",
            rec.title.trim().to_lowercase(),
            rec.trigger.trim().to_lowercase()
        );
        if rec_fingerprint == proposal_fingerprint {
            reasons.push(format!(
                "history.{}_fingerprint_pending",
                normalized_review_status(&rec.status)
            ));
            break;
        }
    }

    if pending_same_category.len() >= pending_limit {
        reasons.push(format!(
            "auto_queue.pending_limit_reached={}/{}",
            pending_same_category.len(),
            pending_limit
        ));
    }

    AutoRecommendationAdmissionDecision {
        accepted: reasons.is_empty(),
        reasons,
        priority_score,
        pending_same_category: pending_same_category.len(),
        pending_limit,
    }
}

pub fn matches_category_filter(category: &str, filter: Option<&str>) -> bool {
    let Some(filter) = filter.map(str::trim).filter(|s| !s.is_empty()) else {
        return true;
    };
    if filter.eq_ignore_ascii_case("all") {
        return true;
    }
    category.eq_ignore_ascii_case(filter)
}

pub fn recommendation_category_browsing_enabled() -> bool {
    std::env::var("ALLVIA_ENABLE_RECOMMENDATION_CATEGORY_BROWSING")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

pub fn default_list_category_filter() -> Option<String> {
    let raw = std::env::var("ALLVIA_RECOMMENDATION_LIST_DEFAULT_CATEGORY")
        .ok()
        .unwrap_or_else(|| CATEGORY_WORK.to_string());
    let normalized = raw.trim().to_lowercase();
    if normalized.is_empty() || normalized == "all" {
        None
    } else {
        Some(normalized)
    }
}

pub fn normalize_list_category_filter(requested: Option<&str>) -> Option<String> {
    if !recommendation_category_browsing_enabled() {
        return Some(CATEGORY_WORK.to_string());
    }

    requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase())
        .or_else(default_list_category_filter)
}
