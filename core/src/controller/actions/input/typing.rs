use serde_json::json;
use tokio::time::{sleep, Duration};

use crate::controller::heuristics;
use crate::platform::{app_role_primary_name, current_platform, AppRole};
use crate::visual_driver::{SmartStep, UiAction, VisualDriver};

use super::super::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) async fn handle_type(
        plan: &serde_json::Value,
        goal: &str,
        driver: &mut VisualDriver,
        history: &mut Vec<String>,
        last_read_number: &mut Option<String>,
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
        action_data: &mut Option<serde_json::Value>,
    ) {
        let mut text = plan["text"].as_str().unwrap_or("").to_string();
        let mut forced_app = false;
        if let Some(app_name) = plan.get("app").and_then(|v| v.as_str()) {
            let _ = heuristics::ensure_app_focus(app_name, 1).await;
            forced_app = true;
        } else if let Some(target_app) =
            Self::preferred_target_app_from_history("type", plan, history)
        {
            let _ = heuristics::ensure_app_focus(&target_app, 2).await;
        }

        let looks_like_calc = Self::looks_like_calc_expression(&text);
        if !forced_app && looks_like_calc {
            if let Some(front) = current_platform().frontmost_app_name().ok().flatten() {
                if !Self::app_has_role(&front, AppRole::Calculator) {
                    let _ = heuristics::ensure_app_focus(
                        app_role_primary_name(current_platform().kind(), AppRole::Calculator),
                        3,
                    )
                    .await;
                }
            }
        }

        let front_app = current_platform()
            .frontmost_app_name()
            .ok()
            .flatten()
            .unwrap_or_default();
        let app_name = plan
            .get("app")
            .and_then(|v| v.as_str())
            .filter(|v| !v.trim().is_empty())
            .map(|v| v.to_string())
            .unwrap_or_else(|| front_app.clone());
        if !app_name.trim().is_empty() {
            if Self::app_has_role(&front_app, AppRole::Calculator) {
                let mut cleaned = text.replace(['×', 'x', 'X'], "*").replace(' ', "");

                if cleaned.chars().all(|c| c.is_ascii_digit()) {
                    if let Some(num) = last_read_number.as_ref() {
                        if num.contains('.') {
                            cleaned = num.clone();
                        }
                    }
                }

                if (cleaned.contains('*')
                    || cleaned.contains('+')
                    || cleaned.contains('-')
                    || cleaned.contains('/'))
                    && !cleaned.ends_with('=')
                {
                    cleaned.push('=');
                }
                text = cleaned;
            }

            if Self::app_has_role(&app_name, AppRole::MailClient) {
                let draft_id = Self::mail_ensure_draft(Some(goal), history)
                    .ok()
                    .filter(|v| !v.trim().is_empty());
                let recipient_hint = Self::preferred_mail_recipient(Some(goal));
                if let Err(e) = Self::mail_set_recipient_if_missing(Some(goal), draft_id.as_deref())
                {
                    *description = format!("Type failed (mail recipient): {}", e);
                    *action_status_override = Some("failed");
                } else if let Some(recipient) = recipient_hint {
                    let draft_for_log = draft_id.clone().unwrap_or_default();
                    Self::log_evidence(
                        "mail",
                        "write",
                        &[
                            ("status", "confirmed".to_string()),
                            ("recipient", recipient),
                            ("draft_id", draft_for_log),
                        ],
                    );
                }
                let subject_already_set = history
                    .iter()
                    .any(|h| h.to_lowercase().contains("(mail subject)"));
                let prefer_subject = heuristics::looks_like_subject(&text)
                    || (!subject_already_set && !text.contains('\n') && text.len() <= 120);
                if *action_status_override != Some("failed") && prefer_subject {
                    match Self::mail_set_subject(&text, draft_id.as_deref()) {
                        Ok(target_draft_id) => {
                            Self::remember_mail_draft_id(history, &target_draft_id);
                            *description = format!("Typed '{}' (mail subject)", text);
                            *action_data = Some(json!({
                                "proof": "mail_subject_set",
                                "text_len": text.chars().count()
                            }));
                            Self::log_evidence(
                                "mail",
                                "write",
                                &[
                                    ("status", "confirmed".to_string()),
                                    ("subject", text.clone()),
                                    ("draft_id", target_draft_id),
                                ],
                            );
                        }
                        Err(e) => {
                            *description = format!("Type failed (mail subject): {}", e);
                            *action_status_override = Some("failed");
                        }
                    }
                } else if *action_status_override != Some("failed") {
                    match Self::mail_append_body(&text, draft_id.as_deref()) {
                        Ok((target_draft_id, mut readback_len)) => {
                            let mut effective_draft_id = target_draft_id.clone();
                            let mut write_source = "append".to_string();
                            Self::remember_mail_draft_id(history, &target_draft_id);
                            if readback_len <= 2 {
                                let forced_text = Self::extract_quoted_fragments(goal)
                                    .into_iter()
                                    .filter(|s| s.len() >= 3)
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                if !forced_text.trim().is_empty()
                                    && forced_text.trim() != text.trim()
                                {
                                    if let Ok((forced_draft_id, forced_len)) =
                                        Self::mail_append_body(
                                            &forced_text,
                                            Some(target_draft_id.as_str()),
                                        )
                                    {
                                        Self::remember_mail_draft_id(history, &forced_draft_id);
                                        effective_draft_id = forced_draft_id;
                                        readback_len = forced_len;
                                    }
                                }
                            }
                            if readback_len <= 2 {
                                if let Some((fresh_draft_id, fresh_len, fresh_source)) =
                                    Self::mail_recover_body_with_fresh_draft(goal, &text, history)
                                {
                                    effective_draft_id = fresh_draft_id;
                                    readback_len = fresh_len;
                                    write_source = fresh_source;
                                }
                            }
                            if readback_len <= 2 {
                                *description =
                                    "Type failed (mail body): empty readback after append"
                                        .to_string();
                                *action_status_override = Some("failed");
                            } else {
                                *description = format!("Typed '{}' (mail body)", text);
                                *action_data = Some(json!({
                                    "proof": "mail_body_appended",
                                    "text_len": text.chars().count(),
                                    "readback_len": readback_len,
                                    "source": write_source
                                }));
                                Self::log_evidence(
                                    "mail",
                                    "write",
                                    &[
                                        ("status", "confirmed".to_string()),
                                        ("body_len", readback_len.to_string()),
                                        ("draft_id", effective_draft_id),
                                    ],
                                );
                            }
                        }
                        Err(e) => {
                            *description = format!("Type failed (mail body): {}", e);
                            *action_status_override = Some("failed");
                        }
                    }
                }
                if *action_status_override != Some("failed") {
                    *action_status_override = Some("success");
                }
            } else if Self::app_has_role(&app_name, AppRole::NotesApp) {
                if app_name.eq_ignore_ascii_case("Notion") {
                    Self::handle_notion_type_readback(
                        goal,
                        &text,
                        driver,
                        description,
                        action_status_override,
                        action_data,
                    )
                    .await;
                } else {
                    let mut write_text = text.clone();
                    if write_text.trim().is_empty() {
                        let quoted = Self::extract_quoted_fragments(goal)
                            .into_iter()
                            .filter(|s| {
                                s.len() >= 3
                                    && !s.to_lowercase().contains("status:")
                                    && !s.to_lowercase().contains("cmd+")
                                    && !s.to_uppercase().starts_with("RUN_SCOPE_")
                            })
                            .collect::<Vec<_>>();
                        if !quoted.is_empty() {
                            write_text = quoted.join("\n");
                        }
                    }
                    match Self::notes_write_text(&write_text, Some(goal)) {
                        Ok(result) => {
                            *description = format!("Typed '{}' (notes body)", write_text);
                            *action_data = Some(json!({
                                "proof": "notes_write_text",
                                "text_len": write_text.chars().count(),
                                "body_len": result.body_len
                            }));
                            *action_status_override = Some("success");
                            Self::log_evidence(
                                "notes",
                                "write",
                                &[
                                    ("status", "confirmed".to_string()),
                                    ("note_id", result.note_id),
                                    ("note_name", result.note_name),
                                    ("body_len", result.body_len.to_string()),
                                ],
                            );
                        }
                        Err(e) => {
                            let step =
                                SmartStep::new(UiAction::Type(write_text.clone()), "Typing Note");
                            driver.add_step(step);
                            *description = format!(
                                "Typed '{}' (notes body) [visual fallback: {}]",
                                write_text, e
                            );
                            *action_data = Some(json!({
                                "proof": "notes_write_visual_fallback",
                                "text_len": write_text.chars().count(),
                                "error": e.to_string()
                            }));
                            *action_status_override = Some("success");
                            let len_str = write_text.chars().count().to_string();
                            Self::log_evidence(
                                "notes",
                                "write",
                                &[
                                    ("status", "fallback".to_string()),
                                    ("note_id", "visual_fallback".to_string()),
                                    ("note_name", "Visual_Note".to_string()),
                                    ("body_len", len_str),
                                ],
                            );
                        }
                    }
                }
            } else if Self::app_has_role(&app_name, AppRole::TextEditor) {
                let step = SmartStep::new(UiAction::Type(text.clone()), "Typing TextEdit");
                driver.add_step(step);
                *description = format!("Typed '{}' (textedit body visual)", text);
                *action_status_override = Some("success");
                let len_str = text.chars().count().to_string();
                Self::log_evidence(
                    "textedit",
                    "write",
                    &[
                        ("status", "confirmed".to_string()),
                        ("doc_id", "visual_fallback".to_string()),
                        ("doc_name", "Visual_Doc".to_string()),
                        ("body_len", len_str),
                    ],
                );
            }
        }

        if !description.contains("(mail subject)")
            && !description.contains("(mail body)")
            && !description.contains("(notes body)")
            && !description.contains("(textedit body)")
            && *action_status_override != Some("failed")
        {
            let step = SmartStep::new(UiAction::Type(text.to_string()), "Typing");
            driver.add_step(step);
            *description = format!("Typed '{}'", text);
        }
    }

    async fn handle_notion_type_readback(
        goal: &str,
        text: &str,
        driver: &mut VisualDriver,
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
        action_data: &mut Option<serde_json::Value>,
    ) {
        let step = SmartStep::new(UiAction::Type(text.to_string()), "Typing Notion");
        driver.add_step(step);
        sleep(Duration::from_millis(220)).await;
        let readback = Self::capture_front_text_via_platform(true, 120, 180).await;
        let marker = Self::preferred_run_scope_marker(Some(goal)).unwrap_or_default();
        let expected = if !marker.trim().is_empty() {
            marker
        } else {
            text.lines()
                .find(|line| !line.trim().is_empty())
                .map(|line| line.trim().to_string())
                .unwrap_or_default()
        };
        let matched = !expected.is_empty() && readback.contains(&expected);
        if matched {
            *description = format!("Typed '{}' (notion body)", text);
            *action_status_override = Some("success");
            *action_data = Some(json!({
                "proof": "notion_readback",
                "text_len": text.chars().count(),
                "expected": expected,
                "readback_len": readback.chars().count()
            }));
        } else {
            *description = if expected.is_empty() {
                "Type failed (notion readback: empty expectation)".to_string()
            } else {
                format!(
                    "Type failed (notion readback missing marker: '{}')",
                    expected
                )
            };
            *action_status_override = Some("failed");
            *action_data = Some(json!({
                "proof": "notion_readback_failed",
                "text_len": text.chars().count(),
                "expected": expected,
                "readback_len": readback.chars().count()
            }));
        }
    }
}
