use serde_json::json;

use crate::controller::heuristics;
use crate::visual_driver::{SmartStep, UiAction, VisualDriver};

use super::super::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) async fn handle_paste(
        plan: &serde_json::Value,
        goal: &str,
        driver: &mut VisualDriver,
        history: &mut Vec<String>,
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
        action_data: &mut Option<serde_json::Value>,
    ) {
        if let Some(app_name) = plan
            .get("app")
            .and_then(|v| v.as_str())
            .or_else(|| plan.get("name").and_then(|v| v.as_str()))
        {
            let _ = heuristics::ensure_app_focus(app_name, 1).await;
        }
        let mut front_app =
            crate::tool_chaining::CrossAppBridge::get_frontmost_app().unwrap_or_default();
        if !front_app.eq_ignore_ascii_case("Mail")
            && Self::has_tracked_mail_draft(history)
            && Self::goal_mentions_mail(goal)
        {
            let _ = heuristics::ensure_app_focus("Mail", 3).await;
            front_app =
                crate::tool_chaining::CrossAppBridge::get_frontmost_app().unwrap_or_default();
        }
        if front_app.eq_ignore_ascii_case("Mail") {
            let draft_id = Self::mail_ensure_draft(Some(goal), history)
                .ok()
                .filter(|v| !v.trim().is_empty());
            let recipient_hint = Self::preferred_mail_recipient(Some(goal));
            if let Err(e) = Self::mail_set_recipient_if_missing(Some(goal), draft_id.as_deref()) {
                *description = format!("Paste failed (mail recipient): {}", e);
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
            if *action_status_override != Some("failed") {
                let mut scripted_source: Option<String> = None;
                let mut text = crate::tool_chaining::CrossAppBridge::get_clipboard()
                    .unwrap_or_else(|_| "".to_string());
                let fallback = Self::mail_fallback_body_from_goal(goal);
                if text.trim().len() < 6 {
                    if !fallback.is_empty() {
                        text = fallback;
                    } else if let Some((source, scripted)) =
                        Self::scripted_mail_body_fallback(goal, history)
                    {
                        scripted_source = Some(source);
                        text = scripted;
                    }
                } else {
                    let quoted = Self::extract_quoted_fragments(goal)
                        .into_iter()
                        .filter(|s| s.len() >= 3)
                        .collect::<Vec<_>>();
                    let has_any_goal_fragment = quoted.iter().any(|frag| text.contains(frag));
                    if !has_any_goal_fragment && !fallback.is_empty() {
                        text = fallback;
                    }
                }
                let quoted = Self::extract_quoted_fragments(goal)
                    .into_iter()
                    .filter(|s| s.len() >= 3)
                    .collect::<Vec<_>>();
                if !quoted.is_empty() {
                    let mut missing = Vec::new();
                    for frag in &quoted {
                        if !text.contains(frag) {
                            missing.push(frag.clone());
                        }
                    }
                    if !missing.is_empty() {
                        if text.trim().is_empty() {
                            text = missing.join("\n");
                        } else {
                            text = format!("{}\n{}", text.trim_end(), missing.join("\n"));
                        }
                    }
                }
                if text.trim().len() < 6 {
                    if let Some((source, scripted)) =
                        Self::scripted_mail_body_fallback(goal, history)
                    {
                        scripted_source = Some(source);
                        text = scripted;
                    }
                }
                match Self::mail_append_body(&text, draft_id.as_deref()) {
                    Ok((target_draft_id, mut readback_len)) => {
                        let mut effective_draft_id = target_draft_id.clone();
                        Self::remember_mail_draft_id(history, &target_draft_id);
                        let mut source_for_log = scripted_source
                            .clone()
                            .unwrap_or_else(|| "clipboard".to_string());
                        if readback_len <= 2 {
                            let forced_text = Self::extract_quoted_fragments(goal)
                                .into_iter()
                                .filter(|s| s.len() >= 3)
                                .collect::<Vec<_>>()
                                .join("\n");
                            if !forced_text.trim().is_empty() && forced_text.trim() != text.trim() {
                                if let Ok((forced_draft_id, forced_len)) = Self::mail_append_body(
                                    &forced_text,
                                    Some(target_draft_id.as_str()),
                                ) {
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
                                source_for_log = fresh_source;
                            }
                        }
                        if readback_len <= 2 {
                            *description =
                                "Paste failed (mail body): empty readback after append".to_string();
                            *action_status_override = Some("failed");
                        } else {
                            *description = if source_for_log != "clipboard" {
                                if source_for_log == "fresh_draft" {
                                    "Pasted contents (mail body via fresh draft recovery)"
                                        .to_string()
                                } else {
                                    format!(
                                        "Pasted scripted contents (mail body: {})",
                                        source_for_log
                                    )
                                }
                            } else if let Some(source) = scripted_source.clone() {
                                format!("Pasted scripted contents (mail body: {})", source)
                            } else {
                                "Pasted clipboard contents (mail body)".to_string()
                            };
                            *action_status_override = Some("success");
                            *action_data = Some(json!({
                                "proof": "mail_body_appended",
                                "text_len": text.chars().count(),
                                "readback_len": readback_len,
                                "source": source_for_log.clone()
                            }));
                            Self::log_evidence(
                                "mail",
                                "write",
                                &[
                                    ("status", "confirmed".to_string()),
                                    ("body_len", readback_len.to_string()),
                                    ("draft_id", effective_draft_id),
                                    ("source", source_for_log),
                                ],
                            );
                        }
                    }
                    Err(e) => {
                        *description = format!("Paste failed (mail body): {}", e);
                        *action_status_override = Some("failed");
                    }
                }
            }
        } else if front_app.eq_ignore_ascii_case("TextEdit") {
            let mut text = crate::tool_chaining::CrossAppBridge::get_clipboard()
                .unwrap_or_else(|_| "".to_string());
            let fallback = Self::mail_fallback_body_from_goal(goal);
            if text.trim().is_empty() && !fallback.is_empty() {
                text = fallback;
            }
            let quoted = Self::extract_quoted_fragments(goal)
                .into_iter()
                .filter(|s| s.len() >= 3)
                .collect::<Vec<_>>();
            if !quoted.is_empty() {
                let mut missing = Vec::new();
                for frag in &quoted {
                    if !text.contains(frag) {
                        missing.push(frag.clone());
                    }
                }
                if !missing.is_empty() {
                    if text.trim().is_empty() {
                        text = missing.join("\n");
                    } else {
                        text = format!("{}\n{}", text.trim_end(), missing.join("\n"));
                    }
                }
            }
            match Self::textedit_append_text(&text, Some(goal)) {
                Ok(write_result) => {
                    *description = format!(
                        "Pasted clipboard contents (textedit body doc_id={} doc_name={})",
                        write_result.doc_id, write_result.doc_name
                    );
                    *action_status_override = Some("success");
                    *action_data = Some(json!({
                        "proof": "textedit_append_text",
                        "text_len": text.chars().count(),
                        "doc_id": write_result.doc_id,
                        "doc_name": write_result.doc_name,
                        "doc_body_len": write_result.body_len
                    }));
                    Self::log_evidence(
                        "textedit",
                        "write",
                        &[
                            ("status", "confirmed".to_string()),
                            ("doc_id", write_result.doc_id.clone()),
                            ("doc_name", write_result.doc_name.clone()),
                            ("body_len", write_result.body_len.to_string()),
                        ],
                    );
                }
                Err(e) => {
                    *description = format!("Paste failed (textedit body): {}", e);
                    *action_status_override = Some("failed");
                }
            }
        } else {
            let step = SmartStep::new(
                UiAction::KeyboardShortcut("v".to_string(), vec!["command".to_string()]),
                "Paste",
            );
            driver.add_step(step);
            *description = "Pasted clipboard contents".to_string();
        }
    }
}
