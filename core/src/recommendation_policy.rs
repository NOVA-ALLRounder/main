use crate::pattern_detector::DetectedPattern;
use crate::recommendation::AutomationProposal;
use std::collections::{HashMap, HashSet};

pub const CATEGORY_WORK: &str = "work";
pub const CATEGORY_PERSONAL: &str = "personal";
pub const CATEGORY_SYSTEM: &str = "system";
pub const CATEGORY_UNKNOWN: &str = "unknown";

const STRONG_WORK_SIGNALS: &[&str] = &[
    "slack",
    "teams",
    "gmail",
    "outlook",
    "calendar",
    "notion",
    "jira",
    "confluence",
    "github",
    "gitlab",
    "linear",
    "figma",
    "zoom",
    "meet",
    "invoice",
    "proposal",
    "meeting",
    "agenda",
    "report",
    "briefing",
    "approval",
    "client",
    "spec",
    "roadmap",
    "ticket",
    "sprint",
    "release",
    "deploy",
];

const SUPPORT_WORK_SIGNALS: &[&str] = &[
    "terminal",
    "vscode",
    "xcode",
    "cursor",
    "intellij",
    "excel",
    "word",
    "powerpoint",
    "docs",
    "sheets",
    "drive",
    "mail",
    "email",
    "todoist",
    "reminders",
    "docx",
    "pptx",
    "xlsx",
    "pdf",
    "python",
    "rust",
    "typescript",
    "meetingnote",
    "documentation",
];

const PERSONAL_SIGNALS: &[&str] = &[
    "youtube",
    "netflix",
    "spotify",
    "tiktok",
    "instagram",
    "facebook",
    "twitter",
    "reddit",
    "steam",
    "game",
    "games",
    "shopping",
    "coupon",
    "movie",
    "music",
    "entertainment",
    "kakao",
    "whatsapp",
];

const SYSTEM_SIGNALS: &[&str] = &[
    "download",
    "downloads",
    "cleanup",
    "archive",
    "backup",
    "folder",
    "filesystem",
    "screenshot",
    "file",
    "files",
];

const TARGET_NOTION: &str = "notion";
const TARGET_TELEGRAM: &str = "telegram";
const TARGET_EMAIL: &str = "email";
const TARGET_SLACK: &str = "slack";

const NOTION_TARGET_ALIASES: &[&str] = &["notion", "노션"];
const TELEGRAM_TARGET_ALIASES: &[&str] = &["telegram", "텔레그램"];
const EMAIL_TARGET_ALIASES: &[&str] = &["email", "mail", "gmail", "이메일", "메일", "지메일"];
const SLACK_TARGET_ALIASES: &[&str] = &["slack", "슬랙"];

const PREFERENCE_SWAP_MARKERS: &[&str] = &["말고", "대신", "instead of", "rather than"];

#[derive(Debug, Clone)]
pub struct RecommendationPolicyDecision {
    pub category: String,
    pub business_score: f64,
    pub accepted: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AutoRecommendationAdmissionDecision {
    pub accepted: bool,
    pub reasons: Vec<String>,
    pub priority_score: f64,
    pub pending_same_category: usize,
    pub pending_limit: usize,
}

#[derive(Debug, Clone, Default)]
pub struct RecommendationPreferenceProfile {
    pub preferred_targets: Vec<String>,
    pub avoided_targets: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RecommendationApprovalReadinessDecision {
    pub ready: bool,
    pub reasons: Vec<String>,
    pub category: String,
    pub business_score: f64,
    pub confidence: f64,
    pub occurrences: Option<u32>,
    pub distinct_days: Option<u32>,
}

fn clamp_score(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn env_default_category() -> String {
    std::env::var("ALLVIA_RECOMMENDATION_DEFAULT_CATEGORY")
        .ok()
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| CATEGORY_WORK.to_string())
}

fn min_work_business_score() -> f64 {
    std::env::var("ALLVIA_WORK_MIN_BUSINESS_SCORE")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(clamp_score)
        .unwrap_or(0.55)
}

fn min_auto_approval_business_score() -> f64 {
    std::env::var("ALLVIA_AUTO_APPROVAL_MIN_BUSINESS_SCORE")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(clamp_score)
        .unwrap_or(0.68)
}

fn min_auto_approval_confidence() -> f64 {
    std::env::var("ALLVIA_AUTO_APPROVAL_MIN_CONFIDENCE")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(clamp_score)
        .unwrap_or(0.80)
}

fn min_auto_approval_distinct_days() -> u32 {
    std::env::var("ALLVIA_AUTO_APPROVAL_MIN_DISTINCT_DAYS")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .map(|v| v.clamp(1, 30))
        .unwrap_or(3)
}

fn min_auto_approval_occurrences() -> u32 {
    std::env::var("ALLVIA_AUTO_APPROVAL_MIN_OCCURRENCES")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .map(|v| v.clamp(1, 100))
        .unwrap_or(4)
}

fn min_pattern_work_context_score() -> f64 {
    std::env::var("ALLVIA_WORK_PATTERN_CONTEXT_MIN")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(clamp_score)
        .unwrap_or(0.6)
}

fn off_hours_pattern_context_score() -> f64 {
    std::env::var("ALLVIA_OFF_HOURS_PATTERN_CONTEXT_MAX")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(clamp_score)
        .unwrap_or(0.35)
}

fn auto_pending_limit_for_category(category: &str) -> usize {
    let env_key = if category.eq_ignore_ascii_case(CATEGORY_WORK) {
        "ALLVIA_AUTO_PENDING_WORK_LIMIT"
    } else {
        "ALLVIA_AUTO_PENDING_OTHER_LIMIT"
    };
    std::env::var(env_key)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|v| v.min(50))
        .unwrap_or_else(|| {
            if category.eq_ignore_ascii_case(CATEGORY_WORK) {
                5
            } else {
                0
            }
        })
}

pub fn pending_recommendation_display_limit() -> usize {
    std::env::var("ALLVIA_PENDING_RECOMMENDATION_DISPLAY_LIMIT")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|v| v.clamp(1, 100))
        .unwrap_or(5)
}

