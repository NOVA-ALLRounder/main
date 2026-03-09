use super::*;
use std::collections::HashSet;

fn ordered_target_mentions(text: &str) -> Vec<(usize, &'static str)> {
    let normalized = text.trim().to_lowercase();
    if normalized.is_empty() {
        return Vec::new();
    }

    let target_aliases = [
        (TARGET_NOTION, NOTION_TARGET_ALIASES),
        (TARGET_TELEGRAM, TELEGRAM_TARGET_ALIASES),
        (TARGET_EMAIL, EMAIL_TARGET_ALIASES),
        (TARGET_SLACK, SLACK_TARGET_ALIASES),
    ];

    let mut hits = Vec::new();
    for (target, aliases) in target_aliases {
        let position = aliases
            .iter()
            .filter_map(|alias| normalized.find(alias))
            .min();
        if let Some(pos) = position {
            hits.push((pos, target));
        }
    }
    hits.sort_by_key(|(pos, _)| *pos);
    hits
}

fn humanize_target(target: &str) -> &'static str {
    match target {
        TARGET_NOTION => "Notion",
        TARGET_TELEGRAM => "Telegram",
        TARGET_EMAIL => "Email",
        TARGET_SLACK => "Slack",
        _ => "Unknown",
    }
}

fn humanize_targets(targets: &[String]) -> String {
    targets
        .iter()
        .map(|target| humanize_target(target))
        .collect::<Vec<_>>()
        .join(", ")
}

fn recommendation_target_overlap(left: &[String], right: &[String]) -> Vec<String> {
    let right_lookup = right.iter().collect::<HashSet<_>>();
    left.iter()
        .filter(|target| right_lookup.contains(target))
        .cloned()
        .collect()
}

fn unique_recommendation_targets(rec: &crate::db::Recommendation) -> Vec<String> {
    let mut targets = Vec::new();
    for (_, target) in ordered_target_mentions(&texts_from_recommendation(rec).join(" ")) {
        push_unique(&mut targets, target.to_string());
    }
    targets
}

fn extract_feedback_targets(note: &str, sentiment: &str) -> (Vec<String>, Vec<String>) {
    let normalized = note.trim().to_lowercase();
    let ordered_mentions = ordered_target_mentions(note);
    let mut preferred = Vec::new();
    let mut avoided = Vec::new();
    let has_swap_marker = PREFERENCE_SWAP_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker));

    if has_swap_marker && ordered_mentions.len() >= 2 {
        push_unique(&mut avoided, ordered_mentions[0].1.to_string());
        push_unique(
            &mut preferred,
            ordered_mentions
                .last()
                .map(|(_, target)| (*target).to_string())
                .unwrap_or_default(),
        );
    }

    match sentiment {
        "positive" => {
            for (_, target) in &ordered_mentions {
                push_unique(&mut preferred, (*target).to_string());
            }
        }
        "negative" => {
            if avoided.is_empty() {
                for (_, target) in &ordered_mentions {
                    push_unique(&mut avoided, (*target).to_string());
                }
            }
        }
        "refine" => {
            if preferred.is_empty() {
                if let Some((_, target)) = ordered_mentions.last() {
                    push_unique(&mut preferred, (*target).to_string());
                }
            }
        }
        _ => {}
    }

    (preferred, avoided)
}

pub fn effective_recommendation_category(rec: &crate::db::Recommendation) -> String {
    if rec.category.trim().is_empty() || rec.category.eq_ignore_ascii_case(CATEGORY_UNKNOWN) {
        classify_recommendation_record(rec).category
    } else {
        rec.category.trim().to_lowercase()
    }
}

pub fn effective_recommendation_business_score(rec: &crate::db::Recommendation) -> f64 {
    if rec.business_score > 0.0 {
        rec.business_score.clamp(0.0, 1.0)
    } else {
        classify_recommendation_record(rec).business_score
    }
}

