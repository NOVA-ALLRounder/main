mod baseline;
mod comparison;
mod support;

use crate::{
    consistency_check, db, launch_eval, performance_verification, quality_scorer,
    semantic_verification,
};
use comparison::evaluate_release_gate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalSummary {
    pub generated_at: String,
    pub config_path: Option<String>,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub failed_case_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseBaseline {
    pub created_at: String,
    pub consistency: Option<consistency_check::ConsistencyCheckResult>,
    pub semantic: Option<semantic_verification::SemanticVerificationResult>,
    pub performance: Option<performance_verification::PerformanceVerificationResult>,
    pub quality: Option<quality_scorer::QualityScore>,
    pub launch_ops: Option<db::LaunchOpsMetrics>,
    pub nl_run_metrics: Option<db::NLRunMetrics>,
    pub exec_approval_metrics: Option<db::ExecApprovalMetrics>,
    pub recommendation_metrics: Option<db::RecommendationMetrics>,
    pub recommendation_review_metrics: Option<db::RecommendationReviewMetrics>,
    pub launch_eval: Option<LaunchEvalSummary>,
    pub launch_eval_candidate_snapshot: Option<launch_eval::LaunchEvalCandidateSnapshotInfo>,
    pub launch_eval_candidate_snapshot_refresh_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseGateResult {
    pub ok: bool,
    pub regressions: Vec<String>,
    pub warnings: Vec<String>,
    pub baseline: Option<ReleaseBaseline>,
    pub current: ReleaseBaseline,
    pub template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseGateRequest {
    pub workdir: Option<String>,
    pub max_files: Option<usize>,
    pub consistency: Option<consistency_check::ConsistencyCheckResult>,
    pub semantic: Option<semantic_verification::SemanticVerificationResult>,
    pub performance: Option<performance_verification::PerformanceVerificationResult>,
    pub quality: Option<quality_scorer::QualityScore>,
    pub perf_regression_pct: Option<f64>,
    pub quality_drop: Option<f64>,
    pub launch_error_rate_pct: Option<f64>,
    pub launch_low_confidence_rate_pct: Option<f64>,
    pub launch_cache_hit_rate_drop_pct: Option<f64>,
    pub recommendation_approval_rate_min: Option<f64>,
    pub launch_eval_config_path: Option<String>,
    pub launch_eval_report_path: Option<String>,
    pub refresh_launch_eval_candidates: Option<bool>,
    pub launch_eval_candidate_limit: Option<usize>,
    pub launch_eval_snapshot_output_path: Option<String>,
}

pub type ReleaseBaselineRequest = ReleaseGateRequest;

pub fn build_baseline(req: ReleaseBaselineRequest) -> ReleaseBaseline {
    baseline::build_baseline(req)
}

pub fn save_baseline(baseline: &ReleaseBaseline) {
    baseline::save_baseline(baseline)
}

pub use support::load_baseline_from_path;

pub fn run_release_gate(req: ReleaseGateRequest) -> ReleaseGateResult {
    let perf_override = req.perf_regression_pct;
    let quality_override = req.quality_drop;
    let launch_error_override = req.launch_error_rate_pct;
    let launch_low_conf_override = req.launch_low_confidence_rate_pct;
    let launch_cache_drop_override = req.launch_cache_hit_rate_drop_pct;
    let recommendation_approval_min = req.recommendation_approval_rate_min;
    let current = build_baseline(req);
    let baseline = baseline::load_baseline();
    evaluate_release_gate(
        current,
        baseline,
        perf_override,
        quality_override,
        launch_error_override,
        launch_low_conf_override,
        launch_cache_drop_override,
        recommendation_approval_min,
    )
}

#[cfg(test)]
mod tests;