pub fn auto_recommendation_history_limit() -> i64 {
    std::env::var("ALLVIA_AUTO_RECOMMENDATION_HISTORY_LIMIT")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .map(|v| v.clamp(20, 1000))
        .unwrap_or(200)
}

fn push_unique(vec: &mut Vec<String>, value: String) {
    if !vec.iter().any(|existing| existing == &value) {
        vec.push(value);
    }
}

fn recommendation_auto_generated(rec: &crate::db::Recommendation) -> bool {
    rec.pattern_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
}

fn proposal_auto_generated(proposal: &AutomationProposal) -> bool {
    proposal
        .pattern_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
}

fn first_numeric_token(text: &str) -> Option<u32> {
    text.split(|c: char| !c.is_ascii_digit())
        .find_map(|token| token.parse::<u32>().ok())
}

fn extract_occurrences_from_recommendation(rec: &crate::db::Recommendation) -> Option<u32> {
    extract_occurrences_from_texts(&rec.evidence, &rec.summary)
}

fn extract_distinct_days_from_recommendation(rec: &crate::db::Recommendation) -> Option<u32> {
    extract_distinct_days_from_texts(&rec.evidence, &rec.summary)
}

fn extract_f64_suffix(entry: &str, marker: &str) -> Option<f64> {
    entry.split_once(marker)?.1.trim().parse::<f64>().ok()
}

fn evidence_has_marker(evidence: &[String], marker: &str) -> bool {
    evidence.iter().any(|entry| entry.contains(marker))
}

fn extract_policy_context_score(rec: &crate::db::Recommendation) -> Option<f64> {
    extract_policy_context_score_from_evidence(&rec.evidence)
}

fn extract_occurrences_from_texts(evidence: &[String], summary: &str) -> Option<u32> {
    evidence
        .iter()
        .find(|entry| {
            let lower = entry.to_ascii_lowercase();
            lower.contains("frequency:") || lower.contains("occurrence") || lower.contains("repeat")
        })
        .and_then(|entry| first_numeric_token(entry))
        .or_else(|| {
            let summary_lower = summary.to_ascii_lowercase();
            if summary_lower.contains("occurrence") || summary_lower.contains("repeat") {
                first_numeric_token(summary)
            } else {
                None
            }
        })
}

fn extract_distinct_days_from_texts(evidence: &[String], summary: &str) -> Option<u32> {
    evidence
        .iter()
        .find(|entry| entry.to_ascii_lowercase().contains("distinct day"))
        .and_then(|entry| first_numeric_token(entry))
        .or_else(|| {
            if summary.to_ascii_lowercase().contains("distinct day") {
                first_numeric_token(summary)
            } else {
                None
            }
        })
}

fn extract_policy_context_score_from_evidence(evidence: &[String]) -> Option<f64> {
    evidence.iter().find_map(|entry| {
        extract_f64_suffix(entry, "policy.reason=pattern.context_score=")
            .or_else(|| extract_f64_suffix(entry, "pattern.context_score="))
    })
}

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

