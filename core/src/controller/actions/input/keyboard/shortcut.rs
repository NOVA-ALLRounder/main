use log::info;
use serde_json::json;

use crate::controller::heuristics;
use crate::platform::{current_platform, AppRole};
use crate::visual_driver::{SmartStep, UiAction, VisualDriver};

use crate::controller::actions::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) async fn handle_shortcut(
        plan: &serde_json::Value,
        goal: &str,
        driver: &mut VisualDriver,
        session: &mut crate::session_store::Session,
        history: &mut Vec<String>,
        description_out: &mut String,
        action_status_override_out: &mut Option<&'static str>,
        action_data_out: &mut Option<serde_json::Value>,
    ) {
        let description: String;
        let mut action_status_override = *action_status_override_out;
        let mut action_data = action_data_out.take();

        let raw_key = plan["key"].as_str().unwrap_or("").to_string();
        let raw_modifiers: Vec<String> = if let Some(arr) = plan["modifiers"].as_array() {
            arr.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        } else {
            Vec::new()
        };
        let (key, modifiers) = Self::normalize_shortcut_parts(&raw_key, &raw_modifiers);

        if key.is_empty() {
            description = "shortcut failed: empty key".to_string();
            action_status_override = Some("failed");
        } else {
            if let Some(app_name) = plan.get("app").and_then(|v| v.as_str()) {
                let _ = crate::tool_chaining::CrossAppBridge::switch_to_app(app_name);
                let _ = heuristics::ensure_app_focus(app_name, 5).await;
            } else if let Some(target_app) =
                Self::preferred_target_app_from_history("shortcut", plan, history)
            {
                let _ = heuristics::ensure_app_focus(&target_app, 5).await;
            }

            let is_cmd_n =
                key == "n" && modifiers.iter().any(|m| m.eq_ignore_ascii_case("command"));
            let is_cmd_shift_d = key == "d"
                && modifiers.iter().any(|m| m.eq_ignore_ascii_case("command"))
                && modifiers.iter().any(|m| m.eq_ignore_ascii_case("shift"));
            let is_cmd_s =
                key == "s" && modifiers.iter().any(|m| m.eq_ignore_ascii_case("command"));
            let front_app = current_platform()
                .frontmost_app_name()
                .ok()
                .flatten()
                .unwrap_or_default();
            let cmd_n_target_app =
                Self::resolve_shortcut_target_app("shortcut", plan, history, goal, &front_app);
            let cmd_n_single_fire_key = format!(
                "shortcut:{}:command+n:new_item",
                cmd_n_target_app.to_lowercase()
            );
            let cmd_n_history_skip = is_cmd_n
                && !cmd_n_target_app.is_empty()
                && Self::should_skip_redundant_cmd_n(history, &cmd_n_target_app);
            let cmd_n_session_skip = is_cmd_n
                && !cmd_n_target_app.is_empty()
                && Self::session_has_single_fire_new_item(session, &cmd_n_single_fire_key);
            let cmd_n_mail_draft_tracked = is_cmd_n
                && Self::app_has_role(&cmd_n_target_app, AppRole::MailClient)
                && Self::has_tracked_mail_draft(history);
            let cmd_n_redundant_skip =
                cmd_n_history_skip || cmd_n_session_skip || cmd_n_mail_draft_tracked;
            let (cmd_n_attempts, cmd_n_successes) = if is_cmd_n {
                Self::session_cmd_n_stats(session, &cmd_n_target_app)
            } else {
                (0, 0)
            };
            let cmd_n_attempt_guard_hit = is_cmd_n
                && !cmd_n_target_app.is_empty()
                && cmd_n_successes == 0
                && cmd_n_attempts >= Self::max_cmd_n_attempts_per_app();
            let cmd_n_recent_created = if is_cmd_n {
                Self::history_recent_cmd_n_created_count(
                    history,
                    &cmd_n_target_app,
                    Self::cmd_n_window_flood_history_window(),
                )
            } else {
                0
            };
            let cmd_n_flood_limit = if is_cmd_n {
                Self::cmd_n_window_flood_limit_for_app(&cmd_n_target_app)
            } else {
                Self::cmd_n_window_flood_limit()
            };
            let cmd_n_window_flood_hit = is_cmd_n && cmd_n_recent_created >= cmd_n_flood_limit;
            if is_cmd_n
                && (cmd_n_target_app.is_empty() || cmd_n_target_app.eq_ignore_ascii_case("unknown"))
            {
                description =
                    "Shortcut 'n' + [command] blocked (target app unresolved)".to_string();
                action_status_override = Some("failed");
                action_data = Some(json!({
                    "proof": "cmd_n_target_unknown_block",
                    "shortcut": "cmd+n",
                    "front_app": front_app
                }));
            } else if cmd_n_window_flood_hit {
                description = format!(
                    "Shortcut '{}' + {:?} blocked (window flood guard, recent_created={})",
                    key, modifiers, cmd_n_recent_created
                );
                action_status_override = Some("failed");
                action_data = Some(json!({
                    "proof": "cmd_n_window_flood_block",
                    "front_app": cmd_n_target_app,
                    "shortcut": "cmd+n",
                    "recent_created": cmd_n_recent_created,
                    "flood_limit": cmd_n_flood_limit
                }));
            } else if cmd_n_attempt_guard_hit {
                description = format!(
                    "Shortcut '{}' + {:?} blocked (cmd+n loop guard in {}, attempts={})",
                    key, modifiers, cmd_n_target_app, cmd_n_attempts
                );
                action_status_override = Some("failed");
                action_data = Some(json!({
                    "proof": "cmd_n_loop_guard_block",
                    "front_app": cmd_n_target_app,
                    "shortcut": "cmd+n",
                    "attempts": cmd_n_attempts
                }));
            } else if is_cmd_n && !cmd_n_target_app.is_empty() && cmd_n_redundant_skip {
                let skip_reason = if cmd_n_mail_draft_tracked {
                    "mail_draft_tracked"
                } else if cmd_n_session_skip {
                    "session_single_fire"
                } else {
                    "history_redundant"
                };
                description = format!(
                    "Shortcut '{}' + {:?} skipped (redundant new item in {})",
                    key, modifiers, cmd_n_target_app
                );
                action_status_override = Some("success");
                action_data = Some(json!({
                    "proof": "redundant_new_item_skip",
                    "front_app": cmd_n_target_app,
                    "shortcut": "cmd+n",
                    "reason": skip_reason
                }));
            } else if is_cmd_n && Self::app_has_role(&cmd_n_target_app, AppRole::MailClient) {
                Self::ensure_role_focus(AppRole::MailClient, 5).await;
                match Self::mail_ensure_draft(Some(goal), history) {
                    Ok(draft_id) => {
                        Self::remember_mail_draft_id(history, &draft_id);
                        description =
                            format!("Shortcut '{}' + {:?} (Created new item)", key, modifiers);
                        action_status_override = Some("success");
                        action_data = Some(json!({
                            "proof": "mail_draft_ready",
                            "front_app": Self::role_app_name(AppRole::MailClient)
                        }));
                    }
                    Err(e) => {
                        description = format!("shortcut cmd+n (mail) failed: {}", e);
                        action_status_override = Some("failed");
                    }
                }
            } else if is_cmd_shift_d && Self::app_has_role(&front_app, AppRole::MailClient) {
                let draft_id = Self::mail_current_draft_id(history);
                info!("      📧 [MailSend] shortcut-path draft_id={:?}", draft_id);
                match Self::mail_send_latest_message(Some(goal), draft_id.as_deref()) {
                    Ok(raw_result) => {
                        let send_result = Self::parse_mail_send_result(&raw_result);
                        let outgoing_after = send_result
                            .outgoing_after
                            .unwrap_or_else(|| Self::mail_outgoing_count().unwrap_or(-1));
                        let policy_error =
                            Self::enforce_mail_send_policy(Some(goal), &send_result).err();
                        if policy_error.is_none() && send_result.status == "sent_confirmed" {
                            description =
                                format!("Shortcut '{}' + {:?} (Mail sent)", key, modifiers);
                            action_status_override = Some("success");
                        } else if let Some(policy_err) = policy_error.as_ref() {
                            description = format!(
                                "Shortcut '{}' + {:?} (mail blocked by outbound policy: {})",
                                key, modifiers, policy_err
                            );
                            action_status_override = Some("failed");
                        } else {
                            description = format!(
                                "Shortcut '{}' + {:?} (mail send blocked: {})",
                                key, modifiers, raw_result
                            );
                            action_status_override = Some("failed");
                        }
                        action_data = Some(json!({
                            "proof": "mail_send",
                            "result": raw_result,
                            "send_status": send_result.status,
                            "outgoing_before": send_result.outgoing_before,
                            "outgoing_after": outgoing_after,
                            "recipient": send_result.recipient,
                            "subject": send_result.subject,
                            "draft_id": send_result.draft_id,
                            "body_len": send_result.body_len,
                            "outbound_policy_error": policy_error.as_ref().map(|e| e.to_string()).unwrap_or_default()
                        }));
                        println!(
                            "MAIL_SEND_PROOF|status={}|recipient={}|subject={}|body_len={}|draft_id={}",
                            send_result.status,
                            send_result.recipient,
                            send_result.subject,
                            send_result.body_len.unwrap_or(-1),
                            send_result.draft_id
                        );
                        Self::log_evidence(
                            "mail",
                            "send",
                            &[
                                ("status", send_result.status.clone()),
                                ("recipient", send_result.recipient.clone()),
                                ("subject", send_result.subject.clone()),
                                ("body_len", send_result.body_len.unwrap_or(-1).to_string()),
                                ("draft_id", send_result.draft_id.clone()),
                                (
                                    "outbound_policy",
                                    policy_error
                                        .as_ref()
                                        .map(|e| format!("blocked:{}", e))
                                        .unwrap_or_else(|| "pass".to_string()),
                                ),
                            ],
                        );
                        if policy_error.is_none()
                            && send_result.status == "sent_confirmed"
                            && !send_result.draft_id.trim().is_empty()
                        {
                            if let Ok(removed) = Self::mail_cleanup_marker_outgoing(
                                Some(goal),
                                Some(send_result.draft_id.as_str()),
                            ) {
                                if removed > 0 {
                                    Self::log_evidence(
                                        "mail",
                                        "cleanup",
                                        &[("removed", removed.to_string())],
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        description = format!(
                            "Shortcut '{}' + {:?} (mail send failed: {})",
                            key, modifiers, e
                        );
                        action_status_override = Some("failed");
                    }
                }
            } else {
                let step = SmartStep::new(
                    UiAction::KeyboardShortcut(key.clone(), modifiers.clone()),
                    "Shortcut",
                );
                driver.add_step(step);
                if is_cmd_n {
                    description =
                        format!("Shortcut '{}' + {:?} (Created new item)", key, modifiers);
                } else if is_cmd_s && Self::app_has_role(&front_app, AppRole::TextEditor) {
                    description = format!(
                        "Shortcut '{}' + {:?} ({} saved)",
                        key,
                        modifiers,
                        Self::role_app_name(AppRole::TextEditor)
                    );
                    action_data = Some(json!({
                        "proof": "textedit_save",
                        "front_app": Self::role_app_name(AppRole::TextEditor)
                    }));
                    Self::log_evidence("textedit", "save", &[("status", "confirmed".to_string())]);
                } else {
                    description = format!("Shortcut '{}' + {:?}", key, modifiers);
                }
            }
        }

        *description_out = description;
        *action_status_override_out = action_status_override;
        *action_data_out = action_data;
    }
}
