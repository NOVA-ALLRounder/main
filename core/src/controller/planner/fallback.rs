use super::Planner;
use crate::platform::AppRole;

impl Planner {
    pub(super) fn fallback_plan_from_goal(
        goal: &str,
        history: &[String],
    ) -> Option<serde_json::Value> {
        if let Some(plan) = Self::fallback_n8n_workflow_goal(goal, history) {
            return Some(plan);
        }

        if let Some(plan) = Self::fallback_product_comparison_goal(goal, history) {
            return Some(plan);
        }

        if let Some(plan) = Self::fallback_ai_news_to_notion_goal(goal, history) {
            return Some(plan);
        }

        if let Some(plan) = Self::fallback_todo_summary_goal(goal, history) {
            return Some(plan);
        }

        Self::fallback_general_goal(goal, history)
    }

    pub(super) fn maybe_rewrite_mail_subject_before_paste(
        goal: &str,
        history: &[String],
        plan: &mut serde_json::Value,
    ) {
        let action = plan["action"].as_str().unwrap_or("");
        if !matches!(action, "paste" | "done" | "shortcut") {
            return;
        }

        let in_mail_context =
            match Self::last_opened_app(history) {
                Some(app) => Self::app_is_role(&app, AppRole::MailClient),
                None => false,
            } || Self::history_contains_opened_role_app(history, AppRole::MailClient);
        if !in_mail_context {
            return;
        }

        if Self::history_has_mail_subject(history) {
            return;
        }

        let is_cmd_n_shortcut = action == "shortcut"
            && plan["key"].as_str().map(|k| k.eq_ignore_ascii_case("n")) == Some(true)
            && plan["modifiers"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .any(|m| m.as_str().unwrap_or("").eq_ignore_ascii_case("command"))
                })
                .unwrap_or(false);
        if action == "shortcut" && !is_cmd_n_shortcut {
            return;
        }

        if let Some(subject) = Self::extract_mail_subject_from_goal(goal) {
            *plan = serde_json::json!({
                "action": "type",
                "app": Self::mail_client_app_name(),
                "text": subject
            });
            println!("   🔁 Rewrote action to set mail subject before paste.");
        }
    }

    pub(super) fn maybe_rewrite_shortcut_to_next_app(
        goal: &str,
        history: &[String],
        plan: &mut serde_json::Value,
    ) {
        if plan["action"].as_str() != Some("shortcut") {
            return;
        }
        if plan.get("app").and_then(|v| v.as_str()).is_some() {
            return;
        }

        let key = plan["key"].as_str().unwrap_or("").to_lowercase();
        let has_command = plan["modifiers"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .any(|m| m.as_str().unwrap_or("").eq_ignore_ascii_case("command"))
            })
            .unwrap_or(false);
        if key != "n" || !has_command {
            return;
        }

        let Some(last_opened) = Self::last_opened_app(history) else {
            return;
        };
        if !Self::has_recent_created_item(history) {
            return;
        }

        let ordered = Self::ordered_apps_in_goal(goal);
        let mut current_idx: Option<usize> = None;
        for (idx, app) in ordered.iter().enumerate() {
            if app.eq_ignore_ascii_case(&last_opened) {
                current_idx = Some(idx);
                break;
            }
        }
        let Some(idx) = current_idx else {
            return;
        };
        let Some(next_app) = ordered.get(idx + 1) else {
            return;
        };

        if Self::history_contains_opened_app(history, next_app) {
            return;
        }

        *plan = serde_json::json!({ "action": "open_app", "name": next_app });
        println!(
            "   🔁 Rewrote repeated Cmd+N shortcut to next app transition: {}",
            next_app
        );
    }

    pub(super) fn maybe_rewrite_redundant_new_item_shortcut(
        goal: &str,
        history: &[String],
        plan: &mut serde_json::Value,
    ) {
        if !Self::plan_is_cmd_n_shortcut(plan) {
            return;
        }

        let target_app = plan["app"]
            .as_str()
            .map(|v| v.to_string())
            .or_else(|| Self::last_opened_app(history));
        let Some(app_name) = target_app else {
            return;
        };
        if !Self::is_textual_app(&app_name) {
            return;
        }
        if !Self::history_has_recent_new_item_for_app(history, &app_name) {
            return;
        }

        if let Some(fallback_plan) = Self::fallback_plan_from_goal(goal, history) {
            if !Self::plan_is_cmd_n_shortcut(&fallback_plan) {
                let action = fallback_plan["action"].as_str().unwrap_or("").to_string();
                *plan = fallback_plan;
                println!(
                    "   🔁 Rewrote redundant Cmd+N to progress action: {} (app={})",
                    action, app_name
                );
                return;
            }
        }

        if Self::app_is_role(&app_name, AppRole::MailClient) {
            if Self::goal_requires_mail_send(goal) && !Self::history_has_mail_send_done(history) {
                if !Self::history_has_mail_body(history) {
                    *plan = serde_json::json!({ "action": "paste", "app": Self::mail_client_app_name() });
                    println!("   🔁 Rewrote redundant Cmd+N to paste (mail body pending).");
                } else {
                    *plan = serde_json::json!({
                        "action": "mail_send",
                        "app": Self::mail_client_app_name()
                    });
                    println!("   🔁 Rewrote redundant Cmd+N to mail_send (mail send pending).");
                }
            } else {
                *plan = serde_json::json!({ "action": "done" });
                println!("   🔁 Rewrote redundant Cmd+N to done (mail already satisfied).");
            }
            return;
        }

        *plan = serde_json::json!({ "action": "done" });
        println!(
            "   🔁 Rewrote redundant Cmd+N to done (new item already created in {}).",
            app_name
        );
    }
}
