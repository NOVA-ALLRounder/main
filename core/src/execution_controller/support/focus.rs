use super::*;
use crate::platform::app_role_primary_name;

pub(crate) fn step_requires_browser_focus(step_type: &StepType) -> bool {
    matches!(
        step_type,
        StepType::Fill | StepType::Click | StepType::Select | StepType::Extract
    )
}

pub(crate) fn is_browser_app(app_name: &str) -> bool {
    app_matches_role(current_platform().kind(), AppRole::Browser, app_name)
}

pub(crate) fn interrupt_guard_enabled() -> bool {
    std::env::var("STEER_INTERRUPT_GUARD")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(true)
}

pub(crate) fn should_guard_interrupt_for_step(step_type: &StepType) -> bool {
    matches!(
        step_type,
        StepType::Fill | StepType::Click | StepType::Select | StepType::Extract
    )
}

pub(crate) fn expected_front_app_for_step(step_data: &Value) -> Option<String> {
    step_data
        .get("app")
        .and_then(|v| v.as_str())
        .or_else(|| step_data.get("name").and_then(|v| v.as_str()))
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
}

#[derive(Debug, Default)]
pub(crate) struct FocusHandoffState {
    drift_events: usize,
    recovery_attempts: usize,
    recovered_events: usize,
    failed_events: usize,
}

