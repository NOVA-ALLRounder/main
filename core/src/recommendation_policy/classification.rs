use super::*;

fn classify_from_texts(
    texts: &[String],
    pattern: Option<&DetectedPattern>,
) -> RecommendationPolicyDecision {
    let tokens = collect_tokens(texts);
    let strong_work_hits = count_hits(&tokens, STRONG_WORK_SIGNALS);
    let support_work_hits = count_hits(&tokens, SUPPORT_WORK_SIGNALS);
    let personal_hits = count_hits(&tokens, PERSONAL_SIGNALS);
    let system_hits = count_hits(&tokens, SYSTEM_SIGNALS);

    let has_work_file_combo = ["docx", "pptx", "xlsx", "pdf", "py", "rs", "ts", "tsx"]
        .iter()
        .filter(|ext| tokens.contains(**ext))
        .count()
        >= 2;

    let mut reasons = Vec::new();
    if !strong_work_hits.is_empty() {
        push_unique(
            &mut reasons,
            format!("strong_work_signals={}", strong_work_hits.join(",")),
        );
    }
    if !support_work_hits.is_empty() {
        push_unique(
            &mut reasons,
            format!("support_work_signals={}", support_work_hits.join(",")),
        );
    }
    if !personal_hits.is_empty() {
        push_unique(
            &mut reasons,
            format!("personal_signals={}", personal_hits.join(",")),
        );
    }
    if !system_hits.is_empty() {
        push_unique(
            &mut reasons,
            format!("system_signals={}", system_hits.join(",")),
        );
    }
    if has_work_file_combo {
        push_unique(&mut reasons, "work_file_combo=true".to_string());
    }

    let pattern_context = pattern_context_profile(pattern);
    if let Some((weekday_ratio, work_hour_ratio, context_score)) = pattern_context {
        push_unique(
            &mut reasons,
            format!("pattern.weekday_ratio={:.2}", weekday_ratio),
        );
        push_unique(
            &mut reasons,
            format!("pattern.work_hour_ratio={:.2}", work_hour_ratio),
        );
        push_unique(
            &mut reasons,
            format!("pattern.context_score={:.2}", context_score),
        );
    }

    let has_strong_work_evidence = strong_work_hits.len() >= 2
        || (strong_work_hits.len() >= 1 && !support_work_hits.is_empty())
        || has_work_file_combo;
    let pattern_context_score = pattern_context.map(|(_, _, score)| score);
    let pattern_supports_work =
        pattern_context_score.is_some_and(|score| score >= min_pattern_work_context_score());
    let pattern_looks_off_hours =
        pattern_context_score.is_some_and(|score| score <= off_hours_pattern_context_score());

    if pattern_supports_work {
        if strong_work_hits.is_empty() && support_work_hits.len() >= 2 {
            push_unique(
                &mut reasons,
                "pattern.work_context=promoted_support_only".to_string(),
            );
        } else {
            push_unique(&mut reasons, "pattern.work_context=strong".to_string());
        }
    } else if pattern_looks_off_hours {
        push_unique(&mut reasons, "pattern.work_context=weak".to_string());
    }

    let timing_adjustment =
        if pattern_supports_work && strong_work_hits.is_empty() && support_work_hits.len() >= 2 {
            0.25
        } else if pattern_supports_work {
            0.10
        } else if pattern_looks_off_hours && !has_strong_work_evidence {
            -0.08
        } else {
            0.0
        };

    let business_score = clamp_score(
        0.12 + (strong_work_hits.len() as f64 * 0.20)
            + (support_work_hits.len() as f64 * 0.09)
            + if has_work_file_combo { 0.14 } else { 0.0 }
            + timing_adjustment
            - (personal_hits.len() as f64 * 0.23),
    );

    let category = if !personal_hits.is_empty()
        && strong_work_hits.is_empty()
        && support_work_hits.is_empty()
    {
        CATEGORY_PERSONAL.to_string()
    } else if has_strong_work_evidence
        || (pattern_supports_work && (!strong_work_hits.is_empty() || support_work_hits.len() >= 2))
    {
        CATEGORY_WORK.to_string()
    } else if !system_hits.is_empty() && strong_work_hits.is_empty() {
        CATEGORY_SYSTEM.to_string()
    } else if !personal_hits.is_empty() && strong_work_hits.is_empty() {
        CATEGORY_PERSONAL.to_string()
    } else {
        CATEGORY_UNKNOWN.to_string()
    };

    let accepted = match env_default_category().as_str() {
        CATEGORY_WORK => category == CATEGORY_WORK && business_score >= min_work_business_score(),
        CATEGORY_PERSONAL => category == CATEGORY_PERSONAL,
        CATEGORY_SYSTEM => category == CATEGORY_SYSTEM,
        _ => business_score >= min_work_business_score(),
    };

    RecommendationPolicyDecision {
        category,
        business_score,
        accepted,
        reasons,
    }
}

pub(super) fn texts_from_pattern_and_proposal(
    proposal: &AutomationProposal,
    pattern: Option<&DetectedPattern>,
) -> Vec<String> {
    let mut texts = vec![
        proposal.title.clone(),
        proposal.summary.clone(),
        proposal.trigger.clone(),
        proposal.n8n_prompt.clone(),
        proposal.actions.join(" "),
        proposal.evidence.join(" "),
    ];

    if let Some(pattern) = pattern {
        texts.push(pattern.description.clone());
        texts.extend(pattern.sample_events.clone());
    }

    texts
        .into_iter()
        .filter(|text| !text.trim().is_empty())
        .collect()
}

pub(super) fn texts_from_recommendation(rec: &crate::db::Recommendation) -> Vec<String> {
    vec![
        rec.title.clone(),
        rec.summary.clone(),
        rec.trigger.clone(),
        rec.n8n_prompt.clone(),
        rec.actions.join(" "),
        rec.evidence.join(" "),
    ]
    .into_iter()
    .filter(|text| !text.trim().is_empty())
    .collect()
}

fn append_policy_evidence(
    proposal: &mut AutomationProposal,
    decision: &RecommendationPolicyDecision,
) {
    push_unique(
        &mut proposal.evidence,
        format!("policy.category={}", decision.category),
    );
    push_unique(
        &mut proposal.evidence,
        format!("policy.business_score={:.2}", decision.business_score),
    );
    push_unique(
        &mut proposal.evidence,
        format!("policy.accepted={}", decision.accepted),
    );
    for reason in &decision.reasons {
        push_unique(&mut proposal.evidence, format!("policy.reason={}", reason));
    }
}

pub fn apply_mvp_policy(
    proposal: &mut AutomationProposal,
    pattern: Option<&DetectedPattern>,
) -> RecommendationPolicyDecision {
    let decision =
        classify_from_texts(&texts_from_pattern_and_proposal(proposal, pattern), pattern);
    proposal.category = decision.category.clone();
    proposal.business_score = decision.business_score;
    append_policy_evidence(proposal, &decision);
    decision
}

pub fn classify_recommendation_record(
    rec: &crate::db::Recommendation,
) -> RecommendationPolicyDecision {
    classify_from_texts(&texts_from_recommendation(rec), None)
}
