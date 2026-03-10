use super::Planner;
use crate::platform::AppRole;

impl Planner {
    pub(super) fn scenario_mode_enabled() -> bool {
        matches!(
            std::env::var("STEER_SCENARIO_MODE").as_deref(),
            Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
        )
    }

    pub(super) fn history_contains_case_insensitive(history: &[String], needle: &str) -> bool {
        let needle_lower = needle.to_lowercase();
        history
            .iter()
            .any(|h| h.to_lowercase().contains(&needle_lower))
    }

    pub(super) fn goal_contains_any(goal_lower: &str, needles: &[&str]) -> bool {
        needles.iter().any(|needle| goal_lower.contains(needle))
    }

    pub(super) fn normalize_text_for_matching(text: &str) -> String {
        use unicode_normalization::UnicodeNormalization;

        text.nfkc().collect::<String>().to_lowercase()
    }

    pub(super) fn goal_requires_mail_send(goal: &str) -> bool {
        let lower = Self::normalize_text_for_matching(goal);
        let mentions_mail = Self::goal_mentions_app_role(&lower, AppRole::MailClient);
        let mentions_send =
            lower.contains("send") || lower.contains("보내") || lower.contains("발송");
        mentions_mail && mentions_send
    }

    pub(super) fn goal_requires_telegram_send(goal: &str) -> bool {
        let lower = goal.to_lowercase();
        let mentions_telegram = lower.contains("telegram") || lower.contains("텔레그램");
        let mentions_send = lower.contains("send")
            || lower.contains("보내")
            || lower.contains("발송")
            || lower.contains("전송");
        mentions_telegram && mentions_send
    }

    pub(super) fn goal_prefers_n8n_orchestration(goal: &str) -> bool {
        let route_product_research =
            Self::env_truthy_default("STEER_ROUTE_PRODUCT_RESEARCH_TO_N8N", false);
        route_product_research && Self::goal_targets_product_comparison_research(goal)
    }

    pub(super) fn goal_targets_n8n_workflow_request(goal: &str) -> bool {
        if Self::goal_prefers_n8n_orchestration(goal) {
            return true;
        }
        let lower = Self::normalize_text_for_matching(goal);
        if !lower.contains("n8n") {
            return false;
        }
        Self::goal_contains_any(
            &lower,
            &[
                "workflow",
                "워크플로우",
                "추천 기능",
                "create",
                "생성",
                "만들",
                "작성",
                "build",
                "구성",
            ],
        )
    }

    pub(super) fn goal_requires_n8n_execution(goal: &str) -> bool {
        if Self::goal_prefers_n8n_orchestration(goal) {
            return true;
        }
        let lower = Self::normalize_text_for_matching(goal);
        if !lower.contains("n8n") {
            return false;
        }
        Self::goal_contains_any(
            &lower,
            &[
                "execute",
                "run",
                "실행",
                "돌려",
                "트리거",
                "trigger",
                "start",
            ],
        )
    }

    pub(super) fn maybe_rewrite_click_visual_to_app_action(
        goal: &str,
        history: &[String],
        plan: &mut serde_json::Value,
    ) {
        if plan["action"].as_str() != Some("click_visual") {
            return;
        }

        let description = plan["description"].as_str().unwrap_or("").trim();
        if description.is_empty() {
            return;
        }

        let desc_lower = description.to_lowercase();
        let looks_like_app_switch = desc_lower.contains("dock")
            || desc_lower.contains("icon")
            || desc_lower.contains("앱")
            || desc_lower.contains("application");
        if !looks_like_app_switch {
            return;
        }

        let target_app = Self::extract_known_app_from_text(description)
            .or_else(|| Self::next_unopened_app_in_goal(goal, history));
        let Some(target_app) = target_app else {
            return;
        };

        if Self::history_contains_opened_app(history, target_app) {
            *plan = serde_json::json!({ "action": "switch_app", "app": target_app });
            println!(
                "   🔁 Rewrote click_visual dock/app action to switch_app: {}",
                target_app
            );
        } else {
            *plan = serde_json::json!({ "action": "open_app", "name": target_app });
            println!(
                "   🔁 Rewrote click_visual dock/app action to open_app: {}",
                target_app
            );
        }
    }

    pub(super) fn maybe_repair_open_app_missing_name(
        goal: &str,
        history: &[String],
        plan: &mut serde_json::Value,
    ) {
        if plan["action"].as_str() != Some("open_app") {
            return;
        }

        let has_name = plan["name"]
            .as_str()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        if has_name {
            return;
        }

        let mut inferred: Option<&'static str> = None;

        if inferred.is_none() {
            if let Some(app_text) = plan["app"].as_str() {
                inferred = Self::extract_known_app_from_text(app_text);
            }
        }

        if inferred.is_none() {
            if let Some(desc) = plan["description"].as_str() {
                inferred = Self::extract_known_app_from_text(desc);
            }
        }

        if inferred.is_none() {
            inferred = Self::next_unopened_app_in_goal(goal, history);
        }

        if inferred.is_none() {
            inferred = Self::ordered_apps_in_goal(goal).into_iter().next();
        }

        if let Some(app) = inferred {
            plan["name"] = serde_json::Value::String(app.to_string());
            println!("   🛠️ Repaired open_app missing name -> {}", app);
        }
    }

    pub(super) fn in_mail_context(history: &[String]) -> bool {
        (match Self::last_opened_app(history) {
            Some(app) => Self::app_is_role(&app, AppRole::MailClient),
            None => false,
        }) || Self::history_contains_opened_role_app(history, AppRole::MailClient)
    }

    pub(super) fn history_has_mail_body(history: &[String]) -> bool {
        Self::history_contains_case_insensitive(history, "(mail body)")
            || Self::history_contains_case_insensitive(
                history,
                "pasted clipboard contents (mail body)",
            )
    }

