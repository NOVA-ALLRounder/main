use anyhow::Result;
use log::info;

use crate::controller::actions::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) fn mail_send_fresh_from_goal(
        goal: &str,
        history: &mut Vec<String>,
    ) -> Result<String> {
        if let Ok(removed) = Self::mail_cleanup_marker_outgoing(Some(goal), None) {
            if removed > 0 {
                info!(
                    "      📧 [MailDraft] cleaned stale marker drafts before fresh send: {}",
                    removed
                );
            }
        }
        let mut body_lines = Self::extract_quoted_fragments(goal)
            .into_iter()
            .filter(|s| s.len() >= 3)
            .collect::<Vec<_>>();
        if body_lines.is_empty() {
            return Err(anyhow::anyhow!(
                "no quoted payload available for fresh mail"
            ));
        }
        if let Some(marker) = Self::preferred_run_scope_marker(Some(goal)) {
            if !body_lines.iter().any(|line| line == &marker) {
                body_lines.push(marker);
            }
        }
        let subject = Self::preferred_mail_subject(Some(goal)).unwrap_or_else(|| {
            body_lines
                .first()
                .cloned()
                .unwrap_or_else(|| "Steer Auto Message".to_string())
        });
        let recipient = Self::preferred_mail_recipient(Some(goal)).unwrap_or_default();
        let body_text = body_lines.join("\n");
        let draft_goal = format!(
            "{} \"{}\" \"{}\" \"{}\"",
            goal, subject, recipient, body_text
        );
        let (draft_id, body_len) = Self::mail_create_filled_draft(Some(&draft_goal), &body_text)?;
        if body_len <= 2 {
            return Err(anyhow::anyhow!(
                "fresh draft body too short (draft_id={}, body_len={})",
                draft_id,
                body_len
            ));
        }
        Self::remember_mail_draft_id(history, &draft_id);
        let raw = Self::mail_send_latest_message(Some(&draft_goal), Some(draft_id.as_str()))?;
        let parsed = Self::parse_mail_send_result(&raw);
        if parsed.status != "sent_confirmed" {
            return Err(anyhow::anyhow!("fresh send blocked: {}", raw));
        }
        Ok(raw)
    }

    pub(in crate::controller::actions) fn mail_recover_body_with_fresh_draft(
        goal: &str,
        preferred_body: &str,
        history: &mut Vec<String>,
    ) -> Option<(String, i64, String)> {
        if !Self::bool_env_with_default("STEER_MAIL_ALLOW_FRESH_DRAFT_ON_BODY_FAILURE", true) {
            return None;
        }
        if Self::mail_fresh_recovery_used(history) {
            return None;
        }

        let mut body_text = preferred_body.trim().to_string();
        if body_text.len() < 3 {
            body_text = Self::mail_fallback_body_from_goal(goal);
        }
        if body_text.trim().len() < 3 {
            let quoted = Self::extract_quoted_fragments(goal)
                .into_iter()
                .filter(|s| s.len() >= 3)
                .collect::<Vec<_>>();
            if !quoted.is_empty() {
                body_text = quoted.join("\n");
            }
        }
        if body_text.trim().len() < 3 {
            return None;
        }

        let subject = Self::preferred_mail_subject(Some(goal)).unwrap_or_default();
        let recipient = Self::preferred_mail_recipient(Some(goal)).unwrap_or_default();
        let draft_goal = format!(
            "{} \"{}\" \"{}\" \"{}\"",
            goal, subject, recipient, body_text
        );
        let (draft_id, body_len) =
            Self::mail_create_filled_draft(Some(&draft_goal), body_text.as_str()).ok()?;
        let mut final_len = body_len;
        if final_len <= 2 {
            if let Ok((_, retry_len)) =
                Self::mail_append_body(body_text.as_str(), Some(draft_id.as_str()))
            {
                final_len = retry_len;
            }
        }
        if final_len <= 2 {
            return None;
        }

        if !subject.trim().is_empty() {
            let _ = Self::mail_set_subject(subject.as_str(), Some(draft_id.as_str()));
        }
        let _ = Self::mail_set_recipient_if_missing(Some(goal), Some(draft_id.as_str()));
        Self::remember_mail_draft_id(history, &draft_id);
        Self::mark_mail_fresh_recovery_used(history);
        Some((draft_id, final_len, "fresh_draft".to_string()))
    }
}
