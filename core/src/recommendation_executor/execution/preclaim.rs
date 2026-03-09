use super::super::*;

pub fn precreate_async_provisioning(id: i64) -> Result<PreclaimedProvisioning> {
    let rec =
        db::get_recommendation(id)?.ok_or_else(|| anyhow!("recommendation {} not found", id))?;

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

    let force_recreate = env_flag("STEER_APPROVE_FORCE_RECREATE");
    let claim_token = if force_recreate {
        None
    } else {
        Some(make_claim_token(id))
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
            } else {
                return Err(anyhow!(
                    "recommendation {} already provisioned ({})",
                    id,
                    existing_id
                ));
            }
        }

        let token = claim_token
            .as_deref()
            .ok_or_else(|| anyhow!("missing provisioning claim token"))?;
        if let Some(existing) = claim_or_get_existing(id, token)? {
            return Err(anyhow!(
                "recommendation {} already provisioned ({})",
                id,
                existing
            ));
        }
    }

    let provision_op_id = db::create_workflow_provision_op(id, claim_token.as_deref())?;
    Ok(PreclaimedProvisioning {
        claim_token,
        provision_op_id,
        force_recreate,
    })
}
