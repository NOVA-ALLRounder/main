use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::{db, recommendation_executor};

use super::super::helpers::{
    log_recommendation_review_event, recommendation_approval_block_message,
    recommendation_review_actor, workflow_editor_url,
};
use super::super::types::RecommendationReviewActionRequest;
use crate::api_server::AppState;

pub(crate) async fn approve_recommendation(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    payload: Option<Json<RecommendationReviewActionRequest>>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    println!("🔔 Received approval request for Recommendation ID: {}", id);
    let latest_op_snapshot = |recommendation_id: i64| {
        db::latest_workflow_provision_op(recommendation_id)
            .ok()
            .flatten()
            .map(|op| (op.id, op.status, op.updated_at))
    };
    let actor = recommendation_review_actor(
        payload.as_ref().and_then(|p| p.actor.as_deref()),
        "api.recommendation_review",
    );
    let note = payload.as_ref().and_then(|p| p.note.as_deref());

    let rec = db::get_recommendation(id)
        .map_err(|e| {
            let details = e.to_string();
            log_recommendation_review_event(
                id,
                &format!("recommendation:{}", id),
                Some("error"),
                None,
                "approve",
                Some(actor.as_str()),
                note,
                false,
                Some(&details),
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "recommendation_lookup_failed",
                    "details": details
                })),
            )
        })?
        .ok_or_else(|| {
            log_recommendation_review_event(
                id,
                &format!("recommendation:{}", id),
                Some("missing"),
                None,
                "approve",
                Some(actor.as_str()),
                note,
                false,
                Some("recommendation not found"),
            );
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "recommendation_not_found",
                    "id": id
                })),
            )
        })?;

    if rec.status.eq_ignore_ascii_case("rejected") {
        log_recommendation_review_event(
            rec.id,
            &rec.title,
            Some(rec.status.as_str()),
            Some(rec.category.as_str()),
            "approve",
            Some(actor.as_str()),
            note,
            false,
            Some("recommendation is rejected"),
        );
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "recommendation_rejected",
                "details": format!("recommendation {} is rejected", id),
            })),
        ));
    }

    let approval_readiness =
        crate::recommendation_policy::evaluate_recommendation_approval_readiness(&rec);
    if !approval_readiness.ready {
        let details = recommendation_approval_block_message(id, &approval_readiness.reasons);
        log_recommendation_review_event(
            rec.id,
            &rec.title,
            Some(rec.status.as_str()),
            Some(rec.category.as_str()),
            "approve",
            Some(actor.as_str()),
            note,
            false,
            Some(&details),
        );
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({
                "error": details,
                "details": details,
                "reasons": approval_readiness.reasons,
            })),
        ));
    }

    let approved_now = !rec.status.eq_ignore_ascii_case("approved");
    if approved_now {
        db::update_recommendation_review_status(id, "approved").map_err(|e| {
            let details = e.to_string();
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some(rec.status.as_str()),
                Some(rec.category.as_str()),
                "approve",
                Some(actor.as_str()),
                note,
                false,
                Some(&details),
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "recommendation_approve_update_failed",
                    "details": details,
                })),
            )
        })?;
    }

    if let Some(existing_id) = rec
        .workflow_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if existing_id.starts_with("provisioning:") {
            let latest_op = latest_op_snapshot(id);
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some("approved"),
                Some(rec.category.as_str()),
                "approve",
                Some(actor.as_str()),
                note,
                true,
                Some("workflow provisioning already in progress"),
            );
            return Ok((
                StatusCode::ACCEPTED,
                Json(serde_json::json!({
                    "status": "accepted",
                    "id": serde_json::Value::Null,
                    "workflow_id": serde_json::Value::Null,
                    "workflow_url": serde_json::Value::Null,
                    "provision_op_id": latest_op.as_ref().map(|(op_id, _, _)| *op_id),
                    "provision_status": latest_op.as_ref().map(|(_, status, _)| status.clone()),
                    "provision_updated_at": latest_op.as_ref().map(|(_, _, updated_at)| updated_at.clone()),
                    "provision_claim": existing_id,
                    "approved_now": approved_now,
                    "reused_existing": false,
                    "message": "Workflow provisioning already in progress",
                })),
            ));
        }

        let workflow_url = workflow_editor_url(existing_id);
        log_recommendation_review_event(
            rec.id,
            &rec.title,
            Some("approved"),
            Some(rec.category.as_str()),
            "approve",
            Some(actor.as_str()),
            note,
            true,
            Some("workflow already existed; reused existing workflow_id"),
        );
        return Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "success",
                "id": existing_id,
                "workflow_id": existing_id,
                "workflow_url": workflow_url,
                "provision_op_id": serde_json::Value::Null,
                "provision_status": serde_json::Value::Null,
                "provision_updated_at": serde_json::Value::Null,
                "approved_now": approved_now,
                "reused_existing": true,
                "message": "Workflow already existed; reused existing workflow_id",
            })),
        ));
    }

    let provisioning = match recommendation_executor::precreate_async_provisioning(id) {
        Ok(p) => p,
        Err(error) => {
            let message = error.to_string();
            if message.contains("already being provisioned") {
                let latest_op = latest_op_snapshot(id);
                log_recommendation_review_event(
                    rec.id,
                    &rec.title,
                    Some("approved"),
                    Some(rec.category.as_str()),
                    "approve",
                    Some(actor.as_str()),
                    note,
                    true,
                    Some("workflow provisioning already in progress"),
                );
                return Ok((
                    StatusCode::ACCEPTED,
                    Json(serde_json::json!({
                        "status": "accepted",
                        "id": serde_json::Value::Null,
                        "workflow_id": serde_json::Value::Null,
                        "workflow_url": serde_json::Value::Null,
                        "provision_op_id": latest_op.as_ref().map(|(op_id, _, _)| *op_id),
                        "provision_status": latest_op.as_ref().map(|(_, status, _)| status.clone()),
                        "provision_updated_at": latest_op.as_ref().map(|(_, _, updated_at)| updated_at.clone()),
                        "approved_now": approved_now,
                        "reused_existing": false,
                        "message": "Workflow provisioning already in progress",
                    })),
                ));
            }
            if message.contains("already provisioned") {
                if let Ok(Some(latest_rec)) = db::get_recommendation(id) {
                    if let Some(existing_id) = latest_rec
                        .workflow_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .filter(|s| !s.starts_with("provisioning:"))
                    {
                        let workflow_url = workflow_editor_url(existing_id);
                        log_recommendation_review_event(
                            latest_rec.id,
                            &latest_rec.title,
                            Some("approved"),
                            Some(latest_rec.category.as_str()),
                            "approve",
                            Some(actor.as_str()),
                            note,
                            true,
                            Some("workflow already existed; reused existing workflow_id"),
                        );
                        return Ok((
                            StatusCode::OK,
                            Json(serde_json::json!({
                                "status": "success",
                                "id": existing_id,
                                "workflow_id": existing_id,
                                "workflow_url": workflow_url,
                                "provision_op_id": serde_json::Value::Null,
                                "provision_status": serde_json::Value::Null,
                                "provision_updated_at": serde_json::Value::Null,
                                "approved_now": approved_now,
                                "reused_existing": true,
                                "message": "Workflow already existed; reused existing workflow_id",
                            })),
                        ));
                    }
                }
            }
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some("approved"),
                Some(rec.category.as_str()),
                "approve",
                Some(actor.as_str()),
                note,
                false,
                Some(&message),
            );
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "workflow_provision_prepare_failed",
                    "details": message,
                })),
            ));
        }
    };

    let llm_client = state.llm_client.clone();
    let provisioning_for_spawn = provisioning.clone();
    tokio::spawn(async move {
        match recommendation_executor::execute_approved_recommendation_with_preclaim(
            id,
            llm_client,
            provisioning_for_spawn,
        )
        .await
        {
            Ok(workflow_id) => {
                println!(
                    "✅ Async workflow provisioning completed: recommendation={} workflow_id={}",
                    id, workflow_id
                );
            }
            Err(error) => {
                eprintln!(
                    "⚠️ Async workflow provisioning failed: recommendation={} error={}",
                    id, error
                );
            }
        }
    });

    let latest_op = latest_op_snapshot(id);
    log_recommendation_review_event(
        rec.id,
        &rec.title,
        Some("approved"),
        Some(rec.category.as_str()),
        "approve",
        Some(actor.as_str()),
        note,
        true,
        Some("approval accepted; workflow provisioning running asynchronously"),
    );
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "status": "accepted",
            "id": serde_json::Value::Null,
            "workflow_id": serde_json::Value::Null,
            "workflow_url": serde_json::Value::Null,
            "provision_op_id": latest_op
                .as_ref()
                .map(|(op_id, _, _)| *op_id)
                .or(Some(provisioning.provision_op_id)),
            "provision_status": latest_op
                .as_ref()
                .map(|(_, status, _)| status.clone())
                .or(Some("requested".to_string())),
            "provision_updated_at": latest_op.as_ref().map(|(_, _, updated_at)| updated_at.clone()),
            "provision_claim": provisioning.claim_token,
            "approved_now": approved_now,
            "reused_existing": false,
            "message": "Approval accepted. Workflow provisioning is running asynchronously.",
        })),
    ))
}