pub fn build_recommendation_preference_profile(
    existing_recommendations: &[crate::db::Recommendation],
) -> RecommendationPreferenceProfile {
    let mut preferred_scores: HashMap<String, i32> = HashMap::new();
    let mut avoided_scores: HashMap<String, i32> = HashMap::new();
    let mut reasons = Vec::new();

    for rec in existing_recommendations {
        if !effective_recommendation_category(rec).eq_ignore_ascii_case(CATEGORY_WORK) {
            continue;
        }
        let review_targets = unique_recommendation_targets(rec);
        let sentiment = rec
            .feedback_status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or_default()
            .to_ascii_lowercase();

        let Some(note) = rec
            .feedback_note
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            if review_targets.len() == 1 {
                let target = review_targets[0].clone();
                if rec.status.eq_ignore_ascii_case("approved") {
                    *preferred_scores.entry(target.clone()).or_insert(0) += 1;
                    push_unique(
                        &mut reasons,
                        format!(
                            "review.approved.prefer={}",
                            humanize_target(&target).to_lowercase()
                        ),
                    );
                } else if rec.status.eq_ignore_ascii_case("rejected") {
                    *avoided_scores.entry(target.clone()).or_insert(0) += 1;
                    push_unique(
                        &mut reasons,
                        format!(
                            "review.rejected.avoid={}",
                            humanize_target(&target).to_lowercase()
                        ),
                    );
                }
            }
            continue;
        };

        let (preferred_targets, avoided_targets) = extract_feedback_targets(note, &sentiment);
        for target in preferred_targets {
            *preferred_scores.entry(target.clone()).or_insert(0) += 2;
            push_unique(
                &mut reasons,
                format!(
                    "feedback.{}.prefer={}",
                    sentiment,
                    humanize_target(&target).to_lowercase()
                ),
            );
        }
        for target in avoided_targets {
            *avoided_scores.entry(target.clone()).or_insert(0) += 2;
            push_unique(
                &mut reasons,
                format!(
                    "feedback.{}.avoid={}",
                    sentiment,
                    humanize_target(&target).to_lowercase()
                ),
            );
        }
    }

    let mut preferred_targets = preferred_scores
        .into_iter()
        .filter_map(|(target, score)| {
            let opposing = avoided_scores.get(&target).copied().unwrap_or(0);
            if score > opposing {
                Some((score - opposing, target))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    preferred_targets
        .sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    let preferred_lookup = preferred_targets
        .iter()
        .map(|(score, target)| (target.clone(), *score))
        .collect::<HashMap<_, _>>();

    let mut avoided_targets = avoided_scores
        .into_iter()
        .filter_map(|(target, score)| {
            let supporting = preferred_lookup.get(&target).copied().unwrap_or(0);
            if score > supporting {
                Some((score - supporting, target))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    avoided_targets.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));

    RecommendationPreferenceProfile {
        preferred_targets: preferred_targets
            .into_iter()
            .map(|(_, target)| target)
            .collect(),
        avoided_targets: avoided_targets
            .into_iter()
            .map(|(_, target)| target)
            .collect(),
        reasons,
    }
}

pub fn apply_recommendation_preferences(
    proposal: &mut AutomationProposal,
    existing_recommendations: &[crate::db::Recommendation],
) -> RecommendationPreferenceProfile {
    let profile = build_recommendation_preference_profile(existing_recommendations);
    if profile.preferred_targets.is_empty() && profile.avoided_targets.is_empty() {
        return profile;
    }

    let proposal_mentions = ordered_target_mentions(&format!(
        "{} {} {} {}",
        proposal.title, proposal.summary, proposal.trigger, proposal.n8n_prompt
    ))
    .into_iter()
    .map(|(_, target)| target.to_string())
    .collect::<Vec<_>>();
    let preferred_matches =
        recommendation_target_overlap(&proposal_mentions, &profile.preferred_targets);
    let conflicting_targets =
        recommendation_target_overlap(&proposal_mentions, &profile.avoided_targets);

    if !profile.preferred_targets.is_empty() || !profile.avoided_targets.is_empty() {
        let mut hints = Vec::new();
        if !profile.preferred_targets.is_empty() {
            hints.push(format!(
                "Prefer {} as the destination or delivery channel unless the user explicitly asks otherwise.",
                humanize_targets(&profile.preferred_targets)
            ));
            push_unique(
                &mut proposal.evidence,
                format!("preference.prefer={}", profile.preferred_targets.join(",")),
            );
        }
        if !profile.avoided_targets.is_empty() {
            hints.push(format!(
                "Avoid {} unless the user explicitly requests it.",
                humanize_targets(&profile.avoided_targets)
            ));
            push_unique(
                &mut proposal.evidence,
                format!("preference.avoid={}", profile.avoided_targets.join(",")),
            );
        }
        for reason in &profile.reasons {
            push_unique(
                &mut proposal.evidence,
                format!("preference.reason={}", reason),
            );
        }

        if !conflicting_targets.is_empty() {
            push_unique(
                &mut proposal.evidence,
                format!("preference.conflict={}", conflicting_targets.join(",")),
            );
        }
        if !preferred_matches.is_empty() {
            push_unique(
                &mut proposal.evidence,
                format!("preference.match={}", preferred_matches.join(",")),
            );
        }

        let hint_block = format!(
            "\n\nUser workflow delivery preferences inferred from recent feedback:\n- {}",
            hints.join("\n- ")
        );
        if !proposal
            .n8n_prompt
            .contains("User workflow delivery preferences inferred")
        {
            proposal.n8n_prompt.push_str(&hint_block);
        }
    }

    if !conflicting_targets.is_empty() {
        let conflict_penalty = (0.08 * conflicting_targets.len() as f64).min(0.16);
        proposal.business_score = clamp_score(proposal.business_score - conflict_penalty);
        proposal.confidence = clamp_score(proposal.confidence - (conflict_penalty * 0.5));
        push_unique(
            &mut proposal.evidence,
            format!(
                "preference.business_score_adjusted={:.2}",
                proposal.business_score
            ),
        );
        push_unique(
            &mut proposal.evidence,
            format!("preference.confidence_adjusted={:.2}", proposal.confidence),
        );
    } else if !preferred_matches.is_empty() {
        let preferred_bonus = (0.04 * preferred_matches.len() as f64).min(0.08);
        proposal.business_score = clamp_score(proposal.business_score + preferred_bonus);
        proposal.confidence = clamp_score(proposal.confidence + (preferred_bonus * 0.25));
        push_unique(
            &mut proposal.evidence,
            format!(
                "preference.business_score_adjusted={:.2}",
                proposal.business_score
            ),
        );
        push_unique(
            &mut proposal.evidence,
            format!("preference.confidence_adjusted={:.2}", proposal.confidence),
        );
    }

    profile
}

pub fn recommendation_priority_score(confidence: f64, business_score: f64) -> f64 {
    clamp_score((business_score.clamp(0.0, 1.0) * 0.75) + (confidence.clamp(0.0, 1.0) * 0.25))
}

pub fn proposal_priority_score(proposal: &AutomationProposal) -> f64 {
    recommendation_priority_score(proposal.confidence, proposal.business_score)
}

pub fn recommendation_record_priority_score(rec: &crate::db::Recommendation) -> f64 {
    recommendation_priority_score(rec.confidence, effective_recommendation_business_score(rec))
}

pub fn recommendation_record_priority_score_with_profile(
    rec: &crate::db::Recommendation,
    profile: &RecommendationPreferenceProfile,
) -> f64 {
    let base_score = recommendation_record_priority_score(rec);
    if profile.preferred_targets.is_empty() && profile.avoided_targets.is_empty() {
        return base_score;
    }

    let rec_targets = unique_recommendation_targets(rec);
    let preferred_matches = recommendation_target_overlap(&rec_targets, &profile.preferred_targets);
    let avoided_matches = recommendation_target_overlap(&rec_targets, &profile.avoided_targets);

    if !avoided_matches.is_empty() {
        let penalty = (0.05 * avoided_matches.len() as f64).min(0.10);
        clamp_score(base_score - penalty)
    } else if !preferred_matches.is_empty() {
        let bonus = (0.03 * preferred_matches.len() as f64).min(0.06);
        clamp_score(base_score + bonus)
    } else {
        base_score
    }
}
