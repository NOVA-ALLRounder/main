use axum::{extract::Path, http::StatusCode, Json};

use crate::db;

use super::super::helpers::{log_recommendation_review_event, recommendation_review_actor};
use super::super::types::RecommendationReviewActionRequest;

pub(crate) async fn reject_recommendation(
    Path(id): Path<i64>,
    payload: Option<Json<RecommendationReviewActionRequest>>,
) -> StatusCode {
    let actor = recommendation_review_actor(
        payload.as_ref().and_then(|p| p.actor.as_deref()),
        "api.recommendation_review",
    );
    let note = payload.as_ref().and_then(|p| p.note.as_deref());
    let rec = match db::get_recommendation(id) {
        Ok(Some(rec)) => rec,
        Ok(None) => {
            log_recommendation_review_event(
                id,
                &format!("recommendation:{}", id),
                Some("missing"),
                None,
                "reject",
                Some(actor.as_str()),
                note,
                false,
                Some("recommendation not found"),
            );
            return StatusCode::NOT_FOUND;
        }
        Err(error) => {
            let details = error.to_string();
            log_recommendation_review_event(
                id,
                &format!("recommendation:{}", id),
                Some("error"),
                None,
                "reject",
                Some(actor.as_str()),
                note,
                false,
                Some(&details),
            );
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };
    match db::update_recommendation_review_status(id, "rejected") {
        Ok(_) => {
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some("rejected"),
                Some(rec.category.as_str()),
                "reject",
                Some(actor.as_str()),
                note,
                true,
                Some("recommendation rejected"),
            );
            StatusCode::OK
        }
        Err(error) => {
            let details = error.to_string();
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some(rec.status.as_str()),
                Some(rec.category.as_str()),
                "reject",
                Some(actor.as_str()),
                note,
                false,
                Some(&details),
            );
            if details.contains("not found") {
                StatusCode::NOT_FOUND
            } else if details.contains("invalid recommendation transition") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

pub(crate) async fn later_recommendation(
    Path(id): Path<i64>,
    payload: Option<Json<RecommendationReviewActionRequest>>,
) -> StatusCode {
    let actor = recommendation_review_actor(
        payload.as_ref().and_then(|p| p.actor.as_deref()),
        "api.recommendation_review",
    );
    let note = payload.as_ref().and_then(|p| p.note.as_deref());
    let rec = match db::get_recommendation(id) {
        Ok(Some(rec)) => rec,
        Ok(None) => {
            log_recommendation_review_event(
                id,
                &format!("recommendation:{}", id),
                Some("missing"),
                None,
                "later",
                Some(actor.as_str()),
                note,
                false,
                Some("recommendation not found"),
            );
            return StatusCode::NOT_FOUND;
        }
        Err(error) => {
            let details = error.to_string();
            log_recommendation_review_event(
                id,
                &format!("recommendation:{}", id),
                Some("error"),
                None,
                "later",
                Some(actor.as_str()),
                note,
                false,
                Some(&details),
            );
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };
    let hours = std::env::var("ALLVIA_RECOMMENDATION_SNOOZE_HOURS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .map(|v| v.clamp(1, 24 * 30))
        .unwrap_or(24 * 7);
    match db::snooze_recommendation(id, hours) {
        Ok(_) => {
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some("pending"),
                Some(rec.category.as_str()),
                "later",
                Some(actor.as_str()),
                note,
                true,
                Some("recommendation snoozed"),
            );
            StatusCode::OK
        }
        Err(error) => {
            let details = error.to_string();
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some(rec.status.as_str()),
                Some(rec.category.as_str()),
                "later",
                Some(actor.as_str()),
                note,
                false,
                Some(&details),
            );
            if details.contains("not found") {
                StatusCode::NOT_FOUND
            } else if details.contains("cannot snooze recommendation") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

pub(crate) async fn restore_recommendation(
    Path(id): Path<i64>,
    payload: Option<Json<RecommendationReviewActionRequest>>,
) -> StatusCode {
    let actor = recommendation_review_actor(
        payload.as_ref().and_then(|p| p.actor.as_deref()),
        "api.recommendation_review",
    );
    let note = payload.as_ref().and_then(|p| p.note.as_deref());
    let rec = match db::get_recommendation(id) {
        Ok(Some(rec)) => rec,
        Ok(None) => {
            log_recommendation_review_event(
                id,
                &format!("recommendation:{}", id),
                Some("missing"),
                None,
                "restore",
                Some(actor.as_str()),
                note,
                false,
                Some("recommendation not found"),
            );
            return StatusCode::NOT_FOUND;
        }
        Err(error) => {
            let details = error.to_string();
            log_recommendation_review_event(
                id,
                &format!("recommendation:{}", id),
                Some("error"),
                None,
                "restore",
                Some(actor.as_str()),
                note,
                false,
                Some(&details),
            );
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };
    match db::restore_recommendation(id) {
        Ok(_) => {
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some("pending"),
                Some(rec.category.as_str()),
                "restore",
                Some(actor.as_str()),
                note,
                true,
                Some("recommendation restored"),
            );
            StatusCode::OK
        }
        Err(error) => {
            let details = error.to_string();
            log_recommendation_review_event(
                rec.id,
                &rec.title,
                Some(rec.status.as_str()),
                Some(rec.category.as_str()),
                "restore",
                Some(actor.as_str()),
                note,
                false,
                Some(&details),
            );
            if details.contains("not found") {
                StatusCode::NOT_FOUND
            } else if details.contains("cannot restore recommendation") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}
