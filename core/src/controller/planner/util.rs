use super::Planner;
use crate::platform::current_platform;
use std::path::{Path, PathBuf};

pub(super) fn notion_api_ready() -> bool {
    crate::load_env_with_fallback();
    let has_key = std::env::var("NOTION_API_KEY")
        .ok()
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    let has_target = std::env::var("NOTION_DATABASE_ID")
        .ok()
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
        || std::env::var("NOTION_PAGE_ID")
            .ok()
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
    has_key && has_target
}

pub(super) fn planner_retry_config() -> crate::retry_logic::RetryConfig {
    crate::retry_logic::RetryConfig {
        max_attempts: Planner::env_usize("STEER_PLANNER_MAX_ATTEMPTS", 1),
        base_delay_ms: Planner::env_u64("STEER_PLANNER_RETRY_BASE_DELAY_MS", 300),
        max_delay_ms: Planner::env_u64("STEER_PLANNER_RETRY_MAX_DELAY_MS", 3000),
        backoff_multiplier: 2.0,
    }
}

pub(super) fn sanitize_filename_token(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let collapsed = out.trim_matches('_');
    let mut final_token = if collapsed.is_empty() {
        "node".to_string()
    } else {
        collapsed.to_string()
    };
    if final_token.len() > 48 {
        final_token.truncate(48);
    }
    final_token
}

pub(super) fn capture_node_evidence(
    base_dir: &Path,
    seq: usize,
    step: usize,
    phase: &str,
    plan: &serde_json::Value,
    note: &str,
) -> Option<PathBuf> {
    let action = plan["action"].as_str().unwrap_or("unknown");
    let action_token = sanitize_filename_token(action);
    let phase_token = sanitize_filename_token(phase);
    let note_token = sanitize_filename_token(note);
    let file_name = format!(
        "node_{:03}_step_{:02}_{}_{}_{}.png",
        seq, step, action_token, phase_token, note_token
    );
    let full_path = base_dir.join(file_name);

    let status = std::process::Command::new("screencapture")
        .arg("-x")
        .arg(&full_path)
        .status();

    match status {
        Ok(s) if s.success() => {
            let front_app = current_platform()
                .frontmost_app_name()
                .ok()
                .flatten()
                .unwrap_or_else(|| "unknown".to_string());
            println!(
                "   📸 Node evidence: {} | step={} action={} phase={} front_app={} note={}",
                full_path.display(),
                step,
                action,
                phase,
                front_app,
                note
            );
            Some(full_path)
        }
        Ok(s) => {
            println!(
                "   ⚠️ Node evidence capture failed (exit={:?}) for step={} action={}",
                s.code(),
                step,
                action
            );
            None
        }
        Err(e) => {
            println!(
                "   ⚠️ Node evidence capture error for step={} action={}: {}",
                step, action, e
            );
            None
        }
    }
}
