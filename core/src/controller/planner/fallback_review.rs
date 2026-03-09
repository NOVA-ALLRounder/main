use super::Planner;

impl Planner {
    pub(super) fn should_relax_review(reason: &str, notes: &str) -> bool {
        let text = format!("{} {}", reason.to_lowercase(), notes.to_lowercase());

        let strict_signals = [
            "full sequence",
            "entire sequence",
            "initial step",
            "only the first step",
            "only includes opening",
            "incomplete",
            "single step",
            "not the complete",
        ];
        let has_strict_signal = strict_signals.iter().any(|s| text.contains(s));

        let hard_blockers = [
            "danger",
            "unsafe",
            "impossible",
            "stuck in a loop",
            "does not relate",
            "not related",
            "before opening safari",
            "without ensuring safari is open",
        ];
        let has_hard_blocker = hard_blockers.iter().any(|s| text.contains(s));

        has_strict_signal && !has_hard_blocker
    }

    pub(super) fn goal_has_explicit_sequence(goal: &str) -> bool {
        let lower = goal.to_lowercase();
        lower.contains(" then ")
            || lower.contains("다음")
            || lower.contains("후")
            || lower.contains("이후")
            || lower.contains("next")
    }

    pub(super) fn goal_has_multi_app(goal: &str) -> bool {
        Self::ordered_apps_in_goal(goal).len() > 1
    }

    pub(super) fn should_accept_text_flow_after_type(
        plan: &serde_json::Value,
        history: &[String],
        reason: &str,
        notes: &str,
    ) -> bool {
        let action = plan["action"].as_str().unwrap_or("");
        if !matches!(action, "select_all" | "copy" | "paste") {
            return false;
        }

        let recent_typed = history.iter().rev().take(10).any(|h| {
            let lower = h.to_lowercase();
            lower.starts_with("typed '")
                || lower.contains("typed \"")
                || lower.contains("typed ")
                || lower.contains("read_result:")
        });
        if !recent_typed {
            return false;
        }

        let text = format!("{} {}", reason.to_lowercase(), notes.to_lowercase());
        let confirmation_only_signals = [
            "confirmation",
            "confirm",
            "visible",
            "evidence",
            "fully entered",
            "full content",
            "입력",
            "보이",
            "확인",
        ];
        let has_confirmation_signal = confirmation_only_signals
            .iter()
            .any(|s| text.contains(&s.to_lowercase()));

        let hard_blockers = [
            "danger",
            "unsafe",
            "impossible",
            "not related",
            "does not relate",
            "wrong app",
            "before opening",
        ];
        let has_hard_blocker = hard_blockers.iter().any(|s| text.contains(s));

        has_confirmation_signal && !has_hard_blocker
    }

    pub(super) fn should_accept_typing_after_new_item_shortcut(
        plan: &serde_json::Value,
        history: &[String],
        reason: &str,
        notes: &str,
    ) -> bool {
        if plan["action"].as_str() != Some("type") {
            return false;
        }

        let has_new_item_shortcut = history.iter().rev().take(8).any(|h| {
            let lower = h.to_lowercase();
            lower.contains("shortcut 'n'")
                && lower.contains("command")
                && (lower.contains("created new item") || lower.contains("shortcut"))
        });
        if !has_new_item_shortcut {
            return false;
        }

        let text = format!("{} {}", reason.to_lowercase(), notes.to_lowercase());
        let note_creation_signals = [
            "new note",
            "new item",
            "새 메모",
            "생성",
            "no evidence",
            "must ensure",
        ];
        let has_note_creation_signal = note_creation_signals
            .iter()
            .any(|s| text.contains(&s.to_lowercase()));

        let hard_blockers = [
            "danger",
            "unsafe",
            "impossible",
            "not related",
            "wrong app",
            "before opening",
        ];
        let has_hard_blocker = hard_blockers.iter().any(|s| text.contains(s));

        has_note_creation_signal && !has_hard_blocker
    }

    pub(super) fn history_contains_shortcut(history: &[String], key: &str) -> bool {
        let needle = format!("shortcut '{}'", key.to_lowercase());
        history.iter().any(|h| h.to_lowercase().contains(&needle))
    }
}
