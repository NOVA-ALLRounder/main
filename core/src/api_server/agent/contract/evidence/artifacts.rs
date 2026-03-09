use super::parsing::{
    current_run_scope_id, evidence_fields_match_run_scope, latest_evidence_field,
    latest_evidence_fields, latest_evidence_int, latest_legacy_mail_send_field,
    logs_have_evidence_fields, parse_evidence_fields, run_scoped_evidence_required,
};

#[derive(Debug, Clone)]
pub(crate) struct ArtifactEvidenceAssertion {
    pub(crate) key: &'static str,
    pub(crate) expected: String,
    pub(crate) actual: String,
    pub(crate) passed: bool,
    pub(crate) evidence: String,
}

pub(crate) fn detect_artifact_evidence_assertions(
    plan: &crate::nl_automation::Plan,
    logs: &[String],
) -> Vec<ArtifactEvidenceAssertion> {
    let plan_text = plan
        .steps
        .iter()
        .map(|step| step.description.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    let lowered_logs: Vec<String> = logs.iter().map(|line| line.to_lowercase()).collect();
    let joined_logs = lowered_logs.join("\n");

    let keyword_required =
        |keywords: &[&str]| -> bool { keywords.iter().any(|keyword| plan_text.contains(keyword)) };
    let marker_confirmed =
        |markers: &[&str]| -> bool { markers.iter().any(|marker| joined_logs.contains(marker)) };

    let mail_required = keyword_required(&["mail", "email", "이메일"]);
    let notes_required = keyword_required(&["notes", "note", "메모"]);
    let textedit_required = keyword_required(&["textedit", "텍스트편집", "text edit"]);
    let notion_required = keyword_required(&["notion", "노션"]);
    let telegram_required = keyword_required(&["telegram", "텔레그램"]);

    let mail_sent_confirmed =
        logs_have_evidence_fields(
            logs,
            &[
                ("target", "mail"),
                ("event", "send"),
                ("status", "sent_confirmed"),
            ],
        ) || marker_confirmed(&["mail_send_proof|status=sent_confirmed", "mail sent"]);
    let notes_write_confirmed = logs_have_evidence_fields(
        logs,
        &[
            ("target", "notes"),
            ("event", "write"),
            ("status", "confirmed"),
        ],
    ) || marker_confirmed(&[
        "notes_write_confirmed",
        "notes_write_text",
        "notes appended",
    ]);
    let textedit_write_confirmed = logs_have_evidence_fields(
        logs,
        &[
            ("target", "textedit"),
            ("event", "write"),
            ("status", "confirmed"),
        ],
    ) || marker_confirmed(&[
        "textedit_write_confirmed",
        "textedit_append_text",
        "shared via textedit",
    ]);
    let textedit_save_confirmed =
        logs_have_evidence_fields(
            logs,
            &[
                ("target", "textedit"),
                ("event", "save"),
                ("status", "confirmed"),
            ],
        ) || marker_confirmed(&["textedit_save_confirmed", "saved in textedit", "cmd+s"]);
    let notion_write_confirmed =
        logs_have_evidence_fields(
            logs,
            &[
                ("target", "notion"),
                ("event", "write"),
                ("status", "confirmed"),
            ],
        ) || marker_confirmed(&["notion: https://www.notion.so", "notion page created"]);
    let telegram_delivery_confirmed = logs_have_evidence_fields(
        logs,
        &[
            ("target", "telegram"),
            ("event", "send"),
            ("status", "sent"),
        ],
    ) || marker_confirmed(&["telegram: sent", "telegram sent"]);

    let notes_structured = latest_evidence_fields(logs, "notes", "write");
    let notes_note_id = notes_structured
        .as_ref()
        .and_then(|fields| fields.get("note_id").map(|v| v.trim().to_string()))
        .unwrap_or_default();
    let notes_body_len = latest_evidence_int(logs, "notes", "write", "body_len").unwrap_or(-1);
    let notes_note_id_required = notes_required && notes_structured.is_some();

    let textedit_write_structured = latest_evidence_fields(logs, "textedit", "write");
    let textedit_doc_id = textedit_write_structured
        .as_ref()
        .and_then(|fields| fields.get("doc_id").map(|v| v.trim().to_string()))
        .unwrap_or_default();
    let textedit_body_len =
        latest_evidence_int(logs, "textedit", "write", "body_len").unwrap_or(-1);
    let textedit_doc_id_required = textedit_required && textedit_write_structured.is_some();

    let run_scope_id = current_run_scope_id(logs);
    let scoped_evidence_count = logs
        .iter()
        .filter_map(|line| parse_evidence_fields(line))
        .filter(|fields| evidence_fields_match_run_scope(fields, run_scope_id.as_deref()))
        .count();
    let run_scope_required = run_scoped_evidence_required();
    let run_scope_present = run_scope_id.is_some();

    let mut out = vec![
        ArtifactEvidenceAssertion {
            key: "artifact.run_scope_present",
            expected: if run_scope_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: run_scope_present.to_string(),
            passed: if run_scope_required {
                run_scope_present
            } else {
                true
            },
            evidence: format!(
                "run_scope_required={} run_scope_id={} scoped_evidence_count={}",
                run_scope_required,
                run_scope_id.clone().unwrap_or_default(),
                scoped_evidence_count
            ),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.mail_sent_confirmed",
            expected: if mail_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: mail_sent_confirmed.to_string(),
            passed: if mail_required {
                mail_sent_confirmed
            } else {
                true
            },
            evidence: format!(
                "required={} marker_hint={} plan_keywords={}",
                mail_required,
                "EVIDENCE target=mail,event=send,status=sent_confirmed or legacy marker",
                plan_text
            ),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.notes_write_confirmed",
            expected: if notes_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: notes_write_confirmed.to_string(),
            passed: if notes_required {
                notes_write_confirmed
            } else {
                true
            },
            evidence: format!(
                "required={} marker_hint={} plan_keywords={}",
                notes_required,
                "EVIDENCE target=notes,event=write,status=confirmed or legacy marker",
                plan_text
            ),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.notes_note_id_present",
            expected: if notes_note_id_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: (!notes_note_id.is_empty()).to_string(),
            passed: if notes_note_id_required {
                !notes_note_id.is_empty()
            } else {
                true
            },
            evidence: format!(
                "structured_required={} note_id={}",
                notes_note_id_required, notes_note_id
            ),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.notes_body_nonempty",
            expected: if notes_required {
                ">2".to_string()
            } else {
                "optional".to_string()
            },
            actual: notes_body_len.to_string(),
            passed: if notes_required {
                notes_body_len > 2
            } else {
                true
            },
            evidence: "notes write body_len from EVIDENCE target=notes,event=write".to_string(),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.textedit_write_confirmed",
            expected: if textedit_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: textedit_write_confirmed.to_string(),
            passed: if textedit_required {
                textedit_write_confirmed
            } else {
                true
            },
            evidence: format!(
                "required={} marker_hint={} plan_keywords={}",
                textedit_required,
                "EVIDENCE target=textedit,event=write,status=confirmed or legacy marker",
                plan_text
            ),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.textedit_doc_id_present",
            expected: if textedit_doc_id_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: (!textedit_doc_id.is_empty()).to_string(),
            passed: if textedit_doc_id_required {
                !textedit_doc_id.is_empty()
            } else {
                true
            },
            evidence: format!(
                "structured_required={} doc_id={}",
                textedit_doc_id_required, textedit_doc_id
            ),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.textedit_body_nonempty",
            expected: if textedit_required {
                ">2".to_string()
            } else {
                "optional".to_string()
            },
            actual: textedit_body_len.to_string(),
            passed: if textedit_required {
                textedit_body_len > 2
            } else {
                true
            },
            evidence: "textedit write body_len from EVIDENCE target=textedit,event=write"
                .to_string(),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.textedit_save_confirmed",
            expected: if textedit_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: textedit_save_confirmed.to_string(),
            passed: if textedit_required {
                textedit_save_confirmed
            } else {
                true
            },
            evidence: format!(
                "required={} marker_hint={} plan_keywords={}",
                textedit_required,
                "EVIDENCE target=textedit,event=save,status=confirmed or legacy marker",
                plan_text
            ),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.notion_write_confirmed",
            expected: if notion_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: notion_write_confirmed.to_string(),
            passed: if notion_required {
                notion_write_confirmed
            } else {
                true
            },
            evidence: format!(
                "required={} marker_hint={} plan_keywords={}",
                notion_required,
                "EVIDENCE target=notion,event=write,status=confirmed or legacy marker",
                plan_text
            ),
        },
        ArtifactEvidenceAssertion {
            key: "artifact.telegram_delivery_confirmed",
            expected: if telegram_required {
                "true".to_string()
            } else {
                "optional".to_string()
            },
            actual: telegram_delivery_confirmed.to_string(),
            passed: if telegram_required {
                telegram_delivery_confirmed
            } else {
                true
            },
            evidence: format!(
                "required={} marker_hint={} plan_keywords={}",
                telegram_required,
                "EVIDENCE target=telegram,event=send,status=sent or legacy marker",
                plan_text
            ),
        },
    ];

    if mail_required {
        let mail_recipient = latest_evidence_field(logs, "mail", "send", "recipient")
            .or_else(|| latest_legacy_mail_send_field(logs, "recipient"))
            .unwrap_or_default();
        let recipient_present = !mail_recipient.trim().is_empty();
        out.push(ArtifactEvidenceAssertion {
            key: "artifact.mail_recipient_present",
            expected: "true".to_string(),
            actual: recipient_present.to_string(),
            passed: recipient_present,
            evidence: format!("mail_recipient={}", mail_recipient),
        });

        let mail_body_len = latest_evidence_int(logs, "mail", "send", "body_len").or_else(|| {
            latest_legacy_mail_send_field(logs, "body_len").and_then(|v| v.parse::<i64>().ok())
        });
        let body_len_value = mail_body_len.unwrap_or(-1);
        out.push(ArtifactEvidenceAssertion {
            key: "artifact.mail_body_nonempty",
            expected: ">2".to_string(),
            actual: body_len_value.to_string(),
            passed: body_len_value > 2,
            evidence: "mail body evidence from EVIDENCE/mail_send_proof".to_string(),
        });
    }

    let notion_structured = latest_evidence_fields(logs, "notion", "write");
    let notion_page_required = notion_required && notion_structured.is_some();
    let notion_page_id = notion_structured
        .as_ref()
        .and_then(|fields| fields.get("page_id").map(|v| v.trim().to_string()))
        .unwrap_or_default();
    out.push(ArtifactEvidenceAssertion {
        key: "artifact.notion_page_id_present",
        expected: if notion_page_required {
            "true".to_string()
        } else {
            "optional".to_string()
        },
        actual: (!notion_page_id.is_empty()).to_string(),
        passed: if notion_page_required {
            !notion_page_id.is_empty()
        } else {
            true
        },
        evidence: format!(
            "structured_required={} page_id={}",
            notion_page_required, notion_page_id
        ),
    });

    let telegram_structured = latest_evidence_fields(logs, "telegram", "send");
    let telegram_message_required = telegram_required && telegram_structured.is_some();
    let telegram_message_id = telegram_structured
        .as_ref()
        .and_then(|fields| fields.get("message_id").map(|v| v.trim().to_string()))
        .unwrap_or_default();
    out.push(ArtifactEvidenceAssertion {
        key: "artifact.telegram_message_id_present",
        expected: if telegram_message_required {
            "true".to_string()
        } else {
            "optional".to_string()
        },
        actual: (!telegram_message_id.is_empty()).to_string(),
        passed: if telegram_message_required {
            !telegram_message_id.is_empty()
        } else {
            true
        },
        evidence: format!(
            "structured_required={} message_id={}",
            telegram_message_required, telegram_message_id
        ),
    });

    out
}
