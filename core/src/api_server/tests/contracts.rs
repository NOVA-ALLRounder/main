use super::*;

#[test]
fn business_contract_rejects_missing_mail_notion_telegram_evidence() {
    let plan = test_plan_with_descriptions(
        IntentType::GenericTask,
        &[
            "Mail에서 qed4950@gmail.com으로 결과를 보내세요.",
            "Notion에 요약을 작성하세요.",
            "텔레그램으로 전송하세요.",
        ],
    );
    let logs = vec![
        "Step 1: Open app".to_string(),
        "Summary: requested integrations done".to_string(),
    ];

    let (ok, detail) = evaluate_business_evidence(&plan, &logs);
    assert!(!ok);
    assert!(detail.contains("contract_missing_mail_send_confirmation"));
    assert!(detail.contains("contract_missing_mail_recipient=qed4950@gmail.com"));
    assert!(detail.contains("contract_missing_notion_write_confirmation"));
    assert!(detail.contains("contract_missing_telegram_send_confirmation"));
}

#[test]
fn business_contract_accepts_mail_notion_telegram_evidence_markers() {
    let plan = test_plan_with_descriptions(
        IntentType::GenericTask,
        &[
            "Mail에서 qed4950@gmail.com으로 결과를 보내세요.",
            "Notion에 요약을 작성하세요.",
            "텔레그램으로 전송하세요.",
        ],
    );
    let logs = vec![
        "MAIL_SEND_PROOF|status=sent_confirmed|recipient=qed4950@gmail.com|subject=Digest"
            .to_string(),
        "Notion: https://www.notion.so/abcd1234".to_string(),
        "telegram: sent".to_string(),
        "Summary: integrations completed with artifacts".to_string(),
    ];

    let (ok, detail) = evaluate_business_evidence(&plan, &logs);
    assert!(ok, "expected ok but got {}", detail);
}

#[test]
fn business_contract_accepts_structured_evidence_schema() {
    let plan = test_plan_with_descriptions(
        IntentType::GenericTask,
        &[
            "Mail에서 qed4950@gmail.com으로 결과를 보내세요.",
            "Notion에 요약을 작성하세요.",
            "텔레그램으로 전송하세요.",
            "TextEdit에 저장하세요.",
        ],
    );
    let logs = vec![
            "EVIDENCE|target=mail|event=send|status=sent_confirmed|recipient=qed4950@gmail.com|subject=Digest|body_len=120".to_string(),
            "EVIDENCE|target=notion|event=write|status=confirmed|page_id=abcd1234".to_string(),
            "EVIDENCE|target=telegram|event=send|status=sent|message_id=123".to_string(),
            "EVIDENCE|target=textedit|event=save|status=confirmed|doc_id=doc-1".to_string(),
            "Summary: integrations completed with structured evidence".to_string(),
        ];

    let (ok, detail) = evaluate_business_evidence(&plan, &logs);
    assert!(ok, "expected ok but got {}", detail);
}

#[test]
fn business_contract_rejects_structured_mail_without_recipient() {
    let plan = test_plan_with_descriptions(
        IntentType::GenericTask,
        &["Mail에서 qed4950@gmail.com으로 결과를 보내세요."],
    );
    let logs = vec![
            "EVIDENCE|target=mail|event=send|status=sent_confirmed|recipient=|subject=Digest|body_len=120".to_string(),
            "Summary: mail send done".to_string(),
        ];

    let (ok, detail) = evaluate_business_evidence(&plan, &logs);
    assert!(!ok);
    assert!(detail.contains("contract_missing_mail_recipient_evidence"));
}

#[test]
fn business_contract_rejects_structured_notion_telegram_without_ids() {
    let plan = test_plan_with_descriptions(
        IntentType::GenericTask,
        &["Notion에 요약을 작성하고 텔레그램으로 전송하세요."],
    );
    let logs = vec![
        "EVIDENCE|target=notion|event=write|status=confirmed|page_id=".to_string(),
        "EVIDENCE|target=telegram|event=send|status=sent|message_id=".to_string(),
        "Summary: integrations completed".to_string(),
    ];

    let (ok, detail) = evaluate_business_evidence(&plan, &logs);
    assert!(!ok);
    assert!(detail.contains("contract_missing_notion_page_id"));
    assert!(detail.contains("contract_missing_telegram_message_id"));
}

