use super::Planner;
use crate::platform::AppRole;

impl Planner {
    pub(super) fn fallback_general_goal(
        goal: &str,
        history: &[String],
    ) -> Option<serde_json::Value> {
        let goal_lower = goal.to_lowercase();
        let wants_downloads = goal_lower.contains("downloads") || goal_lower.contains("다운로드");
        let apps_in_goal = Self::ordered_apps_in_goal(goal);
        let current_app = Self::last_opened_app_from_history(history);

        // Keep file-manager -> Downloads progression explicit before jumping to later apps.
        if wants_downloads {
            let file_manager = Self::file_manager_app_name();
            let file_manager_opened = Self::history_contains_opened_app(history, file_manager);
            if !file_manager_opened {
                return Some(serde_json::json!({ "action": "open_app", "name": file_manager }));
            }

            let downloads_opened = Self::history_contains_case_insensitive(
                history,
                &format!("Opened Downloads folder in {}", file_manager),
            );
            if !downloads_opened {
                return Some(
                    serde_json::json!({ "action": "click_ref", "ref": "LeftSidebarDownloads", "app": file_manager }),
                );
            }
        }

        if let Some(app_name) = current_app.as_deref() {
            if Self::app_is_role(app_name, AppRole::Calendar)
                && Self::goal_requires_telegram_send(goal)
                && !Self::history_has_read_result(history)
            {
                return Some(serde_json::json!({
                    "action": "read",
                    "query": "오늘 일정의 핵심 항목을 짧게 요약"
                }));
            }
            if Self::app_is_role(app_name, AppRole::NotesApp) {
                let wants_textedit = apps_in_goal
                    .iter()
                    .any(|app| Self::app_is_role(app, AppRole::TextEditor));
                if wants_textedit {
                    let copied_from_notes =
                        Self::history_contains_case_insensitive(history, "Copied selection");
                    if copied_from_notes {
                        let last_notes_idx =
                            Self::last_history_index_opened_role_app(history, AppRole::NotesApp);
                        let last_textedit_idx =
                            Self::last_history_index_opened_role_app(history, AppRole::TextEditor);
                        let textedit_after_notes = match (last_notes_idx, last_textedit_idx) {
                            (Some(n_idx), Some(t_idx)) => t_idx > n_idx,
                            _ => false,
                        };
                        if !textedit_after_notes {
                            return Some(
                                serde_json::json!({ "action": "open_app", "name": Self::text_editor_app_name() }),
                            );
                        }
                    }
                }
            }

            let mentions_new_item = Self::goal_contains_any(
                &goal_lower,
                &[
                    "cmd+n",
                    "command+n",
                    "새 메모",
                    "새 문서",
                    "새 이메일",
                    "new note",
                    "new document",
                    "new email",
                    "new draft",
                ],
            );
            if mentions_new_item
                && !Self::history_contains_shortcut(history, "n")
                && (Self::app_is_role(app_name, AppRole::NotesApp)
                    || Self::app_is_role(app_name, AppRole::TextEditor)
                    || Self::app_is_role(app_name, AppRole::MailClient))
            {
                return Some(serde_json::json!({
                    "action": "shortcut",
                    "key": "n",
                    "modifiers": ["command"],
                    "app": app_name
                }));
            }

            if Self::app_is_role(app_name, AppRole::MailClient) {
                if let Some(subject) = Self::extract_mail_subject_from_goal(goal) {
                    if !Self::history_contains_case_insensitive(history, "(mail subject)") {
                        return Some(
                            serde_json::json!({ "action": "type", "text": subject, "app": Self::mail_client_app_name() }),
                        );
                    }
                }

                let mail_body_done =
                    Self::history_contains_case_insensitive(history, "(mail body)")
                        || Self::history_contains_case_insensitive(
                            history,
                            "pasted clipboard contents (mail body)",
                        );
                let wants_mail_paste = Self::goal_contains_any(
                    &goal_lower,
                    &["붙여넣", "paste", "cmd+v", "command+v"],
                );
                if wants_mail_paste && !mail_body_done {
                    return Some(
                        serde_json::json!({ "action": "paste", "app": Self::mail_client_app_name() }),
                    );
                }

                let mail_send_done = Self::history_has_mail_send_done(history);
                if Self::goal_requires_mail_send(goal) && !mail_send_done {
                    return Some(
                        serde_json::json!({ "action": "mail_send", "app": Self::mail_client_app_name() }),
                    );
                }
            }

            if Self::goal_requires_telegram_send(goal)
                && !Self::history_has_telegram_send_done(history)
            {
                let mail_ready = !Self::goal_requires_mail_send(goal)
                    || Self::history_has_mail_send_done(history);
                if mail_ready && Self::history_has_read_result(history) {
                    return Some(serde_json::json!({ "action": "telegram_send" }));
                }
            }

            if Self::is_textual_app(app_name) {
                let mail_subject = Self::extract_mail_subject_from_goal(goal);
                if !Self::app_is_role(app_name, AppRole::MailClient) {
                    let mut fragments: Vec<String> = Vec::new();
                    for fragment in Self::extract_goal_text_fragments(goal) {
                        let trimmed = fragment.trim();
                        let lower = trimmed.to_lowercase();
                        if trimmed.len() < 2
                            || lower.starts_with("cmd+")
                            || lower.starts_with("status:")
                            || Self::is_email_like(trimmed)
                        {
                            continue;
                        }

                        if let Some(subject) = mail_subject.as_deref() {
                            if trimmed.eq_ignore_ascii_case(subject) {
                                continue;
                            }
                        }
                        fragments.push(trimmed.to_string());
                    }

                    if Self::app_is_role(app_name, AppRole::NotesApp) && fragments.len() > 1 {
                        let combined = fragments.join("\n");
                        if !Self::history_contains_case_insensitive(history, &combined) {
                            return Some(serde_json::json!({
                                "action": "type",
                                "text": combined,
                                "app": app_name
                            }));
                        }
                    }

                    for trimmed in fragments {
                        if !Self::history_contains_case_insensitive(history, &trimmed) {
                            return Some(serde_json::json!({
                                "action": "type",
                                "text": trimmed,
                                "app": app_name
                            }));
                        }
                    }

                    if Self::goal_contains_any(
                        &goal_lower,
                        &["select all", "전체 선택", "cmd+a", "command+a"],
                    ) && !Self::history_contains_case_insensitive(
                        history,
                        "Selected all contents",
                    ) {
                        return Some(
                            serde_json::json!({ "action": "select_all", "app": app_name }),
                        );
                    }

                    if Self::goal_contains_any(&goal_lower, &["copy", "복사", "cmd+c", "command+c"])
                        && !Self::history_contains_case_insensitive(history, "Copied selection")
                    {
                        return Some(serde_json::json!({ "action": "copy", "app": app_name }));
                    }
                }

                if Self::goal_contains_any(&goal_lower, &["paste", "붙여넣", "cmd+v", "command+v"])
                    && !Self::history_contains_case_insensitive(history, "Pasted")
                {
                    if Self::goal_requires_mail_send(goal)
                        && !Self::app_is_role(app_name, AppRole::MailClient)
                    {
                        return Some(
                            serde_json::json!({ "action": "open_app", "name": Self::mail_client_app_name() }),
                        );
                    }
                    return Some(serde_json::json!({ "action": "paste", "app": app_name }));
                }
            }
        }

        if Self::goal_requires_telegram_send(goal) && !Self::history_has_telegram_send_done(history)
        {
            let mail_ready =
                !Self::goal_requires_mail_send(goal) || Self::history_has_mail_send_done(history);
            if mail_ready && Self::history_has_read_result(history) {
                return Some(serde_json::json!({ "action": "telegram_send" }));
            }
        }

        for app in &apps_in_goal {
            if !Self::history_contains_opened_app(history, app) {
                return Some(serde_json::json!({ "action": "open_app", "name": app }));
            }
        }

        if apps_in_goal.is_empty() {
            let fragments = Self::extract_goal_text_fragments(goal)
                .into_iter()
                .filter(|frag| {
                    let trimmed = frag.trim();
                    let lower = trimmed.to_lowercase();
                    trimmed.len() >= 3
                        && !lower.starts_with("cmd+")
                        && lower != "done"
                        && !lower.starts_with("status:")
                        && !Self::is_email_like(trimmed)
                })
                .collect::<Vec<_>>();
            if !fragments.is_empty() {
                let staging_app = Self::text_staging_app();
                if !Self::history_contains_opened_app(history, staging_app) {
                    return Some(serde_json::json!({ "action": "open_app", "name": staging_app }));
                }

                let typed_marker = if Self::app_is_role(staging_app, AppRole::NotesApp) {
                    "(notes body)"
                } else {
                    "(textedit body)"
                };
                if !Self::history_contains_case_insensitive(history, typed_marker) {
                    let combined = fragments.join("\n");
                    return Some(serde_json::json!({
                        "action": "type",
                        "text": combined,
                        "app": staging_app
                    }));
                }
            }
        }

        if !apps_in_goal.is_empty() {
            return Some(serde_json::json!({ "action": "done" }));
        }

        None
    }
}
