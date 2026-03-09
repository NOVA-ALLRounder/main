use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(crate) struct RecommendationFeedbackRequest {
    pub feedback: String,
    pub actor: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct RecommendationReviewActionRequest {
    pub actor: Option<String>,
    pub note: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct RecommendationFeedbackResponse {
    pub ok: bool,
    pub sentiment: String,
    pub status: String,
    pub message: String,
    pub suppressed_similar: bool,
}

#[derive(Serialize)]
pub(crate) struct RecommendationItem {
    pub id: i64,
    pub status: String,
    pub title: String,
    pub summary: String,
    pub confidence: f64,
    pub category: String,
    pub business_score: f64,
    pub evidence: Vec<String>,
    pub last_error: Option<String>,
    pub workflow_id: Option<String>,
    pub workflow_url: Option<String>,
    pub snoozed_until: Option<String>,
    pub approval_ready: bool,
    pub approval_reasons: Vec<String>,
}

#[derive(Deserialize)]
pub(crate) struct RecQueryParams {
    pub status: Option<String>,
    pub category: Option<String>,
}
