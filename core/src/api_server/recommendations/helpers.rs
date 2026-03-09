use std::cmp::Ordering as CmpOrdering;

use crate::db;

pub(crate) fn n8n_editor_base_url() -> String {
    let normalize = |input: &str| {
        let mut base = input.trim().trim_end_matches('/').to_string();
        for (from, to) in [
            ("http://127.0.0.1", "http://localhost"),
            ("https://127.0.0.1", "https://localhost"),
            ("http://0.0.0.0", "http://localhost"),
            ("https://0.0.0.0", "https://localhost"),
            ("http://[::1]", "http://localhost"),
            ("https://[::1]", "https://localhost"),
        ] {
            if base.starts_with(from) {
                base = base.replacen(from, to, 1);
                break;
            }
        }
        base
    };

    if let Ok(editor) = std::env::var("N8N_EDITOR_URL") {
        let trimmed = editor.trim().trim_end_matches('/');
        if !trimmed.is_empty() {
            return normalize(trimmed);
        }
    }

    let api = std::env::var("STEER_N8N_API_URL")
        .or_else(|_| std::env::var("N8N_API_URL"))
        .ok()
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "http://localhost:5678/api/v1".to_string());

    for suffix in ["/api/v1", "/api/v2", "/api"] {
        if let Some(stripped) = api.strip_suffix(suffix) {
            let candidate = stripped.trim_end_matches('/');
            if !candidate.is_empty() {
                return normalize(candidate);
            }
        }
    }

    normalize(&api)
}

pub(crate) fn workflow_editor_url(workflow_id: &str) -> Option<String> {
    let trimmed = workflow_id.trim();
    if trimmed.is_empty() || trimmed.starts_with("provisioning:") {
        return None;
    }
    Some(format!("{}/workflow/{}", n8n_editor_base_url(), trimmed))
}

pub(crate) fn recommendation_effective_category_and_score(
    rec: &db::Recommendation,
) -> (String, f64) {
    let derived = crate::recommendation_policy::classify_recommendation_record(rec);
    let effective_category = if rec.category.trim().is_empty()
        || rec
            .category
            .eq_ignore_ascii_case(crate::recommendation_policy::CATEGORY_UNKNOWN)
    {
        derived.category
    } else {
        rec.category.clone()
    };
    let effective_business_score = if rec.business_score > 0.0 {
        rec.business_score
    } else {
        derived.business_score
    };
    (effective_category, effective_business_score)
}

fn recommendation_is_snoozed(rec: &db::Recommendation) -> bool {
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

pub(crate) fn recommendation_effective_status(rec: &db::Recommendation) -> String {
    if rec.status.eq_ignore_ascii_case("pending")
        && rec
            .last_error
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_some()
    {
        "failed".to_string()
    } else if rec.status.eq_ignore_ascii_case("pending") && recommendation_is_snoozed(rec) {
        "later".to_string()
    } else {
        rec.status.clone()
    }
}

pub(crate) fn recommendation_approval_block_message(id: i64, reasons: &[String]) -> String {
    if reasons.is_empty() {
        return format!("Recommendation {} is not ready for approval yet.", id);
    }
    format!(
        "Recommendation {} needs stronger business evidence before approval: {}",
        id,
        reasons.join(" ")
    )
}

pub(crate) fn recommendation_review_actor(actor: Option<&str>, fallback: &str) -> String {
    actor
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

pub(crate) fn log_recommendation_review_event(
    recommendation_id: i64,
    recommendation_title: &str,
    status_after: Option<&str>,
    category: Option<&str>,
    action: &str,
    actor: Option<&str>,
    note: Option<&str>,
    ok: bool,
    message: Option<&str>,
) {
    let _ = db::record_recommendation_review_event(
        recommendation_id,
        recommendation_title,
        status_after,
        category,
        action,
        actor,
        note,
        ok,
        message,
    );
}

pub(crate) fn recommendation_feedback_snooze_hours() -> i64 {
    std::env::var("ALLVIA_RECOMMENDATION_REFINE_SNOOZE_HOURS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .map(|v| v.clamp(1, 24 * 30))
        .unwrap_or(24 * 14)
}

pub(crate) fn classify_recommendation_feedback(feedback: &str) -> &'static str {
    let normalized = feedback.trim().to_lowercase();
    if normalized.is_empty() {
        return "refine";
    }

    let has_negative_signal = [
        "싫",
        "별로",
        "원치",
        "필요없",
        "아냐",
        "아님",
        "잘못",
        "틀렸",
        "노이즈",
        "중복",
        "spam",
        "duplicate",
        "irrelevant",
        "wrong",
        "not useful",
        "don't",
        "do not",
        "no need",
    ]
    .iter()
    .any(|token| normalized.contains(token));
    if has_negative_signal {
        return "negative";
    }

    let has_positive_signal = [
        "좋",
        "괜찮",
        "유용",
        "원하던",
        "마음에",
        "keep",
        "good",
        "great",
        "helpful",
        "useful",
        "correct",
    ]
    .iter()
    .any(|token| normalized.contains(token));
    if has_positive_signal {
        return "positive";
    }

    "refine"
}

pub(crate) fn limit_visible_recommendations(
    recs: Vec<db::Recommendation>,
    category_filter: Option<&str>,
    preference_history: &[db::Recommendation],
) -> Vec<db::Recommendation> {
    let limit = crate::recommendation_policy::pending_recommendation_display_limit();
    let preference_profile =
        crate::recommendation_policy::build_recommendation_preference_profile(preference_history);
    if limit == 0 {
        return recs
            .into_iter()
            .filter(|rec| {
                let (category, _) = recommendation_effective_category_and_score(rec);
                crate::recommendation_policy::matches_category_filter(&category, category_filter)
            })
            .collect();
    }

    let mut manual_pending = Vec::new();
    let mut auto_pending = Vec::new();
    let mut others = Vec::new();

    for rec in recs {
        let (effective_category, _) = recommendation_effective_category_and_score(&rec);
        if !crate::recommendation_policy::matches_category_filter(
            &effective_category,
            category_filter,
        ) {
            continue;
        }
        let effective_status = recommendation_effective_status(&rec);
        if effective_status.eq_ignore_ascii_case("pending") {
            if rec.pattern_id.is_some() {
                let readiness =
                    crate::recommendation_policy::evaluate_recommendation_approval_readiness(&rec);
                if !readiness.ready {
                    continue;
                }
                auto_pending.push(rec);
            } else {
                manual_pending.push(rec);
            }
        } else {
            others.push(rec);
        }
    }

    auto_pending.sort_by(|left, right| {
        let right_ready =
            crate::recommendation_policy::evaluate_recommendation_approval_readiness(right).ready;
        let left_ready =
            crate::recommendation_policy::evaluate_recommendation_approval_readiness(left).ready;
        right_ready.cmp(&left_ready).then_with(|| {
            crate::recommendation_policy::recommendation_record_priority_score_with_profile(
                right,
                &preference_profile,
            )
            .partial_cmp(
                &crate::recommendation_policy::recommendation_record_priority_score_with_profile(
                    left,
                    &preference_profile,
                ),
            )
            .unwrap_or(CmpOrdering::Equal)
        })
    });

    let mut visible = manual_pending;
    visible.extend(auto_pending.into_iter().take(limit));
    visible.extend(others);
    visible
}