pub(crate) fn parse_bool_env_with_default(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

pub(crate) fn focus_handoff_enabled() -> bool {
    parse_bool_env_with_default("STEER_EXEC_FOCUS_HANDOFF", true)
}

pub(crate) fn focus_handoff_finder_bridge_enabled() -> bool {
    parse_bool_env_with_default("STEER_EXEC_FOCUS_HANDOFF_FINDER_BRIDGE", true)
}

pub(crate) fn focus_handoff_retries() -> usize {
    std::env::var("STEER_EXEC_FOCUS_HANDOFF_RETRIES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .map(|v| v.clamp(1, 6))
        .unwrap_or(2)
}

pub(crate) fn focus_handoff_retry_ms() -> u64 {
    std::env::var("STEER_EXEC_FOCUS_HANDOFF_RETRY_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(|v| v.clamp(80, 1200))
        .unwrap_or(220)
}

pub(crate) fn user_activity_guard_enabled() -> bool {
    parse_bool_env_with_default("STEER_USER_ACTIVITY_GUARD_ENABLED", true)
}

pub(crate) fn user_activity_idle_resume_secs() -> u64 {
    std::env::var("STEER_USER_ACTIVITY_IDLE_RESUME_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(|v| v.clamp(5, 600))
        .unwrap_or(60)
}

pub(crate) fn user_activity_poll_ms() -> u64 {
    std::env::var("STEER_USER_ACTIVITY_POLL_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(|v| v.clamp(200, 5000))
        .unwrap_or(1000)
}

#[cfg(target_os = "macos")]
pub(crate) fn parse_idle_ns_from_ioreg(raw: &str) -> Option<u64> {
    for line in raw.lines() {
        if !line.contains("\"HIDIdleTime\"") {
            continue;
        }
        let value = line.split('=').nth(1)?.trim();
        if let Some(hex) = value.strip_prefix("0x") {
            if let Ok(ns) = u64::from_str_radix(hex.trim(), 16) {
                return Some(ns);
            }
        } else if let Some(first_token) = value.split_whitespace().next() {
            if let Ok(ns) = first_token.parse::<u64>() {
                return Some(ns);
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
pub(crate) fn system_idle_secs() -> Option<f64> {
    let output = Command::new("ioreg")
        .args(["-c", "IOHIDSystem"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    parse_idle_ns_from_ioreg(&text).map(|ns| ns as f64 / 1_000_000_000.0)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn system_idle_secs() -> Option<f64> {
    None
}

pub(crate) async fn wait_until_user_idle_if_active(
    logs: &mut Vec<String>,
    step_idx: usize,
    reason: &str,
) -> bool {
    if !user_activity_guard_enabled() {
        return false;
    }
    let resume_secs = user_activity_idle_resume_secs() as f64;
    let poll_ms = user_activity_poll_ms();
    let mut idle_secs = match system_idle_secs() {
        Some(v) => v,
        None => return false,
    };
    if idle_secs >= resume_secs {
        return false;
    }

    push_run_attempt(
        logs,
        "user_activity_pause",
        "waiting",
        &format!(
            "step={} reason={} idle_secs={:.1} resume_secs={}",
            step_idx + 1,
            reason,
            idle_secs,
            resume_secs as u64
        ),
    );
    logs.push(format!(
        "USER_ACTIVITY_PAUSE: step={} reason={} (idle {:.1}s < {}s). Waiting for user idle...",
        step_idx + 1,
        reason,
        idle_secs,
        resume_secs as u64
    ));

    let mut wait_loops = 0usize;
    loop {
        tokio::time::sleep(Duration::from_millis(poll_ms)).await;
        wait_loops += 1;
        idle_secs = system_idle_secs().unwrap_or(0.0);
        if idle_secs >= resume_secs {
            break;
        }
        if wait_loops.is_multiple_of(10) {
            logs.push(format!(
                "USER_ACTIVITY_WAITING: step={} idle={:.1}s target={}s",
                step_idx + 1,
                idle_secs,
                resume_secs as u64
            ));
        }
    }

    push_run_attempt(
        logs,
        "user_activity_pause",
        "resumed",
        &format!(
            "step={} reason={} idle_secs={:.1}",
            step_idx + 1,
            reason,
            idle_secs
        ),
    );
    logs.push(format!(
        "USER_ACTIVITY_RESUMED: step={} reason={} (idle {:.1}s >= {}s)",
        step_idx + 1,
        reason,
        idle_secs,
        resume_secs as u64
    ));
    true
}

pub(crate) fn app_matches_expected(front_app: &str, expected_app: &str) -> bool {
    let front = front_app.trim();
    let expected = expected_app.trim();
    if front.is_empty() || expected.is_empty() {
        return false;
    }
    if front.eq_ignore_ascii_case(expected) {
        return true;
    }
    let front_lower = front.to_ascii_lowercase();
    let expected_lower = expected.to_ascii_lowercase();
    front_lower.contains(&expected_lower) || expected_lower.contains(&front_lower)
}

pub(crate) async fn recover_expected_focus(
    logs: &mut Vec<String>,
    focus_state: &mut FocusHandoffState,
    step_idx: usize,
    expected_app: &str,
    front_before: &str,
) -> (bool, String) {
    focus_state.drift_events += 1;
    let retries = focus_handoff_retries();
    let retry_ms = focus_handoff_retry_ms();
    let use_finder_bridge = focus_handoff_finder_bridge_enabled();
    push_run_attempt(
        logs,
        "focus_handoff",
        "drift_detected",
        &format!(
            "step={} expected_app={} frontmost={}",
            step_idx + 1,
            expected_app,
            front_before
        ),
    );

    let mut last_front = front_before.to_string();
    for attempt in 1..=retries {
        focus_state.recovery_attempts += 1;
        push_run_attempt(
            logs,
            "focus_handoff",
            "recovering",
            &format!(
                "step={} attempt={}/{} target={}",
                step_idx + 1,
                attempt,
                retries,
                expected_app
            ),
        );

        let _ = heuristics::ensure_app_focus(expected_app, 3).await;
        if use_finder_bridge {
            let front_now = current_platform()
                .frontmost_app_name()
                .ok()
                .flatten()
                .unwrap_or_default();
            if !app_matches_expected(&front_now, expected_app) {
                let file_manager =
                    app_role_primary_name(current_platform().kind(), AppRole::FileManager);
                let _ = heuristics::ensure_app_focus(file_manager, 2).await;
                let _ = heuristics::ensure_app_focus(expected_app, 3).await;
            }
        }

        let front_after = current_platform()
            .frontmost_app_name()
            .ok()
            .flatten()
            .unwrap_or_default();
        last_front = front_after.clone();
        if app_matches_expected(&front_after, expected_app) {
            focus_state.recovered_events += 1;
            push_run_attempt(
                logs,
                "focus_handoff",
                "recovered",
                &format!(
                    "step={} attempt={} expected_app={} frontmost={}",
                    step_idx + 1,
                    attempt,
                    expected_app,
                    front_after
                ),
            );
            return (true, front_after);
        }
        tokio::time::sleep(Duration::from_millis(retry_ms)).await;
    }

    focus_state.failed_events += 1;
    push_run_attempt(
        logs,
        "focus_handoff",
        "failed",
        &format!(
            "step={} expected_app={} frontmost={} retries={}",
            step_idx + 1,
            expected_app,
            last_front,
            retries
        ),
    );
    (false, last_front)
}

pub(crate) async fn recover_browser_focus(
    logs: &mut Vec<String>,
    focus_state: &mut FocusHandoffState,
    step_idx: usize,
    front_before: &str,
) -> (bool, String) {
    focus_state.drift_events += 1;
    let browser_candidates = app_role_aliases(current_platform().kind(), AppRole::Browser);
    push_run_attempt(
        logs,
        "focus_handoff_browser",
        "recovering",
        &format!("step={} frontmost={}", step_idx + 1, front_before),
    );
    let mut last_front = front_before.to_string();
    for app in browser_candidates {
        focus_state.recovery_attempts += 1;
        let _ = heuristics::ensure_app_focus(app, 2).await;
        let front_after = current_platform()
            .frontmost_app_name()
            .ok()
            .flatten()
            .unwrap_or_default();
        last_front = front_after.clone();
        if is_browser_app(&front_after) {
            focus_state.recovered_events += 1;
            push_run_attempt(
                logs,
                "focus_handoff_browser",
                "recovered",
                &format!(
                    "step={} target={} frontmost={}",
                    step_idx + 1,
                    app,
                    front_after
                ),
            );
            return (true, front_after);
        }
    }
    focus_state.failed_events += 1;
    push_run_attempt(
        logs,
        "focus_handoff_browser",
        "failed",
        &format!("step={} frontmost={}", step_idx + 1, last_front),
    );
    (false, last_front)
}

pub(crate) fn push_focus_handoff_summary(logs: &mut Vec<String>, focus_state: &FocusHandoffState) {
    push_run_attempt(
        logs,
        "focus_handoff_summary",
        "done",
        &format!(
            "drift_events={},recovery_attempts={},recovered_events={},failed_events={}",
            focus_state.drift_events,
            focus_state.recovery_attempts,
            focus_state.recovered_events,
            focus_state.failed_events
        ),
    );
}
