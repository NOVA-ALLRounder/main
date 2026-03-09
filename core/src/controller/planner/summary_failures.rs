use super::*;

impl Planner {
    fn is_shortcut_permission_failure(step: &SessionStep) -> bool {
        if step.status == "success" {
            return false;
        }
        if !step.action_type.eq_ignore_ascii_case("shortcut") {
            return false;
        }
        let desc = step.description.to_lowercase();
        desc.contains("not allowed to send keystrokes")
            || desc.contains("허용되지 않습니다")
            || desc.contains("osascript")
            || desc.contains("system events")
            || desc.contains("shortcut failed")
    }

    fn is_type_permission_failure(step: &SessionStep) -> bool {
        if step.status == "success" {
            return false;
        }
        if !step.action_type.eq_ignore_ascii_case("type") {
            return false;
        }
        let desc = step.description.to_lowercase();
        desc.contains("keystroke")
            || desc.contains("허용되지 않습니다")
            || desc.contains("(1002)")
            || desc.contains("native fallback timed out")
            || desc.contains("system events")
    }

    pub(super) fn is_benign_failed_step(
        step: &SessionStep,
        goal: &str,
        history: &[String],
    ) -> bool {
        if step.status == "success" {
            return false;
        }
        let comparison_recovery_done = Self::goal_targets_product_comparison_research(goal)
            && Self::history_contains_case_insensitive(
                history,
                "PRODUCT_COMPARISON_REPORT_EMITTED",
            )
            && Self::is_type_permission_failure(step);
        matches!(
            Self::step_mail_send_status(step).as_deref(),
            Some("sent_pending") | Some("no_draft")
        ) || Self::is_shortcut_permission_failure(step)
            || comparison_recovery_done
    }
}
