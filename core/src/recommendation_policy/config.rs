use super::CATEGORY_WORK;

pub(super) fn clamp_score(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

pub(super) fn env_default_category() -> String {
    std::env::var("ALLVIA_RECOMMENDATION_DEFAULT_CATEGORY")
        .ok()
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| CATEGORY_WORK.to_string())
}

pub(super) fn min_work_business_score() -> f64 {
    std::env::var("ALLVIA_WORK_MIN_BUSINESS_SCORE")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(clamp_score)
        .unwrap_or(0.55)
}

pub(super) fn min_auto_approval_business_score() -> f64 {
    std::env::var("ALLVIA_AUTO_APPROVAL_MIN_BUSINESS_SCORE")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(clamp_score)
        .unwrap_or(0.68)
}

pub(super) fn min_auto_approval_confidence() -> f64 {
    std::env::var("ALLVIA_AUTO_APPROVAL_MIN_CONFIDENCE")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(clamp_score)
        .unwrap_or(0.80)
}

pub(super) fn min_auto_approval_distinct_days() -> u32 {
    std::env::var("ALLVIA_AUTO_APPROVAL_MIN_DISTINCT_DAYS")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .map(|v| v.clamp(1, 30))
        .unwrap_or(3)
}

pub(super) fn min_auto_approval_occurrences() -> u32 {
    std::env::var("ALLVIA_AUTO_APPROVAL_MIN_OCCURRENCES")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .map(|v| v.clamp(1, 100))
        .unwrap_or(4)
}

pub(super) fn min_pattern_work_context_score() -> f64 {
    std::env::var("ALLVIA_WORK_PATTERN_CONTEXT_MIN")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(clamp_score)
        .unwrap_or(0.6)
}

pub(super) fn off_hours_pattern_context_score() -> f64 {
    std::env::var("ALLVIA_OFF_HOURS_PATTERN_CONTEXT_MAX")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(clamp_score)
        .unwrap_or(0.35)
}

pub(super) fn auto_pending_limit_for_category(category: &str) -> usize {
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
