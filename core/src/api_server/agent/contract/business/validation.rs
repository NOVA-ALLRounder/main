use std::collections::HashMap;

use super::super::evidence::{
    detect_artifact_evidence_assertions, latest_evidence_field, latest_evidence_fields,
    latest_evidence_int, latest_legacy_mail_send_field, logs_have_evidence_fields,
};
use super::summary::{slot_value, summary_contains_token};

pub(super) fn validate_intent_business_contract(
    plan: &crate::nl_automation::Plan,
    summary: Option<&str>,
    logs: &[String],
) -> Vec<String> {
    let mut issues = Vec::new();
    let summary_text = summary.unwrap_or("");
    let joined_logs = logs.join("\n").to_lowercase();
    let plan_text = plan
        .steps
        .iter()
        .map(|step| step.description.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let plan_text_lower = plan_text.to_lowercase();
    let contains_any = |haystack: &str, needles: &[&str]| -> bool {
        needles.iter().any(|needle| haystack.contains(needle))
    };
    let plan_data_lower = plan
        .steps
        .iter()
        .map(|step| step.data.to_string().to_lowercase())
        .collect::<Vec<_>>()
        .join("\n");
    let data_contains_any = |needles: &[&str]| -> bool { contains_any(&plan_data_lower, needles) };
    let step_matches = |app_needles: &[&str], action_needles: &[&str]| -> bool {
        plan.steps.iter().any(|step| {
            let desc = step.description.to_lowercase();
            let data = step.data.to_string().to_lowercase();
            let app_hit = app_needles
                .iter()
                .any(|needle| desc.contains(needle) || data.contains(needle));
            if !app_hit {
                return false;
            }
            action_needles
                .iter()
                .any(|needle| desc.contains(needle) || data.contains(needle))
        })
    };
    let expected_recipients = {
        let mut recipients = crate::semantic_contract::extract_expected_recipients(&plan_text);
        for value in plan.slots.values() {
            for candidate in crate::semantic_contract::extract_expected_recipients(value) {
                if !recipients
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(&candidate))
                {
                    recipients.push(candidate);
                }
            }
        }
        recipients
    };
    let mail_body_len = latest_evidence_int(logs, "mail", "send", "body_len").or_else(|| {
        latest_legacy_mail_send_field(logs, "body_len").and_then(|raw| raw.parse::<i64>().ok())
    });

    match plan.intent {
        crate::nl_automation::IntentType::FlightSearch => {
            let required = [
                ("from", slot_value(plan, "from")),
                ("to", slot_value(plan, "to")),
                ("date_start", slot_value(plan, "date_start")),
            ];
            for (key, value) in required {
                if let Some(v) = value {
                    let summary_ok = summary_contains_token(summary_text, &v);
                    let log_ok = joined_logs.contains(&format!("auto fill succeeded for {}", key))
                        || joined_logs.contains(&format!("filled {}=", key))
                        || joined_logs.contains(&format!("slot {}=", key));
                    if !summary_ok && !log_ok {
                        issues.push(format!("contract_missing_{}={}", key, v));
                    }
                }
            }
        }
        crate::nl_automation::IntentType::ShoppingCompare => {
            if let Some(product) = slot_value(plan, "product_name") {
                if !summary_contains_token(summary_text, &product) {
                    issues.push(format!("contract_missing_product_name={}", product));
                }
            }
            if summary_text.to_lowercase().contains("unknown") {
                issues.push("shopping_summary_contains_unknown".to_string());
            }
        }
        crate::nl_automation::IntentType::FormFill => {
            if let Some(purpose) = slot_value(plan, "form_purpose") {
                if !summary_contains_token(summary_text, &purpose) {
                    issues.push(format!("contract_missing_form_purpose={}", purpose));
                }
            }
            let has_fill_signal = joined_logs.contains("auto fill succeeded")
                || joined_logs.contains("auto input attempted")
                || joined_logs.contains("manual input required");
            if !has_fill_signal {
                issues.push("form_fill_execution_signal_missing".to_string());
            }
        }
        crate::nl_automation::IntentType::GenericTask => {
            let has_meaningful_summary = !summary_text.trim().is_empty();
            let has_step_signal = joined_logs.contains("step 1:")
                || joined_logs.contains("run_attempt|phase=execution_start");
            if !has_meaningful_summary && !has_step_signal {
                issues.push("generic_execution_signal_missing".to_string());
            }

            let mail_send_required =
                (contains_any(&plan_text_lower, &["mail", "email", "메일", "이메일"])
                    && contains_any(&plan_text_lower, &["send", "보내", "발송", "전송"]))
                    || data_contains_any(&["mail_send", "gmail_send", "\"action\":\"mail_send\""]);
            if mail_send_required {
                let mail_structured = latest_evidence_fields(logs, "mail", "send");
                let mail_sent_confirmed = logs_have_evidence_fields(
                    logs,
                    &[
                        ("target", "mail"),
                        ("event", "send"),
                        ("status", "sent_confirmed"),
                    ],
                ) || (mail_structured.is_none()
                    && contains_any(
                        &joined_logs,
                        &[
                            "mail_send_proof|status=sent_confirmed",
                            "mail send completed",
                            "(mail sent)",
                            "mail sent",
                        ],
                    ));
                if !mail_sent_confirmed {
                    issues.push("contract_missing_mail_send_confirmation".to_string());
                }

                let mail_recipient = latest_evidence_field(logs, "mail", "send", "recipient")
                    .or_else(|| latest_legacy_mail_send_field(logs, "recipient"))
                    .unwrap_or_default();
                if mail_sent_confirmed && mail_recipient.trim().is_empty() {
                    issues.push("contract_missing_mail_recipient_evidence".to_string());
                }

                for recipient in &expected_recipients {
                    let needle = recipient.to_lowercase();
                    if !joined_logs.contains(&needle)
                        && !summary_text.to_lowercase().contains(&needle)
                    {
                        issues.push(format!("contract_missing_mail_recipient={}", recipient));
                    }
                }

                if mail_sent_confirmed && matches!(mail_body_len, Some(len) if len <= 2) {
                    issues.push("contract_mail_body_empty".to_string());
                }
            }

            let notes_write_required = step_matches(
                &["notes", "note", "메모"],
                &["write", "append", "type", "작성", "입력", "기록", "붙여넣"],
            ) || data_contains_any(&[
                "notes_write_text",
                "\"action\":\"notes_write\"",
                "\"target\":\"notes\"",
            ]);
            if notes_write_required {
                let notes_structured = latest_evidence_fields(logs, "notes", "write");
                let notes_write_confirmed = logs_have_evidence_fields(
                    logs,
                    &[
                        ("target", "notes"),
                        ("event", "write"),
                        ("status", "confirmed"),
                    ],
                ) || (notes_structured.is_none()
                    && contains_any(
                        &joined_logs,
                        &[
                            "notes_write_confirmed",
                            "notes_write_text",
                            "(notes body)",
                            "notes appended",
                        ],
                    ));
                if !notes_write_confirmed {
                    issues.push("contract_missing_notes_write_confirmation".to_string());
                }
                if notes_write_confirmed && notes_structured.is_some() {
                    let note_id = latest_evidence_field(logs, "notes", "write", "note_id")
                        .unwrap_or_default();
                    if note_id.trim().is_empty() {
                        issues.push("contract_missing_notes_note_id".to_string());
                    }
                }
                let notes_body_len = latest_evidence_int(logs, "notes", "write", "body_len");
                if notes_write_confirmed && matches!(notes_body_len, Some(len) if len <= 2) {
                    issues.push("contract_notes_body_empty".to_string());
                }
            }

            let textedit_write_required = step_matches(
                &["textedit", "텍스트편집", "text edit"],
                &["write", "append", "type", "작성", "입력", "기록", "붙여넣"],
            ) || data_contains_any(&[
                "textedit_append_text",
                "\"action\":\"textedit_append_text\"",
                "\"target\":\"textedit\"",
            ]);
            if textedit_write_required {
                let textedit_structured = latest_evidence_fields(logs, "textedit", "write");
                let textedit_write_confirmed = logs_have_evidence_fields(
                    logs,
                    &[
                        ("target", "textedit"),
                        ("event", "write"),
                        ("status", "confirmed"),
                    ],
                ) || (textedit_structured.is_none()
                    && contains_any(
                        &joined_logs,
                        &[
                            "textedit_write_confirmed",
                            "textedit_append_text",
                            "(textedit body)",
                            "shared via textedit",
                        ],
                    ));
                if !textedit_write_confirmed {
                    issues.push("contract_missing_textedit_write_confirmation".to_string());
                }
                if textedit_write_confirmed && textedit_structured.is_some() {
                    let doc_id = latest_evidence_field(logs, "textedit", "write", "doc_id")
                        .unwrap_or_default();
                    if doc_id.trim().is_empty() {
                        issues.push("contract_missing_textedit_doc_id".to_string());
                    }
                }
                let textedit_body_len = latest_evidence_int(logs, "textedit", "write", "body_len");
                if textedit_write_confirmed && matches!(textedit_body_len, Some(len) if len <= 2) {
                    issues.push("contract_textedit_body_empty".to_string());
                }
            }

            let notion_write_required = (contains_any(&plan_text_lower, &["notion", "노션"])
                && contains_any(
                    &plan_text_lower,
                    &[
                        "write",
                        "create",
                        "append",
                        "작성",
                        "기록",
                        "저장",
                        "업데이트",
                    ],
                ))
                || data_contains_any(&[
                    "notion_create",
                    "notion_write",
                    "\"action\":\"notion_create\"",
                    "\"action\":\"notion_write\"",
                ]);
            if notion_write_required {
                let notion_structured = latest_evidence_fields(logs, "notion", "write");
                let notion_write_confirmed = logs_have_evidence_fields(
                    logs,
                    &[
                        ("target", "notion"),
                        ("event", "write"),
                        ("status", "confirmed"),
                    ],
                ) || (notion_structured.is_none()
                    && contains_any(
                        &joined_logs,
                        &[
                            "notion: https://www.notion.so",
                            "notion page created",
                            "notion_write_confirmed",
                        ],
                    ));
                if !notion_write_confirmed {
                    issues.push("contract_missing_notion_write_confirmation".to_string());
                }
                if notion_write_confirmed && notion_structured.is_some() {
                    let notion_page_id = latest_evidence_field(logs, "notion", "write", "page_id")
                        .unwrap_or_default();
                    if notion_page_id.trim().is_empty() {
                        issues.push("contract_missing_notion_page_id".to_string());
                    }
                }
            }

            let telegram_send_required =
                (contains_any(&plan_text_lower, &["telegram", "텔레그램"])
                    && contains_any(&plan_text_lower, &["send", "보내", "발송", "전송"]))
                    || data_contains_any(&[
                        "telegram_send",
                        "\"action\":\"telegram_send\"",
                        "\"type\":\"telegram\"",
                    ]);
            if telegram_send_required {
                let telegram_structured = latest_evidence_fields(logs, "telegram", "send");
                let telegram_send_confirmed = logs_have_evidence_fields(
                    logs,
                    &[
                        ("target", "telegram"),
                        ("event", "send"),
                        ("status", "sent"),
                    ],
                ) || (telegram_structured.is_none()
                    && contains_any(
                        &joined_logs,
                        &[
                            "telegram: sent",
                            "telegram sent",
                            "telegram_message_sent",
                            "telegram_send",
                        ],
                    ));
                if !telegram_send_confirmed {
                    issues.push("contract_missing_telegram_send_confirmation".to_string());
                }
                if telegram_send_confirmed && telegram_structured.is_some() {
                    let message_id = latest_evidence_field(logs, "telegram", "send", "message_id")
                        .unwrap_or_default();
                    if message_id.trim().is_empty() {
                        issues.push("contract_missing_telegram_message_id".to_string());
                    }
                }
            }

            let textedit_save_required =
                (contains_any(&plan_text_lower, &["textedit", "텍스트편집", "text edit"])
                    && contains_any(&plan_text_lower, &["save", "저장"]))
                    || data_contains_any(&[
                        "textedit_save",
                        "textedit_save_confirmed",
                        "\"app\":\"textedit\"",
                    ]);
            if textedit_save_required {
                let textedit_save_confirmed = logs_have_evidence_fields(
                    logs,
                    &[
                        ("target", "textedit"),
                        ("event", "save"),
                        ("status", "confirmed"),
                    ],
                ) || contains_any(
                    &joined_logs,
                    &[
                        "textedit_save_confirmed",
                        "saved in textedit",
                        "cmd+s",
                        "shortcut 's' + [\"command\"]",
                    ],
                );
                if !textedit_save_confirmed {
                    issues.push("contract_missing_textedit_save_confirmation".to_string());
                }
            }
        }
    }

    let mut explicit_assertions = crate::semantic_contract::extract_required_assertions(&plan_text);
    for slot_value in plan.slots.values() {
        for key in crate::semantic_contract::extract_required_assertions(slot_value) {
            if !explicit_assertions
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&key))
            {
                explicit_assertions.push(key);
            }
        }
    }
    if !explicit_assertions.is_empty() {
        let assertion_map: HashMap<String, bool> = detect_artifact_evidence_assertions(plan, logs)
            .into_iter()
            .map(|assertion| {
                let actual_true = assertion.actual.eq_ignore_ascii_case("true");
                (assertion.key.to_ascii_lowercase(), actual_true)
            })
            .collect();
        for required_key in explicit_assertions {
            let normalized = required_key.to_ascii_lowercase();
            match assertion_map.get(&normalized) {
                Some(true) => {}
                Some(false) => issues.push(format!(
                    "contract_required_assertion_failed={}",
                    required_key
                )),
                None => issues.push(format!(
                    "contract_required_assertion_unknown={}",
                    required_key
                )),
            }
        }
    }

    issues
}