    pub(super) fn history_has_mail_send_done(history: &[String]) -> bool {
        let allow_pending_as_done =
            !Self::env_truthy_default("STEER_OUTBOUND_MAIL_REQUIRE_SENT_CONFIRMED", true);
        Self::history_contains_case_insensitive(history, "mail send completed")
            || (allow_pending_as_done
                && Self::history_contains_case_insensitive(
                    history,
                    "mail send queued (pending confirmation)",
                ))
            || Self::history_contains_case_insensitive(history, "(mail sent)")
            || (Self::history_contains_case_insensitive(
                history,
                "mail send blocked: sent_pending|",
            ) && Self::history_contains_case_insensitive(
                history,
                "mail send blocked: no_draft|0|0",
            ))
    }

    pub(super) fn maybe_rewrite_click_visual_mail_body(
        history: &[String],
        plan: &mut serde_json::Value,
    ) {
        if plan["action"].as_str() != Some("click_visual") {
            return;
        }
        if !Self::in_mail_context(history) {
            return;
        }

        let desc = plan["description"].as_str().unwrap_or("");
        let desc_lc = desc.to_lowercase();
        let is_mail_body_target = desc_lc.contains("message body")
            || desc_lc.contains("mail body")
            || desc_lc.contains("compose body")
            || desc_lc.contains("본문")
            || desc_lc.contains("메시지");
        if !is_mail_body_target {
            return;
        }

        *plan = serde_json::json!({
            "action": "paste",
            "app": Self::app_name_for_role(AppRole::MailClient)
        });
        println!("   🔁 Rewrote Mail body click_visual to deterministic paste.");
    }

    pub(super) fn maybe_rewrite_snapshot_to_progress_action(
        goal: &str,
        history: &[String],
        plan: &mut serde_json::Value,
    ) {
        if plan["action"].as_str() != Some("snapshot") {
            return;
        }

        if Self::in_mail_context(history) && Self::goal_requires_mail_send(goal) {
            if Self::history_has_mail_send_done(history) {
                *plan = serde_json::json!({ "action": "done" });
                println!("   🔁 Rewrote snapshot to done (Mail send already confirmed).");
                return;
            }
            if !Self::history_has_mail_body(history) {
                *plan = serde_json::json!({
                    "action": "paste",
                    "app": Self::app_name_for_role(AppRole::MailClient)
                });
                println!("   🔁 Rewrote snapshot to paste (Mail body pending).");
            } else {
                *plan = serde_json::json!({
                    "action": "mail_send",
                    "app": Self::app_name_for_role(AppRole::MailClient)
                });
                println!("   🔁 Rewrote snapshot to mail_send (Mail send pending).");
            }
            return;
        }

        if let Some(fallback_plan) = Self::fallback_plan_from_goal(goal, history) {
            let action = fallback_plan["action"].as_str().unwrap_or("").to_string();
            if action != "snapshot" {
                *plan = fallback_plan;
                println!(
                    "   🔁 Rewrote snapshot to fallback progress action: {}",
                    action
                );
            }
        }
    }

    pub(super) fn maybe_rewrite_open_app_to_pending_text_action(
        goal: &str,
        history: &[String],
        plan: &mut serde_json::Value,
    ) {
        if plan["action"].as_str() != Some("open_app") {
            return;
        }

        let target_app = plan["name"].as_str().unwrap_or("").trim();

        let Some(current_app) = Self::last_opened_app_from_history(history) else {
            return;
        };
        if !Self::is_textual_app(&current_app) {
            return;
        }

        if !target_app.is_empty() && !target_app.eq_ignore_ascii_case(&current_app) {
            return;
        }

        let has_pending_literal = Self::extract_quoted_fragments(goal)
            .into_iter()
            .any(|frag| {
                let trimmed = frag.trim();
                let lower = trimmed.to_lowercase();
                if trimmed.len() < 2
                    || lower.starts_with("cmd+")
                    || lower.starts_with("status:")
                    || lower.starts_with("run_scope_")
                {
                    return false;
                }
                !Self::history_contains_case_insensitive(history, trimmed)
            });
        if !has_pending_literal {
            return;
        }

        if let Some(fallback_plan) = Self::fallback_plan_from_goal(goal, history) {
            let action = fallback_plan["action"].as_str().unwrap_or("").to_string();
            if matches!(
                action.as_str(),
                "type" | "select_all" | "copy" | "paste" | "shortcut"
            ) {
                *plan = fallback_plan;
                println!(
                    "   🔁 Rewrote open_app to pending text-flow action: {} (current app: {})",
                    action, current_app
                );
            }
        }
    }

    pub(super) fn can_force_done_for_simple_goal(
        goal: &str,
        plan: &serde_json::Value,
        history: &[String],
    ) -> bool {
        if plan["action"].as_str() != Some("done") {
            return false;
        }

        let goal_lower = Self::normalize_text_for_matching(goal);
        let is_note_creation_goal = Self::goal_mentions_app_role(&goal_lower, AppRole::NotesApp)
            && (goal_lower.contains("새 메모")
                || goal_lower.contains("new note")
                || goal_lower.contains("new memo"));
        if !is_note_creation_goal {
            return false;
        }

        let complex_markers = [
            "입력", "type", "복사", "copy", "붙여", "paste", "보내", "send", "저장", "save", "cmd+",
        ];
        if complex_markers
            .iter()
            .any(|marker| goal_lower.contains(marker))
        {
            return false;
        }

        Self::last_opened_app_from_history(history)
            .is_some_and(|app| Self::app_is_role(&app, AppRole::NotesApp))
    }
}
