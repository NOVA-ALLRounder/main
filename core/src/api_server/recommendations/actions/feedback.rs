use axum::{extract::Path, http::StatusCode, Json};

use crate::db;

use super::super::helpers::{
    classify_recommendation_feedback, log_recommendation_review_event,
    recommendation_effective_status, recommendation_feedback_snooze_hours,
    recommendation_review_actor,
};
use super::super::types::{RecommendationFeedbackRequest, RecommendationFeedbackResponse};

pub(crate) async fn submit_recommendation_feedback(
    Path(id): Path<i64>,
    Json(req): Json<RecommendationFeedbackRequest>,
) -> Result<(StatusCode, Json<RecommendationFeedbackResponse>), StatusCode> {
    let feedback = req.feedback.trim();
    let actor = recommendation_review_actor(req.actor.as_deref(), "api.recommendation_feedback");
    if feedback.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(RecommendationFeedbackResponse {
                ok: false,
                sentiment: "refine".to_string(),
                status: "invalid".to_string(),
                message: "피드백 내용을 입력해 주세요.".to_string(),
                suppressed_similar: false,
            }),
        ));
    }

    let rec = db::get_recommendation(id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let sentiment = classify_recommendation_feedback(feedback).to_string();
    let recorded = db::record_recommendation_feedback(id, &sentiment, feedback)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !recorded {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    match sentiment.as_str() {
        "negative" => {
            db::update_recommendation_review_status(id, "rejected")
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
        "refine" => {
            if !rec.status.eq_ignore_ascii_case("approved")
                && !rec.status.eq_ignore_ascii_case("rejected")
            {
                db::snooze_recommendation(id, recommendation_feedback_snooze_hours())
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            }
        }
        _ => {}
    }

    let updated = db::get_recommendation(id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let effective_status = recommendation_effective_status(&updated);

    let message = match sentiment.as_str() {
        "negative" => "피드백을 기록했고 이 추천은 거절 처리했습니다. 유사 자동 추천도 계속 억제됩니다.",
        "refine" => "피드백을 기록했고 이 추천은 잠시 숨겼습니다. 승인하면 수정 지시를 워크플로우 생성에 반영합니다.",
        _ => "피드백을 기록했습니다.",
    };
    let review_action = match sentiment.as_str() {
        "negative" => "feedback_negative",
        "refine" => "feedback_refine",
        _ => "feedback_positive",
    };
    log_recommendation_review_event(
        updated.id,
        &updated.title,
        Some(effective_status.as_str()),
        Some(updated.category.as_str()),
        review_action,
        Some(actor.as_str()),
        Some(feedback),
        true,
        Some(message),
    );

    Ok((
        StatusCode::OK,
        Json(RecommendationFeedbackResponse {
            ok: true,
            sentiment,
            status: effective_status,
            message: message.to_string(),
            suppressed_similar: updated
                .feedback_status
                .as_deref()
                .map(|value| value != "positive")
                .unwrap_or(false),
        }),
    ))
}