#[test]
fn business_contract_rejects_structured_notes_without_note_id() {
    let plan =
        test_plan_with_descriptions(IntentType::GenericTask, &["Notes에 TODO를 작성하세요."]);
    let logs = vec![
        "EVIDENCE|target=notes|event=write|status=confirmed|note_id=|body_len=20".to_string(),
        "Summary: notes write completed".to_string(),
    ];
    let (ok, detail) = evaluate_business_evidence(&plan, &logs);
    assert!(!ok);
    assert!(detail.contains("contract_missing_notes_note_id"));
}

#[test]
fn business_contract_rejects_structured_textedit_without_doc_id() {
    let plan =
        test_plan_with_descriptions(IntentType::GenericTask, &["TextEdit에 결과를 작성하세요."]);
    let logs = vec![
        "EVIDENCE|target=textedit|event=write|status=confirmed|doc_id=|body_len=42".to_string(),
        "Summary: textedit write completed".to_string(),
    ];
    let (ok, detail) = evaluate_business_evidence(&plan, &logs);
    assert!(!ok);
    assert!(detail.contains("contract_missing_textedit_doc_id"));
}

#[test]
fn business_contract_respects_explicit_semantic_assertions() {
    let plan = test_plan_with_descriptions(
        IntentType::GenericTask,
        &["semantic_assertions: [artifact.mail_sent_confirmed]"],
    );
    let logs = vec!["Summary: generic task completed".to_string()];
    let (ok, detail) = evaluate_business_evidence(&plan, &logs);
    assert!(!ok);
    assert!(detail.contains("contract_required_assertion_failed=artifact.mail_sent_confirmed"));
}

#[test]
fn business_contract_rejects_mail_send_with_empty_body_len() {
    let plan = test_plan_with_descriptions(
        IntentType::GenericTask,
        &["Mail에서 qed4950@gmail.com으로 결과를 보내세요."],
    );
    let logs = vec![
            "MAIL_SEND_PROOF|status=sent_confirmed|recipient=qed4950@gmail.com|subject=Digest|body_len=0".to_string(),
            "Summary: mail send done".to_string(),
        ];

    let (ok, detail) = evaluate_business_evidence(&plan, &logs);
    assert!(!ok);
    assert!(detail.contains("contract_mail_body_empty"));
}

#[test]
fn business_contract_rejects_textedit_save_missing() {
    let plan = test_plan_with_descriptions(
        IntentType::GenericTask,
        &["TextEdit에 결과를 작성하고 저장하세요."],
    );
    let logs = vec![
        "TEXTEDIT_WRITE_CONFIRMED|len=42".to_string(),
        "Summary: textedit write done".to_string(),
    ];

    let (ok, detail) = evaluate_business_evidence(&plan, &logs);
    assert!(!ok);
    assert!(detail.contains("contract_missing_textedit_save_confirmation"));
}

#[test]
fn completion_score_is_high_on_clean_success() {
    let score = compute_completion_score("completed", true, true, true, true, true, 0, 0);
    assert!(score.score >= 90, "score={}", score.score);
    assert!(score.pass);
    assert_eq!(score.label, "Excellent");
}

#[test]
fn completion_score_drops_on_failed_execution() {
    let score = compute_completion_score("error", true, false, false, false, false, 4, 3);
    assert!(score.score < 60, "score={}", score.score);
    assert!(!score.pass);
    assert_eq!(score.label, "Risky");
    assert!(score
        .reasons
        .iter()
        .any(|r| r.contains("final_status=error")));
}

#[test]
fn parse_resume_token_accepts_valid_shape() {
    let parsed = parse_resume_token("resume:plan-123:2:user_approved:1700000000")
        .expect("resume token should parse");
    assert_eq!(parsed.plan_id, "plan-123");
    assert_eq!(parsed.step_index, 2);
    assert_eq!(parsed.reason, "user_approved");
}

#[test]
fn parse_resume_token_rejects_invalid_prefix() {
    let err = parse_resume_token("token:plan-123:2:user_approved:1700000000")
        .expect_err("invalid prefix must fail");
    assert!(err.contains("prefix invalid"));
}

#[test]
fn parse_resume_token_rejects_invalid_step() {
    let err = parse_resume_token("resume:plan-123:x:user_approved:1700000000")
        .expect_err("non-numeric step must fail");
    assert!(err.contains("step index invalid"));
}

#[test]
fn parse_resume_token_rejects_empty_reason() {
    let err =
        parse_resume_token("resume:plan-123:2::1700000000").expect_err("empty reason must fail");
    assert!(err.contains("reason is empty"));
}