fn normalize_title_key(value: &str) -> String {
    value
        .split_whitespace()
        .map(|token| token.trim().to_lowercase())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn collect_tokens(texts: &[String]) -> HashSet<String> {
    let mut tokens = HashSet::new();
    for text in texts {
        for token in text.split(|c: char| !c.is_alphanumeric()) {
            let trimmed = token.trim().to_lowercase();
            if trimmed.len() >= 2 {
                tokens.insert(trimmed);
            }
        }
    }
    tokens
}

fn count_hits(tokens: &HashSet<String>, signals: &[&str]) -> Vec<String> {
    signals
        .iter()
        .filter(|signal| tokens.contains(**signal))
        .map(|signal| (*signal).to_string())
        .collect()
}

fn pattern_context_profile(pattern: Option<&DetectedPattern>) -> Option<(f64, f64, f64)> {
    let pattern = pattern?;
    if pattern.occurrences == 0 {
        return None;
    }
    if pattern.weekday_occurrences == 0 && pattern.work_hour_occurrences == 0 {
        return None;
    }
    let occurrences = pattern.occurrences as f64;
    let weekday_ratio = (pattern.weekday_occurrences as f64 / occurrences).clamp(0.0, 1.0);
    let work_hour_ratio = (pattern.work_hour_occurrences as f64 / occurrences).clamp(0.0, 1.0);
    let context_score = clamp_score((weekday_ratio * 0.4) + (work_hour_ratio * 0.6));
    Some((weekday_ratio, work_hour_ratio, context_score))
}

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

fn texts_from_pattern_and_proposal(
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

fn texts_from_recommendation(rec: &crate::db::Recommendation) -> Vec<String> {
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

fn effective_recommendation_category(rec: &crate::db::Recommendation) -> String {
    if rec.category.trim().is_empty() || rec.category.eq_ignore_ascii_case(CATEGORY_UNKNOWN) {
        classify_recommendation_record(rec).category
    } else {
        rec.category.trim().to_lowercase()
    }
}

fn effective_recommendation_business_score(rec: &crate::db::Recommendation) -> f64 {
    if rec.business_score > 0.0 {
        rec.business_score.clamp(0.0, 1.0)
    } else {
        classify_recommendation_record(rec).business_score
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern_detector::{DetectedPattern, PatternType};
    use chrono::Utc;
    use serial_test::serial;

    fn work_pattern() -> DetectedPattern {
        DetectedPattern {
            pattern_id: "p_work".to_string(),
            pattern_type: PatternType::AppSequence,
            description: "Workflow Cycle: Slack -> Notion -> Calendar".to_string(),
            occurrences: 6,
            distinct_days: 3,
            weekday_occurrences: 6,
            work_hour_occurrences: 5,
            similarity_score: 0.92,
            sample_events: vec![
                r#"{"event_type":"app_switch","payload":{"app":"Slack"}}"#.to_string(),
                r#"{"event_type":"app_switch","payload":{"app":"Notion"}}"#.to_string(),
            ],
            detected_at: Utc::now(),
        }
    }

    fn base_recommendation() -> crate::db::Recommendation {
        crate::db::Recommendation {
            id: 1,
            status: "pending".to_string(),
            title: "Meeting Prep Assistant".to_string(),
            summary: "Detected 6 repeats across 4 distinct day(s).".to_string(),
            trigger: "meeting prep".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            n8n_prompt: "Create a meeting prep workflow that uses Slack and Calendar.".to_string(),
            confidence: 0.88,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![
                "Frequency: Found 6 occurrences".to_string(),
                "Span: 4 distinct day(s)".to_string(),
                "policy.reason=strong_work_signals=slack,calendar".to_string(),
                "policy.reason=pattern.context_score=0.83".to_string(),
                "policy.reason=pattern.work_context=strong".to_string(),
            ],
            pattern_id: Some("pattern-work".to_string()),
            last_error: None,
            snoozed_until: None,
            category: CATEGORY_WORK.to_string(),
            business_score: 0.82,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        }
    }

    fn strong_auto_evidence() -> Vec<String> {
        vec![
            "Frequency: Found 6 occurrences".to_string(),
            "Span: 4 distinct day(s)".to_string(),
            "policy.reason=strong_work_signals=slack,calendar".to_string(),
            "policy.reason=pattern.context_score=0.83".to_string(),
            "policy.reason=pattern.work_context=strong".to_string(),
        ]
    }

    #[test]
    fn work_pattern_is_accepted_for_mvp() {
        let mut proposal = AutomationProposal {
            title: "회의 준비 자동화".to_string(),
            summary: "Slack과 Calendar 기반 회의 준비".to_string(),
            trigger: "meeting prep".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.85,
            n8n_prompt: "Create a meeting prep workflow from Slack and Calendar.".to_string(),
            evidence: vec![],
            pattern_id: Some("p_work".to_string()),
            category: CATEGORY_UNKNOWN.to_string(),
            business_score: 0.0,
        };

        let decision = apply_mvp_policy(&mut proposal, Some(&work_pattern()));
        assert!(decision.accepted);
        assert_eq!(proposal.category, CATEGORY_WORK);
        assert!(proposal.business_score >= 0.55);
    }

    #[test]
    fn personal_pattern_is_rejected_for_mvp() {
        let mut proposal = AutomationProposal {
            title: "유튜브 정리".to_string(),
            summary: "YouTube와 Spotify 사용을 자동 정리".to_string(),
            trigger: "personal media".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.8,
            n8n_prompt: "Create a workflow for YouTube and Spotify usage.".to_string(),
            evidence: vec![],
            pattern_id: Some("p_personal".to_string()),
            category: CATEGORY_UNKNOWN.to_string(),
            business_score: 0.0,
        };

        let decision = apply_mvp_policy(&mut proposal, None);
        assert!(!decision.accepted);
        assert_eq!(proposal.category, CATEGORY_PERSONAL);
    }

    #[test]
    fn support_only_pattern_requires_work_context_for_mvp() {
        let mut proposal = AutomationProposal {
            title: "코딩 포커스 자동화".to_string(),
            summary: "VSCode와 Terminal 사용을 감지해 집중 모드를 켭니다.".to_string(),
            trigger: "coding focus".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.82,
            n8n_prompt: "Create a workflow that reacts to VSCode and Terminal usage.".to_string(),
            evidence: vec![],
            pattern_id: Some("p_support_only".to_string()),
            category: CATEGORY_UNKNOWN.to_string(),
            business_score: 0.0,
        };
        let off_hours_pattern = DetectedPattern {
            pattern_id: "p_support_only".to_string(),
            pattern_type: PatternType::AppSequence,
            description: "Workflow Cycle: VSCode -> Terminal".to_string(),
            occurrences: 6,
            distinct_days: 4,
            weekday_occurrences: 2,
            work_hour_occurrences: 0,
            similarity_score: 0.91,
            sample_events: vec![],
            detected_at: Utc::now(),
        };

        let decision = apply_mvp_policy(&mut proposal, Some(&off_hours_pattern));

        assert!(!decision.accepted);
        assert_eq!(proposal.category, CATEGORY_UNKNOWN);
        assert!(proposal.business_score < 0.55);
    }

    #[test]
    fn strong_work_context_can_promote_support_only_pattern() {
        let mut proposal = AutomationProposal {
            title: "코딩 포커스 자동화".to_string(),
            summary: "VSCode와 Terminal 사용을 감지해 집중 모드를 켭니다.".to_string(),
            trigger: "coding focus".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.82,
            n8n_prompt: "Create a workflow that reacts to VSCode and Terminal usage.".to_string(),
            evidence: vec![],
            pattern_id: Some("p_support_work".to_string()),
            category: CATEGORY_UNKNOWN.to_string(),
            business_score: 0.0,
        };
        let work_hours_pattern = DetectedPattern {
            pattern_id: "p_support_work".to_string(),
            pattern_type: PatternType::AppSequence,
            description: "Workflow Cycle: VSCode -> Terminal".to_string(),
            occurrences: 6,
            distinct_days: 4,
            weekday_occurrences: 6,
            work_hour_occurrences: 5,
            similarity_score: 0.91,
            sample_events: vec![],
            detected_at: Utc::now(),
        };

        let decision = apply_mvp_policy(&mut proposal, Some(&work_hours_pattern));

        assert!(decision.accepted);
        assert_eq!(proposal.category, CATEGORY_WORK);
        assert!(proposal.business_score >= 0.55);
        assert!(proposal
            .evidence
            .iter()
            .any(|entry| entry.contains("pattern.context_score")));
    }

    #[test]
    #[serial]
    fn list_category_filter_defaults_to_work_when_browsing_disabled() {
        std::env::remove_var("ALLVIA_ENABLE_RECOMMENDATION_CATEGORY_BROWSING");
        std::env::set_var("ALLVIA_RECOMMENDATION_LIST_DEFAULT_CATEGORY", "all");

        assert_eq!(
            normalize_list_category_filter(Some("all")).as_deref(),
            Some(CATEGORY_WORK)
        );
        assert_eq!(
            normalize_list_category_filter(Some("personal")).as_deref(),
            Some(CATEGORY_WORK)
        );
        assert_eq!(
            normalize_list_category_filter(None).as_deref(),
            Some(CATEGORY_WORK)
        );

        std::env::remove_var("ALLVIA_RECOMMENDATION_LIST_DEFAULT_CATEGORY");
    }

    #[test]
    #[serial]
    fn list_category_filter_allows_requested_category_when_browsing_enabled() {
        std::env::set_var("ALLVIA_ENABLE_RECOMMENDATION_CATEGORY_BROWSING", "true");
        std::env::set_var("ALLVIA_RECOMMENDATION_LIST_DEFAULT_CATEGORY", "work");

        assert_eq!(
            normalize_list_category_filter(Some("personal")).as_deref(),
            Some(CATEGORY_PERSONAL)
        );
        assert_eq!(
            normalize_list_category_filter(Some("all")).as_deref(),
            Some("all")
        );
        assert_eq!(
            normalize_list_category_filter(None).as_deref(),
            Some(CATEGORY_WORK)
        );

        std::env::remove_var("ALLVIA_ENABLE_RECOMMENDATION_CATEGORY_BROWSING");
        std::env::remove_var("ALLVIA_RECOMMENDATION_LIST_DEFAULT_CATEGORY");
    }

    #[test]
    fn manual_recommendation_is_ready_without_auto_gate() {
        let mut rec = base_recommendation();
        rec.pattern_id = None;
        rec.category = CATEGORY_UNKNOWN.to_string();
        rec.business_score = 0.0;

        let decision = evaluate_recommendation_approval_readiness(&rec);

        assert!(decision.ready);
        assert!(decision.reasons.is_empty());
    }

    #[test]
    fn auto_recommendation_requires_strong_evidence_for_approval() {
        let mut rec = base_recommendation();
        rec.confidence = 0.74;
        rec.business_score = 0.61;
        rec.summary = "Detected 3 repeats across 1 distinct day(s).".to_string();
        rec.evidence = vec![
            "Frequency: Found 3 occurrences".to_string(),
            "Span: 1 distinct day(s)".to_string(),
            "policy.reason=support_work_signals=terminal,vscode".to_string(),
        ];

        let decision = evaluate_recommendation_approval_readiness(&rec);

        assert!(!decision.ready);
        assert!(decision
            .reasons
            .iter()
            .any(|reason| reason.contains("Business score")));
        assert!(decision
            .reasons
            .iter()
            .any(|reason| reason.contains("Confidence")));
        assert!(decision
            .reasons
            .iter()
            .any(|reason| reason.contains("distinct days")));
        assert!(decision
            .reasons
            .iter()
            .any(|reason| reason.contains("strong work evidence")));
    }

    #[test]
    fn auto_recommendation_with_work_context_is_ready_for_approval() {
        let rec = base_recommendation();

        let decision = evaluate_recommendation_approval_readiness(&rec);

        assert!(decision.ready);
        assert_eq!(decision.occurrences, Some(6));
        assert_eq!(decision.distinct_days, Some(4));
    }

    #[test]
    fn auto_proposal_requires_strong_evidence_for_queueing() {
        let proposal = AutomationProposal {
            title: "Weak Queue Candidate".to_string(),
            summary: "Detected 3 repeats across 1 distinct day(s).".to_string(),
            trigger: "weak".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.74,
            n8n_prompt: "Create a workflow.".to_string(),
            evidence: vec![
                "Frequency: Found 3 occurrences".to_string(),
                "Span: 1 distinct day(s)".to_string(),
                "policy.reason=support_work_signals=vscode,terminal".to_string(),
            ],
            pattern_id: Some("weak-queue".to_string()),
            category: CATEGORY_WORK.to_string(),
            business_score: 0.61,
        };

        let decision = evaluate_auto_recommendation_readiness(&proposal);

        assert!(!decision.ready);
        assert!(decision
            .reasons
            .iter()
            .any(|reason| reason.contains("queue threshold")));
    }

    #[test]
    fn auto_proposal_with_strong_evidence_is_ready_for_queueing() {
        let proposal = AutomationProposal {
            title: "Ready Queue Candidate".to_string(),
            summary: "Detected 6 repeats across 4 distinct day(s).".to_string(),
            trigger: "ready".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.88,
            n8n_prompt: "Create a workflow.".to_string(),
            evidence: strong_auto_evidence(),
            pattern_id: Some("ready-queue".to_string()),
            category: CATEGORY_WORK.to_string(),
            business_score: 0.82,
        };

        let decision = evaluate_auto_recommendation_readiness(&proposal);

        assert!(decision.ready);
        assert_eq!(decision.occurrences, Some(6));
        assert_eq!(decision.distinct_days, Some(4));
    }

    #[test]
    fn auto_recommendation_queue_rejects_duplicate_pending_pattern() {
        let proposal = AutomationProposal {
            title: "Work Start Checklist".to_string(),
            summary: "Slack과 Notion 체크리스트".to_string(),
            trigger: "Workflow Cycle".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.88,
            n8n_prompt: "Create a work start workflow.".to_string(),
            evidence: vec![],
            pattern_id: Some("p_work".to_string()),
            category: CATEGORY_WORK.to_string(),
            business_score: 0.82,
        };
        let existing = vec![crate::db::Recommendation {
            id: 1,
            status: "pending".to_string(),
            title: "Work Start Checklist".to_string(),
            summary: "Existing".to_string(),
            trigger: "Workflow Cycle".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            n8n_prompt: "Create a work start workflow.".to_string(),
            confidence: 0.84,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![],
            pattern_id: Some("p_work".to_string()),
            last_error: None,
            snoozed_until: None,
            category: CATEGORY_WORK.to_string(),
            business_score: 0.8,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        }];

        let decision = admit_auto_recommendation(&proposal, &existing);
        assert!(!decision.accepted);
        assert!(decision
            .reasons
            .iter()
            .any(|reason| reason.contains("duplicate_pattern_id")));
    }

    #[test]
    fn auto_recommendation_queue_rejects_when_pending_limit_reached() {
        let proposal = AutomationProposal {
            title: "Meeting Prep Assistant".to_string(),
            summary: "회의 준비".to_string(),
            trigger: "meeting prep".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.9,
            n8n_prompt: "Create a meeting prep workflow.".to_string(),
            evidence: vec![],
            pattern_id: Some("p_work_2".to_string()),
            category: CATEGORY_WORK.to_string(),
            business_score: 0.86,
        };
        let existing = (0..5)
            .map(|idx| crate::db::Recommendation {
                id: idx + 1,
                status: "pending".to_string(),
                title: format!("Daily Briefing {}", idx),
                summary: "Existing".to_string(),
                trigger: format!("daily briefing {}", idx),
                actions: vec!["n8n Workflow".to_string()],
                n8n_prompt: "Create a daily briefing workflow.".to_string(),
                confidence: 0.8,
                workflow_id: None,
                workflow_json: None,
                evidence: vec![],
                pattern_id: Some(format!("p_work_{}", idx)),
                last_error: None,
                snoozed_until: None,
                category: CATEGORY_WORK.to_string(),
                business_score: 0.78,
                feedback_status: None,
                feedback_note: None,
                feedback_count: 0,
                last_feedback_at: None,
            })
            .collect::<Vec<_>>();

        let decision = admit_auto_recommendation(&proposal, &existing);

        assert!(!decision.accepted);
        assert_eq!(decision.pending_same_category, 5);
        assert_eq!(decision.pending_limit, 5);
        assert!(decision
            .reasons
            .iter()
            .any(|reason| reason.contains("pending_limit_reached")));
    }

    #[test]
    fn non_work_auto_recommendation_queue_is_disabled_by_default() {
        let proposal = AutomationProposal {
            title: "Downloads Cleanup".to_string(),
            summary: "downloads folder cleanup".to_string(),
            trigger: "downloads cleanup".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.83,
            n8n_prompt: "Create a downloads cleanup workflow.".to_string(),
            evidence: vec![],
            pattern_id: Some("p_system".to_string()),
            category: CATEGORY_SYSTEM.to_string(),
            business_score: 0.32,
        };

        let decision = admit_auto_recommendation(&proposal, &[]);

        assert!(!decision.accepted);
        assert_eq!(decision.pending_limit, 0);
        assert!(decision
            .reasons
            .iter()
            .any(|reason| reason.contains("pending_limit_reached")));
    }

    #[test]
    fn rejected_history_blocks_similar_auto_recommendation() {
        let proposal = AutomationProposal {
            title: "Meeting Prep Assistant".to_string(),
            summary: "회의 준비".to_string(),
            trigger: "meeting prep".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.9,
            n8n_prompt: "Create a meeting prep workflow.".to_string(),
            evidence: vec![],
            pattern_id: Some("p_work_2".to_string()),
            category: CATEGORY_WORK.to_string(),
            business_score: 0.86,
        };
        let existing = vec![crate::db::Recommendation {
            id: 1,
            status: "rejected".to_string(),
            title: "Meeting Prep Assistant".to_string(),
            summary: "Existing".to_string(),
            trigger: "meeting prep".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            n8n_prompt: "Create a meeting prep workflow.".to_string(),
            confidence: 0.8,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![],
            pattern_id: Some("p_work_2".to_string()),
            last_error: None,
            snoozed_until: None,
            category: CATEGORY_WORK.to_string(),
            business_score: 0.78,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        }];

        let decision = admit_auto_recommendation(&proposal, &existing);

        assert!(!decision.accepted);
        assert!(decision
            .reasons
            .iter()
            .any(|reason| reason.contains("history.rejected_pattern_id")));
    }

    #[test]
    fn approved_history_blocks_duplicate_auto_recommendation() {
        let proposal = AutomationProposal {
            title: "Daily Briefing".to_string(),
            summary: "업무 브리핑".to_string(),
            trigger: "daily briefing".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.9,
            n8n_prompt: "Create a daily briefing workflow.".to_string(),
            evidence: vec![],
            pattern_id: Some("p_work_3".to_string()),
            category: CATEGORY_WORK.to_string(),
            business_score: 0.86,
        };
        let existing = vec![crate::db::Recommendation {
            id: 1,
            status: "approved".to_string(),
            title: "Daily Briefing".to_string(),
            summary: "Existing".to_string(),
            trigger: "daily briefing".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            n8n_prompt: "Create a daily briefing workflow.".to_string(),
            confidence: 0.8,
            workflow_id: Some("wf-123".to_string()),
            workflow_json: None,
            evidence: vec![],
            pattern_id: Some("p_other".to_string()),
            last_error: None,
            snoozed_until: None,
            category: CATEGORY_WORK.to_string(),
            business_score: 0.78,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        }];

        let decision = admit_auto_recommendation(&proposal, &existing);

        assert!(!decision.accepted);
        assert!(
            decision
                .reasons
                .iter()
                .any(|reason| reason.contains("history.approved_title_pending"))
                || decision
                    .reasons
                    .iter()
                    .any(|reason| reason.contains("history.approved_fingerprint_pending"))
        );
    }

    #[test]
    fn refinement_feedback_builds_destination_preference_profile() {
        let existing = vec![crate::db::Recommendation {
            id: 1,
            status: "pending".to_string(),
            title: "Slack Digest".to_string(),
            summary: "digest".to_string(),
            trigger: "digest".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            n8n_prompt: "Send digest to Telegram.".to_string(),
            confidence: 0.8,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![],
            pattern_id: Some("p_feedback".to_string()),
            last_error: None,
            snoozed_until: Some((chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339()),
            category: CATEGORY_WORK.to_string(),
            business_score: 0.8,
            feedback_status: Some("refine".to_string()),
            feedback_note: Some("텔레그램 말고 노션으로 저장해줘".to_string()),
            feedback_count: 1,
            last_feedback_at: Some(chrono::Utc::now().to_rfc3339()),
        }];

        let profile = build_recommendation_preference_profile(&existing);
        assert_eq!(profile.preferred_targets, vec![TARGET_NOTION.to_string()]);
        assert_eq!(profile.avoided_targets, vec![TARGET_TELEGRAM.to_string()]);
    }

    #[test]
    fn approved_and_rejected_reviews_contribute_soft_preferences() {
        let existing = vec![
            crate::db::Recommendation {
                id: 1,
                status: "approved".to_string(),
                title: "Meeting Notes".to_string(),
                summary: "Save notes to Notion".to_string(),
                trigger: "meeting".to_string(),
                actions: vec!["n8n Workflow".to_string()],
                n8n_prompt: "Create a workflow that writes meeting notes to Notion.".to_string(),
                confidence: 0.8,
                workflow_id: Some("wf-1".to_string()),
                workflow_json: None,
                evidence: vec![],
                pattern_id: Some("p_notion".to_string()),
                last_error: None,
                snoozed_until: None,
                category: CATEGORY_WORK.to_string(),
                business_score: 0.8,
                feedback_status: None,
                feedback_note: None,
                feedback_count: 0,
                last_feedback_at: None,
            },
            crate::db::Recommendation {
                id: 2,
                status: "rejected".to_string(),
                title: "Telegram Digest".to_string(),
                summary: "Send digest to Telegram".to_string(),
                trigger: "digest".to_string(),
                actions: vec!["n8n Workflow".to_string()],
                n8n_prompt: "Create a workflow that sends updates to Telegram.".to_string(),
                confidence: 0.8,
                workflow_id: None,
                workflow_json: None,
                evidence: vec![],
                pattern_id: Some("p_telegram".to_string()),
                last_error: None,
                snoozed_until: None,
                category: CATEGORY_WORK.to_string(),
                business_score: 0.8,
                feedback_status: None,
                feedback_note: None,
                feedback_count: 0,
                last_feedback_at: None,
            },
        ];

        let profile = build_recommendation_preference_profile(&existing);
        assert_eq!(profile.preferred_targets, vec![TARGET_NOTION.to_string()]);
        assert_eq!(profile.avoided_targets, vec![TARGET_TELEGRAM.to_string()]);
    }

    #[test]
    fn apply_recommendation_preferences_injects_preference_hints() {
        let existing = vec![crate::db::Recommendation {
            id: 1,
            status: "pending".to_string(),
            title: "Slack Digest".to_string(),
            summary: "digest".to_string(),
            trigger: "digest".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            n8n_prompt: "Send digest to Telegram.".to_string(),
            confidence: 0.8,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![],
            pattern_id: Some("p_feedback".to_string()),
            last_error: None,
            snoozed_until: Some((chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339()),
            category: CATEGORY_WORK.to_string(),
            business_score: 0.8,
            feedback_status: Some("refine".to_string()),
            feedback_note: Some("텔레그램 말고 노션으로 저장해줘".to_string()),
            feedback_count: 1,
            last_feedback_at: Some(chrono::Utc::now().to_rfc3339()),
        }];
        let mut proposal = AutomationProposal {
            title: "Meeting Prep Assistant".to_string(),
            summary: "회의 준비".to_string(),
            trigger: "meeting prep".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.9,
            n8n_prompt: "Create a workflow that sends meeting notes to Telegram.".to_string(),
            evidence: vec![],
            pattern_id: Some("p_work_4".to_string()),
            category: CATEGORY_WORK.to_string(),
            business_score: 0.86,
        };

        let profile = apply_recommendation_preferences(&mut proposal, &existing);

        assert_eq!(profile.preferred_targets, vec![TARGET_NOTION.to_string()]);
        assert_eq!(profile.avoided_targets, vec![TARGET_TELEGRAM.to_string()]);
        assert!(proposal.n8n_prompt.contains("Prefer Notion"));
        assert!(proposal.n8n_prompt.contains("Avoid Telegram"));
        assert!(proposal
            .evidence
            .iter()
            .any(|entry| entry.contains("preference.prefer=notion")));
        assert!(proposal
            .evidence
            .iter()
            .any(|entry| entry.contains("preference.conflict=telegram")));
    }

    #[test]
    fn apply_recommendation_preferences_penalizes_avoided_targets() {
        let existing = vec![crate::db::Recommendation {
            id: 1,
            status: "rejected".to_string(),
            title: "Telegram Digest".to_string(),
            summary: "Send digest to Telegram".to_string(),
            trigger: "digest".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            n8n_prompt: "Create a workflow that sends updates to Telegram.".to_string(),
            confidence: 0.8,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![],
            pattern_id: Some("p_feedback_reject".to_string()),
            last_error: None,
            snoozed_until: None,
            category: CATEGORY_WORK.to_string(),
            business_score: 0.8,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        }];
        let mut proposal = AutomationProposal {
            title: "Status Digest".to_string(),
            summary: "업무 요약".to_string(),
            trigger: "digest".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.90,
            n8n_prompt: "Create a workflow that sends a work digest to Telegram.".to_string(),
            evidence: vec![],
            pattern_id: Some("p_work_penalty".to_string()),
            category: CATEGORY_WORK.to_string(),
            business_score: 0.86,
        };

        let profile = apply_recommendation_preferences(&mut proposal, &existing);

        assert_eq!(profile.avoided_targets, vec![TARGET_TELEGRAM.to_string()]);
        assert!((proposal.business_score - 0.78).abs() < 1e-9);
        assert!((proposal.confidence - 0.86).abs() < 1e-9);
        assert!(proposal
            .evidence
            .iter()
            .any(|entry| entry == "preference.business_score_adjusted=0.78"));
    }

    #[test]
    fn apply_recommendation_preferences_rewards_preferred_targets() {
        let existing = vec![crate::db::Recommendation {
            id: 1,
            status: "approved".to_string(),
            title: "Notion Notes".to_string(),
            summary: "Save notes to Notion".to_string(),
            trigger: "meeting".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            n8n_prompt: "Create a workflow that writes meeting notes to Notion.".to_string(),
            confidence: 0.8,
            workflow_id: Some("wf-1".to_string()),
            workflow_json: None,
            evidence: vec![],
            pattern_id: Some("p_feedback_approve".to_string()),
            last_error: None,
            snoozed_until: None,
            category: CATEGORY_WORK.to_string(),
            business_score: 0.8,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        }];
        let mut proposal = AutomationProposal {
            title: "Meeting Prep".to_string(),
            summary: "회의 준비".to_string(),
            trigger: "meeting".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.90,
            n8n_prompt: "Create a workflow that stores meeting prep notes in Notion.".to_string(),
            evidence: vec![],
            pattern_id: Some("p_work_bonus".to_string()),
            category: CATEGORY_WORK.to_string(),
            business_score: 0.82,
        };

        let profile = apply_recommendation_preferences(&mut proposal, &existing);

        assert_eq!(profile.preferred_targets, vec![TARGET_NOTION.to_string()]);
        assert!((proposal.business_score - 0.86).abs() < 1e-9);
        assert!((proposal.confidence - 0.91).abs() < 1e-9);
        assert!(proposal
            .evidence
            .iter()
            .any(|entry| entry == "preference.match=notion"));
    }

    #[test]
    fn snoozed_recommendations_do_not_consume_pending_limit() {
        let proposal = AutomationProposal {
            title: "Meeting Prep Assistant".to_string(),
            summary: "Detected 6 repeats across 4 distinct day(s).".to_string(),
            trigger: "meeting prep".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.9,
            n8n_prompt: "Create a meeting prep workflow.".to_string(),
            evidence: strong_auto_evidence(),
            pattern_id: Some("p_work_2".to_string()),
            category: CATEGORY_WORK.to_string(),
            business_score: 0.86,
        };
        let snoozed_until = (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339();
        let existing = (0..5)
            .map(|idx| crate::db::Recommendation {
                id: idx + 1,
                status: "pending".to_string(),
                title: format!("Later Workflow {}", idx),
                summary: "Existing".to_string(),
                trigger: format!("later {}", idx),
                actions: vec!["n8n Workflow".to_string()],
                n8n_prompt: "Create a later workflow.".to_string(),
                confidence: 0.8,
                workflow_id: None,
                workflow_json: None,
                evidence: vec![],
                pattern_id: Some(format!("p_later_{}", idx)),
                last_error: None,
                snoozed_until: Some(snoozed_until.clone()),
                category: CATEGORY_WORK.to_string(),
                business_score: 0.78,
                feedback_status: None,
                feedback_note: None,
                feedback_count: 0,
                last_feedback_at: None,
            })
            .collect::<Vec<_>>();

        let decision = admit_auto_recommendation(&proposal, &existing);

        assert!(decision.accepted);
        assert_eq!(decision.pending_same_category, 0);
    }

    #[test]
    fn snoozed_duplicate_recommendation_is_still_suppressed() {
        let proposal = AutomationProposal {
            title: "Meeting Prep Assistant".to_string(),
            summary: "회의 준비".to_string(),
            trigger: "meeting prep".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            confidence: 0.9,
            n8n_prompt: "Create a meeting prep workflow.".to_string(),
            evidence: vec![],
            pattern_id: Some("p_work_2".to_string()),
            category: CATEGORY_WORK.to_string(),
            business_score: 0.86,
        };
        let existing = vec![crate::db::Recommendation {
            id: 1,
            status: "pending".to_string(),
            title: "Meeting Prep Assistant".to_string(),
            summary: "Existing".to_string(),
            trigger: "meeting prep".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            n8n_prompt: "Create a meeting prep workflow.".to_string(),
            confidence: 0.8,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![],
            pattern_id: Some("p_work_2".to_string()),
            last_error: None,
            snoozed_until: Some((chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339()),
            category: CATEGORY_WORK.to_string(),
            business_score: 0.78,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        }];

        let decision = admit_auto_recommendation(&proposal, &existing);

        assert!(!decision.accepted);
        assert!(decision
            .reasons
            .iter()
            .any(|reason| reason.contains("snoozed_pattern_id")));
    }
}
