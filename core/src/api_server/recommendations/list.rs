use axum::{extract::Query, Json};

use crate::db;

use super::helpers::{
    limit_visible_recommendations, recommendation_effective_category_and_score,
    recommendation_effective_status, workflow_editor_url,
};
use super::types::{RecQueryParams, RecommendationItem};

pub(crate) async fn list_recommendations(
    Query(params): Query<RecQueryParams>,
) -> Json<Vec<RecommendationItem>> {
    let _ = db::reconcile_workflow_provision_ops(50);
    let filter = params
        .status
        .as_deref()
        .filter(|s| !s.is_empty())
        .or(Some("all"));
    let category_filter =
        crate::recommendation_policy::normalize_list_category_filter(params.category.as_deref());

    match db::get_recommendations_with_filter(filter) {
        Ok(recs) => {
            let preference_history = db::get_recent_recommendations(
                crate::recommendation_policy::auto_recommendation_history_limit(),
            )
            .unwrap_or_default();
            Json(
                limit_visible_recommendations(
                    recs,
                    category_filter.as_deref(),
                    &preference_history,
                )
                .into_iter()
                .map(|r| {
                    let (effective_category, effective_business_score) =
                        recommendation_effective_category_and_score(&r);
                    let approval_readiness =
                        crate::recommendation_policy::evaluate_recommendation_approval_readiness(
                            &r,
                        );
                    RecommendationItem {
                        id: r.id,
                        status: recommendation_effective_status(&r),
                        title: r.title,
                        summary: r.summary,
                        confidence: r.confidence,
                        category: effective_category,
                        business_score: effective_business_score,
                        evidence: r.evidence,
                        last_error: r.last_error,
                        workflow_id: r.workflow_id.clone(),
                        workflow_url: r.workflow_id.as_deref().and_then(workflow_editor_url),
                        snoozed_until: r.snoozed_until.clone(),
                        approval_ready: approval_readiness.ready,
                        approval_reasons: approval_readiness.reasons,
                    }
                })
                .collect(),
            )
        }
        Err(_) => Json(vec![]),
    }
}
