use crate::platform::{current_platform, AppRole};
use anyhow::Result;
use log::info;
use serde_json::json;

use crate::controller::actions::{ActionRunner, MailSendResult};

impl ActionRunner {
    pub(in crate::controller::actions) async fn handle_mail_send(
        goal: &str,
        history: &mut Vec<String>,
        description_out: &mut String,
        action_status_override_out: &mut Option<&'static str>,
        action_data_out: &mut Option<serde_json::Value>,
    ) {
        let description: String;
        let action_status_override: Option<&'static str>;
        let mut action_data = action_data_out.take();

        let front_app = current_platform()
            .frontmost_app_name()
            .ok()
            .flatten()
            .unwrap_or_default();
        if !Self::app_has_role(&front_app, AppRole::MailClient) {
            Self::ensure_role_focus(AppRole::MailClient, 3).await;
        }
        let draft_id = Self::mail_current_draft_id(history);
        info!("      📧 [MailSend] action-path draft_id={:?}", draft_id);
        match Self::mail_send_latest_message(Some(goal), draft_id.as_deref()) {
            Ok(raw_result) => {
                let mut send_result = Self::parse_mail_send_result(&raw_result);
                let mut result_raw = raw_result.clone();
                let mut fresh_recovery_used = false;
                let allow_fresh_recovery =
                    Self::bool_env_with_default("STEER_MAIL_ALLOW_FRESH_RECOVERY", false)
                        && (Self::is_test_mode_enabled()
                            || Self::preferred_run_scope_marker(Some(goal)).is_some());
                if allow_fresh_recovery
                    && send_result.status != "sent_confirmed"
                    && !Self::mail_fresh_recovery_used(history)
                    && matches!(
                        send_result.status.as_str(),
                        "missing_marker" | "empty_body" | "draft_not_found" | "no_draft"
                    )
                {
                    Self::mark_mail_fresh_recovery_used(history);
                    if let Ok(fresh_raw) = Self::mail_send_fresh_from_goal(goal, history) {
                        result_raw = fresh_raw;
                        send_result = Self::parse_mail_send_result(&result_raw);
                        fresh_recovery_used = true;
                    }
                }
                let outgoing_after = send_result
                    .outgoing_after
                    .unwrap_or_else(|| Self::mail_outgoing_count().unwrap_or(-1));
                let policy_error = Self::enforce_mail_send_policy(Some(goal), &send_result).err();
                let require_sent_confirmed =
                    Self::bool_env_with_default("STEER_OUTBOUND_MAIL_REQUIRE_SENT_CONFIRMED", true);
                if policy_error.is_none()
                    && (send_result.status == "sent_confirmed"
                        || (!require_sent_confirmed && send_result.status == "sent_pending"))
                {
                    description = if send_result.status == "sent_confirmed" {
                        "Mail send completed".to_string()
                    } else {
                        "Mail send queued (pending confirmation)".to_string()
                    };
                    action_status_override = Some("success");
                } else if let Some(policy_err) = policy_error.as_ref() {
                    description = format!("Mail send blocked by outbound policy: {}", policy_err);
                    action_status_override = Some("failed");
                } else {
                    description = format!("Mail send blocked: {}", result_raw);
                    action_status_override = Some("failed");
                }
                action_data = Some(json!({
                    "proof": "mail_send",
                    "result": result_raw,
                    "send_status": send_result.status,
                    "outgoing_before": send_result.outgoing_before,
                    "outgoing_after": outgoing_after,
                    "recipient": send_result.recipient,
                    "subject": send_result.subject,
                    "draft_id": send_result.draft_id,
                    "body_len": send_result.body_len,
                    "fresh_recovery_used": fresh_recovery_used,
                    "outbound_policy_error": policy_error.as_ref().map(|e| e.to_string()).unwrap_or_default()
                }));
                println!(
                    "MAIL_SEND_PROOF|status={}|recipient={}|subject={}|body_len={}|draft_id={}|fresh_recovery={}",
                    send_result.status,
                    send_result.recipient,
                    send_result.subject,
                    send_result.body_len.unwrap_or(-1),
                    send_result.draft_id,
                    fresh_recovery_used
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
                        ("fresh_recovery", fresh_recovery_used.to_string()),
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
                description = format!("mail_send failed: {}", e);
                action_status_override = Some("failed");
            }
        }

        *description_out = description;
        *action_status_override_out = action_status_override;
        *action_data_out = action_data;
    }

    pub(in crate::controller::actions) fn parse_mail_send_result(raw: &str) -> MailSendResult {
        let mut parts = raw.trim().split('|');
        MailSendResult {
            status: parts.next().unwrap_or("").trim().to_string(),
            outgoing_before: parts.next().and_then(|v| v.trim().parse::<i64>().ok()),
            outgoing_after: parts.next().and_then(|v| v.trim().parse::<i64>().ok()),
            recipient: parts.next().unwrap_or("").trim().to_string(),
            subject: parts.next().unwrap_or("").trim().to_string(),
            draft_id: parts.next().unwrap_or("").trim().to_string(),
            body_len: parts.next().and_then(|v| v.trim().parse::<i64>().ok()),
        }
    }

    pub(in crate::controller::actions) fn enforce_mail_send_policy(
        goal: Option<&str>,
        send_result: &MailSendResult,
    ) -> Result<()> {
        crate::outbound_policy::enforce_mail_send_policy(
            goal,
            &send_result.recipient,
            &send_result.subject,
            send_result.body_len,
            &send_result.status,
        )
        .map_err(|e| anyhow::anyhow!(e))
    }
}
