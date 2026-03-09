use super::*;
use std::collections::HashSet;

pub(super) const STRONG_WORK_SIGNALS: &[&str] = &[
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

pub(super) const SUPPORT_WORK_SIGNALS: &[&str] = &[
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

pub(super) const PERSONAL_SIGNALS: &[&str] = &[
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

pub(super) const SYSTEM_SIGNALS: &[&str] = &[
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

pub(super) const TARGET_NOTION: &str = "notion";
pub(super) const TARGET_TELEGRAM: &str = "telegram";
pub(super) const TARGET_EMAIL: &str = "email";
pub(super) const TARGET_SLACK: &str = "slack";

pub(super) const NOTION_TARGET_ALIASES: &[&str] = &["notion", "노션"];
pub(super) const TELEGRAM_TARGET_ALIASES: &[&str] = &["telegram", "텔레그램"];
pub(super) const EMAIL_TARGET_ALIASES: &[&str] =
    &["email", "mail", "gmail", "이메일", "메일", "지메일"];
pub(super) const SLACK_TARGET_ALIASES: &[&str] = &["slack", "슬랙"];

pub(super) const PREFERENCE_SWAP_MARKERS: &[&str] = &["말고", "대신", "instead of", "rather than"];

pub(super) fn push_unique(vec: &mut Vec<String>, value: String) {
    if !vec.iter().any(|existing| existing == &value) {
        vec.push(value);
    }
}

pub(super) fn recommendation_auto_generated(rec: &crate::db::Recommendation) -> bool {
    rec.pattern_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
}

pub(super) fn proposal_auto_generated(proposal: &AutomationProposal) -> bool {
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

pub(super) fn extract_occurrences_from_recommendation(
    rec: &crate::db::Recommendation,
) -> Option<u32> {
    extract_occurrences_from_texts(&rec.evidence, &rec.summary)
}

pub(super) fn extract_distinct_days_from_recommendation(
    rec: &crate::db::Recommendation,
) -> Option<u32> {
    extract_distinct_days_from_texts(&rec.evidence, &rec.summary)
}

fn extract_f64_suffix(entry: &str, marker: &str) -> Option<f64> {
    entry.split_once(marker)?.1.trim().parse::<f64>().ok()
}

pub(super) fn evidence_has_marker(evidence: &[String], marker: &str) -> bool {
    evidence.iter().any(|entry| entry.contains(marker))
}

pub(super) fn extract_policy_context_score(rec: &crate::db::Recommendation) -> Option<f64> {
    extract_policy_context_score_from_evidence(&rec.evidence)
}

pub(super) fn extract_occurrences_from_texts(evidence: &[String], summary: &str) -> Option<u32> {
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

pub(super) fn extract_distinct_days_from_texts(evidence: &[String], summary: &str) -> Option<u32> {
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

pub(super) fn extract_policy_context_score_from_evidence(evidence: &[String]) -> Option<f64> {
    evidence.iter().find_map(|entry| {
        extract_f64_suffix(entry, "policy.reason=pattern.context_score=")
            .or_else(|| extract_f64_suffix(entry, "pattern.context_score="))
    })
}

pub(super) fn normalize_title_key(value: &str) -> String {
    value
        .split_whitespace()
        .map(|token| token.trim().to_lowercase())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn collect_tokens(texts: &[String]) -> HashSet<String> {
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

pub(super) fn count_hits(tokens: &HashSet<String>, signals: &[&str]) -> Vec<String> {
    signals
        .iter()
        .filter(|signal| tokens.contains(**signal))
        .map(|signal| (*signal).to_string())
        .collect()
}

pub(super) fn pattern_context_profile(
    pattern: Option<&DetectedPattern>,
) -> Option<(f64, f64, f64)> {
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
