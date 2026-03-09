use serde_json::json;

use crate::applescript;
use crate::controller::heuristics;

use super::super::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) fn focus_recovery_max_retries() -> usize {
        std::env::var("STEER_FOCUS_RECOVERY_MAX_RETRIES")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .map(|v| v.min(10))
            .unwrap_or(2)
    }

    pub(in crate::controller::actions) fn focus_recovery_profile() -> &'static str {
        let raw = std::env::var("STEER_FOCUS_RECOVERY_PROFILE")
            .ok()
            .unwrap_or_else(|| "standard".to_string());
        match raw.trim().to_ascii_lowercase().as_str() {
            "aggressive" => "aggressive",
            _ => "standard",
        }
    }

    pub(in crate::controller::actions) async fn recover_focus_and_verify(
        target_app: &str,
        retries: usize,
    ) -> (bool, String, usize, Vec<String>) {
        let mut recovery_trace: Vec<String> = Vec::new();
        let mut attempts = 0usize;
        let mut front = crate::tool_chaining::CrossAppBridge::get_frontmost_app()
            .unwrap_or_default()
            .trim()
            .to_string();

        while !front.eq_ignore_ascii_case(target_app) && attempts < retries {
            attempts += 1;
            let _ = heuristics::ensure_app_focus(target_app, 3).await;
            front = crate::tool_chaining::CrossAppBridge::get_frontmost_app()
                .unwrap_or_default()
                .trim()
                .to_string();
            recovery_trace.push(format!("retry#{} front={}", attempts, front));
        }

        if !front.eq_ignore_ascii_case(target_app) && Self::focus_recovery_profile() == "aggressive"
        {
            if heuristics::try_close_front_dialog() {
                recovery_trace.push("dialog_closed".to_string());
            }
            let _ = heuristics::ensure_app_focus("Finder", 2).await;
            attempts += 1;
            let finder_front = crate::tool_chaining::CrossAppBridge::get_frontmost_app()
                .unwrap_or_default()
                .trim()
                .to_string();
            recovery_trace.push(format!("handoff_finder front={}", finder_front));

            let _ = heuristics::ensure_app_focus(target_app, 4).await;
            attempts += 1;
            front = crate::tool_chaining::CrossAppBridge::get_frontmost_app()
                .unwrap_or_default()
                .trim()
                .to_string();
            recovery_trace.push(format!("handoff_target front={}", front));

            if !front.eq_ignore_ascii_case(target_app) {
                let _ = applescript::activate_app(target_app);
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(260));
                front = crate::tool_chaining::CrossAppBridge::get_frontmost_app()
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                recovery_trace.push(format!("activate_app front={}", front));
            }
        }

        (
            front.eq_ignore_ascii_case(target_app),
            front,
            attempts,
            recovery_trace,
        )
    }

    pub(in crate::controller::actions) async fn stabilize_focus_for_action(
        action_type: &str,
        plan: &serde_json::Value,
        history: &[String],
        goal: &str,
    ) {
        if let Some(app_name) = plan
            .get("app")
            .and_then(|v| v.as_str())
            .or_else(|| plan.get("target_app").and_then(|v| v.as_str()))
        {
            let _ = heuristics::ensure_app_focus(app_name, 3).await;
            return;
        }

        let ui_sensitive = matches!(
            action_type,
            "snapshot"
                | "click_visual"
                | "read"
                | "type"
                | "shortcut"
                | "key"
                | "scroll"
                | "paste"
                | "copy"
                | "select_all"
                | "switch_app"
        );
        if !ui_sensitive {
            return;
        }

        if let Some(target_app) =
            Self::preferred_target_app_from_history(action_type, plan, history)
        {
            let front = crate::tool_chaining::CrossAppBridge::get_frontmost_app().ok();
            let strict_focus_actions = matches!(
                action_type,
                "type"
                    | "shortcut"
                    | "key"
                    | "paste"
                    | "copy"
                    | "select_all"
                    | "read"
                    | "read_clipboard"
            );
            let need_refocus = match front.as_deref() {
                Some(front_app) => {
                    !front_app.eq_ignore_ascii_case(&target_app)
                        && (Self::is_focus_noise_app(front_app) || strict_focus_actions)
                }
                None => true,
            };

            if need_refocus {
                let _ = heuristics::ensure_app_focus(&target_app, 3).await;
            }
            return;
        }

        if matches!(action_type, "snapshot" | "click_visual" | "read") {
            if let Some(target_app) = heuristics::goal_primary_app(goal) {
                let _ = heuristics::ensure_app_focus(target_app, 3).await;
            } else if heuristics::prefer_lucky_only(goal) {
                let _ = heuristics::ensure_app_focus("Safari", 2).await;
                let _ = heuristics::ensure_app_focus("Google Chrome", 2).await;
            }
        }
    }

    pub(in crate::controller::actions) async fn apply_focus_guard(
        focus_guard_required: bool,
        focus_guard_target: Option<&str>,
        action_type: &str,
        action_status_override: &mut Option<&'static str>,
        action_data: &mut Option<serde_json::Value>,
        description: &mut String,
    ) {
        if action_status_override.is_some() || !focus_guard_required {
            return;
        }
        let Some(target_app) = focus_guard_target else {
            return;
        };

        let retries = Self::focus_recovery_max_retries();
        let (focus_ok, front_after, attempts, recovery_trace) =
            Self::recover_focus_and_verify(target_app, retries).await;
        if focus_ok {
            if attempts > 0 {
                Self::put_action_data_field(
                    action_data,
                    "focus_recovery",
                    json!({
                        "status": "recovered",
                        "expected_app": target_app,
                        "front_app_after": front_after,
                        "attempts": attempts,
                        "profile": Self::focus_recovery_profile(),
                        "trace": recovery_trace
                    }),
                );
            }
            return;
        }

        let actual = if front_after.trim().is_empty() {
            "unknown".to_string()
        } else {
            front_after
        };
        if matches!(action_type, "open_app" | "switch_app") {
            *action_status_override = Some("success");
            Self::put_action_data_field(
                action_data,
                "focus_recovery",
                json!({
                    "status": "mismatch_non_blocking",
                    "expected_app": target_app,
                    "front_app_after": actual,
                    "attempts": attempts,
                    "max_retries": retries,
                    "profile": Self::focus_recovery_profile(),
                    "trace": recovery_trace
                }),
            );
            return;
        }

        let detail = format!(
            "focus_recovery_failed expected={} actual={} retries={}",
            target_app, actual, attempts
        );
        *description = format!("{} | {}", description, detail);
        *action_status_override = Some("failed");
        Self::put_action_data_field(
            action_data,
            "focus_recovery",
            json!({
                "status": "failed",
                "expected_app": target_app,
                "front_app_after": actual,
                "attempts": attempts,
                "max_retries": retries,
                "profile": Self::focus_recovery_profile(),
                "trace": recovery_trace
            }),
        );
    }
}
