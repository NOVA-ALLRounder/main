use crate::{collector_pipeline, db, recommendation::AutomationProposal};
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn summarize_prompt(prompt: &str, max_chars: usize) -> String {
    let trimmed = prompt.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let short = trimmed.chars().take(max_chars).collect::<String>();
    format!("{}...", short)
}

pub fn recommendation_fingerprint(title: &str, trigger: &str) -> String {
    format!(
        "{}::{}",
        title.trim().to_lowercase(),
        trigger.trim().to_lowercase()
    )
}

pub fn find_recommendation_id_by_fingerprint(target: &str) -> Result<Option<i64>> {
    let rows = db::get_recommendations_with_filter(Some("all"))?;
    for rec in rows {
        let fp = recommendation_fingerprint(&rec.title, &rec.trigger);
        if fp == target {
            return Ok(Some(rec.id));
        }
    }
    Ok(None)
}

pub fn config_path(config_override: Option<&str>) -> PathBuf {
    if let Some(path) = config_override.map(str::trim).filter(|s| !s.is_empty()) {
        return PathBuf::from(path);
    }
    std::env::var("STEER_COLLECTOR_CONFIG")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("configs/config.yaml"))
}

pub fn normalize_abs_path(path: &Path) -> String {
    if let Ok(canon) = std::fs::canonicalize(path) {
        return canon.to_string_lossy().to_string();
    }
    if path.is_absolute() {
        return path.to_string_lossy().to_string();
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(path)
        .to_string_lossy()
        .to_string()
}

pub fn allow_collector_db_mismatch() -> bool {
    std::env::var("STEER_ALLOW_COLLECTOR_DB_MISMATCH")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

pub fn handoff_max_attempts() -> i64 {
    std::env::var("STEER_COLLECTOR_HANDOFF_MAX_ATTEMPTS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(5)
}

pub fn handoff_retry_base_secs() -> u64 {
    std::env::var("STEER_COLLECTOR_HANDOFF_RETRY_BASE_SECS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(60)
}

pub fn handoff_lease_secs() -> i64 {
    std::env::var("STEER_COLLECTOR_HANDOFF_LEASE_SECS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|v| *v >= 10)
        .unwrap_or(180)
}

pub fn expected_handoff_schema_major() -> u64 {
    std::env::var("STEER_COLLECTOR_HANDOFF_SCHEMA_MAJOR")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(1)
}

pub fn parse_handoff_major(payload: &Value) -> Option<u64> {
    let raw = payload.get("version")?.as_str()?.trim();
    if raw.is_empty() {
        return None;
    }
    let normalized = raw.trim_start_matches(['v', 'V']);
    let major = normalized.split('.').next()?.trim();
    major.parse::<u64>().ok()
}

pub fn validate_handoff_schema(payload: &Value) -> Result<()> {
    let expected_major = expected_handoff_schema_major();
    let parsed_major = parse_handoff_major(payload).ok_or_else(|| {
        anyhow!(
            "collector handoff missing/invalid version (expected major={})",
            expected_major
        )
    })?;
    if parsed_major != expected_major {
        return Err(anyhow!(
            "collector handoff schema mismatch: got major={} expected major={}",
            parsed_major,
            expected_major
        ));
    }
    Ok(())
}

pub fn extract_sequence(candidate: &Value) -> String {
    let Some(events) = candidate
        .get("pattern")
        .and_then(|v| v.get("events"))
        .and_then(|v| v.as_array())
    else {
        return String::new();
    };
    let parts = events
        .iter()
        .filter_map(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
        .take(5)
        .collect::<Vec<_>>();
    parts.join(" -> ")
}

pub fn build_proposal_from_handoff(
    row: &collector_pipeline::PendingHandoffRow,
) -> Result<AutomationProposal> {
    let Some(top) = row
        .payload
        .get("routine_candidates")
        .and_then(|v| v.as_array())
        .and_then(|v| v.first())
    else {
        return Err(anyhow!("no routine_candidates in handoff payload"));
    };

    let pattern_id = top
        .get("pattern_id")
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    let support = top.get("support").and_then(|v| v.as_i64()).unwrap_or(0);
    let confidence = top
        .get("confidence")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.6)
        .clamp(0.1, 0.99);
    let sequence = extract_sequence(top);
    let active_app = row
        .payload
        .get("device_context")
        .and_then(|v| v.get("active_app"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let sequence_label = if sequence.is_empty() {
        "Routine candidate".to_string()
    } else {
        sequence.clone()
    };

    let title = format!(
        "Collector Routine: {}",
        summarize_prompt(&sequence_label, 64)
    );
    let trigger = pattern_id
        .clone()
        .map(|p| format!("Collector pattern {}", p))
        .unwrap_or_else(|| format!("Collector package {}", row.package_id));

    let summary = format!(
        "Collector handoff 기반 루틴 후보입니다. support={}, confidence={:.2}, active_app={}",
        support, confidence, active_app
    );
    let n8n_prompt = if sequence.is_empty() {
        format!(
            "Create an n8n workflow for repeated activity detected from collector package {}. Include a Telegram summary and optional Notion logging.",
            row.package_id
        )
    } else {
        format!(
            "Create an n8n workflow for this repeated sequence: {}. support={}, confidence={:.2}, active_app={}. Include Telegram summary and human approval checkpoint before side effects.",
            sequence, support, confidence, active_app
        )
    };

    Ok(AutomationProposal {
        title,
        summary,
        trigger,
        actions: vec!["collector_handoff".to_string(), "n8n Workflow".to_string()],
        confidence,
        n8n_prompt,
        evidence: vec![
            format!("package_id={}", row.package_id),
            format!("handoff_created_at={}", row.created_at),
            format!("support={}", support),
            format!("active_app={}", active_app),
            format!("sequence={}", summarize_prompt(&sequence_label, 160)),
        ],
        pattern_id,
        category: crate::recommendation_policy::CATEGORY_UNKNOWN.to_string(),
        business_score: 0.0,
    })
}
