use super::super::*;

pub async fn execute_approved_recommendation(
    id: i64,
    llm_client: Option<Arc<dyn LLMClient>>,
) -> Result<String> {
    execute_approved_recommendation_internal(id, llm_client, None).await
}

pub async fn execute_approved_recommendation_with_preclaim(
    id: i64,
    llm_client: Option<Arc<dyn LLMClient>>,
    preclaim: PreclaimedProvisioning,
) -> Result<String> {
    execute_approved_recommendation_internal(id, llm_client, Some(preclaim)).await
}

async fn execute_approved_recommendation_internal(
    id: i64,
    llm_client: Option<Arc<dyn LLMClient>>,
    preclaim: Option<PreclaimedProvisioning>,
) -> Result<String> {
    let rec =
        db::get_recommendation(id)?.ok_or_else(|| anyhow!("recommendation {} not found", id))?;
    let workflow_prompt = workflow_generation_prompt(&rec);

    if rec.status.eq_ignore_ascii_case("rejected") {
        return Err(anyhow!(
            "recommendation {} is rejected and cannot be created",
            id
        ));
    }
    if !rec.status.eq_ignore_ascii_case("approved") {
        return Err(anyhow!(
            "recommendation {} is '{}' (approval required before creation)",
            id,
            rec.status
        ));
    }
    ensure_recommendation_ready_for_approval(&rec)?;

    let force_recreate = preclaim
        .as_ref()
        .map(|p| p.force_recreate)
        .unwrap_or_else(|| env_flag("STEER_APPROVE_FORCE_RECREATE"));
    let claim_token = if force_recreate {
        None
    } else {
        preclaim
            .as_ref()
            .and_then(|p| p.claim_token.clone())
            .or_else(|| Some(make_claim_token(id)))
    };

    if !force_recreate {
        if let Some(existing_id) = rec
            .workflow_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if existing_id.starts_with("provisioning:") {
                let our_token = claim_token
                    .as_deref()
                    .ok_or_else(|| anyhow!("missing provisioning claim token"))?;
                if existing_id != our_token {
                    if is_stale_provisioning_claim(existing_id) {
                        let _ = db::release_recommendation_provisioning_claim(id, existing_id);
                    } else {
                        return Err(anyhow!(
                            "recommendation {} is already being provisioned ({})",
                            id,
                            existing_id
                        ));
                    }
                }
            }
            if !existing_id.starts_with("provisioning:") {
                if let Some(pre) = preclaim.as_ref() {
                    let _ = db::commit_workflow_provision_success(
                        pre.provision_op_id,
                        id,
                        existing_id,
                        rec.workflow_json.as_deref(),
                    );
                }
                let existing_json = rec
                    .workflow_json
                    .as_deref()
                    .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
                if let Err(error) = activate_and_auto_trigger_workflow(
                    id,
                    existing_id,
                    existing_json.as_ref(),
                    Some(&workflow_prompt),
                )
                .await
                {
                    if let Some(pre) = preclaim.as_ref() {
                        let _ = db::mark_workflow_provision_failed(
                            pre.provision_op_id,
                            &format!("existing workflow auto-trigger failed: {}", error),
                        );
                    }
                    let _ = db::mark_recommendation_failed(
                        id,
                        &format!("existing workflow auto-trigger failed: {}", error),
                    );
                    return Err(anyhow!(
                        "existing workflow auto-trigger failed: recommendation={} workflow_id={} error={}",
                        id,
                        existing_id,
                        error
                    ));
                }
                println!(
                    "ℹ️ Recommendation {} already provisioned. Reusing workflow_id={}",
                    id, existing_id
                );
                return Ok(existing_id.to_string());
            }
        }

        let token = claim_token
            .as_deref()
            .ok_or_else(|| anyhow!("missing provisioning claim token"))?;
        if let Some(existing) = claim_or_get_existing(id, token)? {
            if let Some(pre) = preclaim.as_ref() {
                let _ = db::commit_workflow_provision_success(
                    pre.provision_op_id,
                    id,
                    &existing,
                    rec.workflow_json.as_deref(),
                );
            }
            let existing_json = rec
                .workflow_json
                .as_deref()
                .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
            if let Err(error) = activate_and_auto_trigger_workflow(
                id,
                &existing,
                existing_json.as_ref(),
                Some(&workflow_prompt),
            )
            .await
            {
                if let Some(pre) = preclaim.as_ref() {
                    let _ = db::mark_workflow_provision_failed(
                        pre.provision_op_id,
                        &format!("existing workflow auto-trigger failed: {}", error),
                    );
                }
                let _ = db::mark_recommendation_failed(
                    id,
                    &format!("existing workflow auto-trigger failed: {}", error),
                );
                return Err(anyhow!(
                    "existing workflow auto-trigger failed: recommendation={} workflow_id={} error={}",
                    id,
                    existing,
                    error
                ));
            }
            println!(
                "ℹ️ Recommendation {} already provisioned. Reusing workflow_id={}",
                id, existing
            );
            return Ok(existing);
        }
    }

    let provision_op_id = match preclaim.as_ref() {
        Some(p) => p.provision_op_id,
        None => db::create_workflow_provision_op(id, claim_token.as_deref())?,
    };
    mark_provision_progress(provision_op_id, "provisioning_started");

    let workflow_json_str_result: Result<String> = async {
        mark_provision_progress(provision_op_id, "preparing_workflow_json");
        if should_use_test_mock_workflow() {
            return serde_json::to_string(&mock_workflow_json(&rec.title))
                .map_err(|e| anyhow!("mock workflow serialization failed: {}", e));
        }

        if let Some(existing_json) = rec
            .workflow_json
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Ok(existing_json.to_string());
        }

        let brain = llm_client
            .clone()
            .ok_or_else(|| anyhow!("LLM Client not available"))?;
        let generated = brain
            .build_n8n_workflow(&workflow_prompt)
            .await
            .map_err(|e| anyhow!("workflow generation failed: {}", e))?;
        mark_provision_progress(provision_op_id, "workflow_json_generated");
        Ok(generated)
    }
    .await;
    let workflow_json_str = match workflow_json_str_result {
        Ok(v) => v,
        Err(e) => {
            if parse_bool_env("STEER_N8N_MINIMAL_ON_LLM_FAILURE", true) {
                eprintln!(
                    "⚠️ workflow generation failed for recommendation {}: {}. Falling back to orchestrator template.",
                    id, e
                );
                match serde_json::to_string(&n8n_api::build_orchestrator_fallback_workflow(
                    &rec.title,
                    Some(&workflow_prompt),
                    "llm_generation_failed",
                )) {
                    Ok(fallback) => fallback,
                    Err(serr) => {
                        let _ = db::mark_workflow_provision_failed(
                            provision_op_id,
                            &format!(
                                "workflow json generation failed: {}; fallback serialization failed: {}",
                                e, serr
                            ),
                        );
                        if !force_recreate {
                            if let Some(token) = claim_token.as_deref() {
                                let _ = db::release_recommendation_provisioning_claim(id, token);
                            }
                        }
                        return Err(anyhow!(
                            "workflow generation failed: {}; fallback serialization failed: {}",
                            e,
                            serr
                        ));
                    }
                }
            } else {
                let _ = db::mark_workflow_provision_failed(
                    provision_op_id,
                    &format!("workflow json generation failed: {}", e),
                );
                if !force_recreate {
                    if let Some(token) = claim_token.as_deref() {
                        let _ = db::release_recommendation_provisioning_claim(id, token);
                    }
                }
                return Err(e);
            }
        }
    };

    let workflow_val_result = serde_json::from_str::<serde_json::Value>(&workflow_json_str)
        .map_err(|e| {
            anyhow!(
                "generated workflow JSON is invalid for recommendation {}: {}",
                id,
                e
            )
        });
    let mut workflow_val = match workflow_val_result {
        Ok(v) => v,
        Err(e) => {
            let _ = db::mark_workflow_provision_failed(
                provision_op_id,
                &format!("workflow json parse failed: {}", e),
            );
            if !force_recreate {
                if let Some(token) = claim_token.as_deref() {
                    let _ = db::release_recommendation_provisioning_claim(id, token);
                }
            }
            return Err(e);
        }
    };

    if !workflow_has_nodes(&workflow_val) {
        eprintln!(
            "⚠️ workflow for recommendation {} had empty/missing nodes before normalization.",
            id
        );
    }
    workflow_val = match n8n_api::normalize_workflow_for_create(&rec.title, &workflow_val) {
        Ok(v) => v,
        Err(e) => {
            let _ = db::mark_workflow_provision_failed(
                provision_op_id,
                &format!("workflow normalization failed: {}", e),
            );
            if !force_recreate {
                if let Some(token) = claim_token.as_deref() {
                    let _ = db::release_recommendation_provisioning_claim(id, token);
                }
            }
            return Err(anyhow!(
                "workflow normalization failed for recommendation {}: {}",
                id,
                e
            ));
        }
    };
    let workflow_json_str = serde_json::to_string(&workflow_val).map_err(|e| {
        anyhow!(
            "workflow serialization failed for recommendation {}: {}",
            id,
            e
        )
    })?;
    mark_provision_progress(provision_op_id, "workflow_json_normalized");

    let n8n = match n8n_api::N8nApi::from_env() {
        Ok(v) => v,
        Err(e) => {
            let _ = db::mark_workflow_provision_failed(
                provision_op_id,
                &format!("n8n initialization failed: {}", e),
            );
            if !force_recreate {
                if let Some(token) = claim_token.as_deref() {
                    let _ = db::release_recommendation_provisioning_claim(id, token);
                }
            }
            return Err(e);
        }
    };
    let active = n8n_create_active_default();
    mark_provision_progress(provision_op_id, "creating_workflow_in_n8n");

    match n8n.create_workflow(&rec.title, &workflow_val, active).await {
        Ok(workflow_id) => {
            mark_provision_progress(provision_op_id, "workflow_created");
            if let Err(e) = db::mark_workflow_provision_created(
                provision_op_id,
                &workflow_id,
                Some(&workflow_json_str),
            ) {
                let _ = db::mark_workflow_provision_reconcile_needed(
                    provision_op_id,
                    &format!("workflow created but op log update failed: {}", e),
                );
                return Err(anyhow!(
                    "workflow created (id={}) but failed to persist operation log: {}",
                    workflow_id,
                    e
                ));
            }
            if let Err(error) = activate_and_auto_trigger_workflow(
                id,
                &workflow_id,
                Some(&workflow_val),
                Some(&workflow_prompt),
            )
            .await
            {
                let _ = db::mark_workflow_provision_failed(
                    provision_op_id,
                    &format!(
                        "workflow created but auto activate/trigger failed: {}",
                        error
                    ),
                );
                let _ = db::mark_recommendation_failed(
                    id,
                    &format!(
                        "workflow created but auto activate/trigger failed: {}",
                        error
                    ),
                );
                if !force_recreate {
                    if let Some(token) = claim_token.as_deref() {
                        let _ = db::release_recommendation_provisioning_claim(id, token);
                    }
                }
                return Err(anyhow!(
                    "workflow created but auto activate/trigger failed: recommendation={} workflow_id={} error={}",
                    id,
                    workflow_id,
                    error
                ));
            }
            if let Err(e) = db::commit_workflow_provision_success(
                provision_op_id,
                id,
                &workflow_id,
                Some(&workflow_json_str),
            ) {
                let _ = db::mark_workflow_provision_reconcile_needed(
                    provision_op_id,
                    &format!("workflow executed but recommendation commit failed: {}", e),
                );
                let _ = db::mark_recommendation_failed(
                    id,
                    &format!("workflow executed but commit failed: {}", e),
                );
                return Err(anyhow!(
                    "workflow executed (id={}) but local commit failed: {}",
                    workflow_id,
                    e
                ));
            }
            println!(
                "✅ Workflow execution completed: recommendation={} workflow_id={}",
                id, workflow_id
            );
            Ok(workflow_id)
        }
        Err(e) => {
            let _ = db::mark_workflow_provision_failed(
                provision_op_id,
                &format!("workflow creation failed: {}", e),
            );
            if !force_recreate {
                if let Some(token) = claim_token.as_deref() {
                    let _ = db::release_recommendation_provisioning_claim(id, token);
                }
            }
            let _ = db::mark_recommendation_failed(id, &e.to_string());
            Err(anyhow!("workflow creation failed: {}", e))
        }
    }
}
