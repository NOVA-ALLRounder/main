use super::*;

impl Planner {
    fn step_data_has_proof(step: &crate::session_store::SessionStep, proof: &str) -> bool {
        step.data
            .as_ref()
            .and_then(|d| d.get("proof"))
            .and_then(|v| v.as_str())
            .map(|v| v == proof)
            .unwrap_or(false)
    }

    fn parse_app_context_from_step(step: &crate::session_store::SessionStep) -> Option<String> {
        let description = step.description.trim();
        if let Some(rest) = description.strip_prefix("Opened app: ") {
            let app = rest.trim();
            if !app.is_empty() {
                return Some(app.to_lowercase());
            }
        }
        if let Some(rest) = description.strip_prefix("Switched to app: ") {
            let app = rest.trim();
            if !app.is_empty() {
                return Some(app.to_lowercase());
            }
        }
        None
    }

    pub(super) fn collect_business_evidence(
        session: &Session,
        history: &[String],
    ) -> RunGoalBusinessEvidence {
        let mut evidence = RunGoalBusinessEvidence::default();
        let mut current_app = Self::last_opened_app_from_history(history).map(|a| a.to_lowercase());
        let mut textedit_context_seen = current_app.as_deref() == Some("textedit");
        let mut sent_pending_step: Option<usize> = None;
        let mut no_draft_after_pending = false;

        for step in &session.steps {
            if let Some(send_status) = Self::step_mail_send_status(step) {
                match send_status.as_str() {
                    "sent_confirmed" => {
                        if Self::step_has_mail_send_confirmed(step) {
                            evidence.mail_send_confirmed = true;
                        }
                    }
                    "sent_pending" => sent_pending_step = Some(step.step_index),
                    "no_draft" => {
                        if sent_pending_step.is_some() {
                            no_draft_after_pending = true;
                        }
                    }
                    _ => {}
                }
            }

            if step.status == "success" {
                if let Some(app) = Self::parse_app_context_from_step(step) {
                    if app == "textedit" {
                        textedit_context_seen = true;
                    }
                    current_app = Some(app);
                }
            }

            if step.status != "success" {
                continue;
            }

            let desc = step.description.to_lowercase();
            if Self::step_has_mail_send_confirmed(step) {
                evidence.mail_send_confirmed = true;
            }
            if Self::step_data_has_proof(step, "notes_write_text") || desc.contains("(notes body)")
            {
                evidence.notes_write_confirmed = true;
            }
            if Self::step_data_has_proof(step, "textedit_append_text")
                || desc.contains("(textedit body)")
            {
                evidence.textedit_write_confirmed = true;
                textedit_context_seen = true;
            }

            if matches!(step.action_type.as_str(), "type" | "paste") {
                match current_app.as_deref() {
                    Some("notes") => evidence.notes_write_confirmed = true,
                    Some("textedit") => {
                        evidence.textedit_write_confirmed = true;
                        textedit_context_seen = true;
                    }
                    _ => {}
                }
            }

            let is_save_shortcut = matches!(step.action_type.as_str(), "shortcut" | "key" | "save")
                && desc.contains("shortcut 's'")
                && desc.contains("command");
            let strict_textedit_save_proof =
                Self::env_truthy_default("STEER_STRICT_TEXTEDIT_SAVE_PROOF", true);
            let has_textedit_save_proof = Self::step_data_has_proof(step, "textedit_save")
                || desc.contains("textedit saved")
                || desc.contains("saved file")
                || desc.contains("file saved");
            if has_textedit_save_proof
                || (!strict_textedit_save_proof
                    && is_save_shortcut
                    && (current_app.as_deref() == Some("textedit") || textedit_context_seen))
            {
                evidence.textedit_save_confirmed = true;
            }
        }

        if !evidence.mail_send_confirmed && no_draft_after_pending {
            evidence.mail_send_confirmed = true;
        }

        if !evidence.mail_send_confirmed
            && !Self::env_truthy_default("STEER_STRICT_MAIL_SEND_PROOF", true)
        {
            evidence.mail_send_confirmed =
                Self::history_contains_case_insensitive(history, "mail send completed")
                    || Self::history_contains_case_insensitive(history, "(mail sent)");
        }
        if !evidence.notes_write_confirmed {
            evidence.notes_write_confirmed =
                Self::history_contains_case_insensitive(history, "(notes body)")
                    || (Self::history_contains_case_insensitive(history, "opened app: notes")
                        && Self::history_contains_case_insensitive(history, "typed '"));
        }
        if !evidence.textedit_write_confirmed {
            evidence.textedit_write_confirmed =
                Self::history_contains_case_insensitive(history, "(textedit body)")
                    || (Self::history_contains_case_insensitive(history, "opened app: textedit")
                        && Self::history_contains_case_insensitive(history, "typed '"));
        }
        if !evidence.textedit_save_confirmed {
            let strict_textedit_save_proof =
                Self::env_truthy_default("STEER_STRICT_TEXTEDIT_SAVE_PROOF", true);
            evidence.textedit_save_confirmed = (!strict_textedit_save_proof
                && Self::history_contains_case_insensitive(history, "opened app: textedit")
                && Self::history_contains_shortcut(history, "s"))
                || Self::history_contains_case_insensitive(history, "textedit saved")
                || Self::history_contains_case_insensitive(history, "file saved");
        }

        evidence
    }

    pub(super) fn step_mail_send_status(
        step: &crate::session_store::SessionStep,
    ) -> Option<String> {
        step.data
            .as_ref()
            .and_then(|data| data.get("send_status"))
            .and_then(|v| v.as_str())
            .map(|v| v.to_string())
    }

    fn step_mail_send_body_len(step: &crate::session_store::SessionStep) -> Option<i64> {
        step.data
            .as_ref()
            .and_then(|data| data.get("body_len"))
            .and_then(|v| v.as_i64())
    }

    fn step_mail_send_recipient(step: &crate::session_store::SessionStep) -> Option<String> {
        step.data
            .as_ref()
            .and_then(|data| data.get("recipient"))
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    }

    fn step_has_mail_send_confirmed(step: &crate::session_store::SessionStep) -> bool {
        let strict_mail_send_proof = Self::env_truthy_default("STEER_STRICT_MAIL_SEND_PROOF", true);
        let allow_pending_as_done =
            !Self::env_truthy_default("STEER_OUTBOUND_MAIL_REQUIRE_SENT_CONFIRMED", true);
        let send_status = Self::step_mail_send_status(step);
        if matches!(send_status.as_deref(), Some("sent_confirmed"))
            || (allow_pending_as_done && matches!(send_status.as_deref(), Some("sent_pending")))
        {
            if !strict_mail_send_proof {
                return true;
            }
            let body_ok = Self::step_mail_send_body_len(step)
                .map(|len| len > 2)
                .unwrap_or(false);
            let recipient_ok = Self::step_mail_send_recipient(step)
                .map(|recipient| recipient.contains('@'))
                .unwrap_or(false);
            if body_ok && recipient_ok {
                return true;
            }
        }
        if strict_mail_send_proof {
            return false;
        }
        let desc = step.description.to_lowercase();
        desc.contains("mail send completed") || desc.contains("(mail sent)")
    }
}
