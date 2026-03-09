use super::super::*;

pub fn maybe_assume_approved_for_test(id: i64) -> Result<()> {
    if !env_flag("STEER_TEST_ASSUME_APPROVED") {
        return Err(anyhow!(
            "STEER_TEST_ASSUME_APPROVED=1 is required for approve_test path"
        ));
    }
    if !env_flag("STEER_N8N_MOCK") {
        return Err(anyhow!(
            "approve_test path requires STEER_N8N_MOCK=1 to prevent real workflow creation"
        ));
    }

    let rec =
        db::get_recommendation(id)?.ok_or_else(|| anyhow!("recommendation {} not found", id))?;
    if rec.status.eq_ignore_ascii_case("rejected") {
        return Err(anyhow!(
            "recommendation {} is rejected and cannot be assumed approved",
            id
        ));
    }
    if !rec.status.eq_ignore_ascii_case("approved") {
        db::update_recommendation_review_status(id, "approved")?;
        println!("🧪 [TEST] Assumed approval for recommendation {}.", id);
    }
    Ok(())
}

pub async fn approve_and_execute_recommendation(
    id: i64,
    llm_client: Option<Arc<dyn LLMClient>>,
) -> Result<ApprovalExecutionOutcome> {
    let rec =
        db::get_recommendation(id)?.ok_or_else(|| anyhow!("recommendation {} not found", id))?;

    if rec.status.eq_ignore_ascii_case("rejected") {
        return Err(anyhow!(
            "recommendation {} is rejected and cannot be approved",
            id
        ));
    }
    ensure_recommendation_ready_for_approval(&rec)?;

    let approved_now = !rec.status.eq_ignore_ascii_case("approved");
    if approved_now {
        db::update_recommendation_review_status(id, "approved")?;
    }

    let preexisting_workflow = rec
        .workflow_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter(|s| !s.starts_with("provisioning:"))
        .map(|s| s.to_string());

    let workflow_id = super::provision::execute_approved_recommendation(id, llm_client).await?;
    let reused_existing = preexisting_workflow
        .as_deref()
        .map(|existing| existing == workflow_id)
        .unwrap_or(false);

    Ok(ApprovalExecutionOutcome {
        workflow_id,
        approved_now,
        reused_existing,
    })
}
