use super::support::{
    derive_release_quality_score, load_launch_eval_summary,
    maybe_refresh_launch_eval_candidate_snapshot, normalize_release_exec_approval_metrics,
    resolve_workdir, visible_auto_work_pending_count,
};
use super::{ReleaseBaseline, ReleaseBaselineRequest};
use crate::{consistency_check, db, launch_eval, performance_verification, semantic_verification};

pub(super) fn build_baseline(req: ReleaseBaselineRequest) -> ReleaseBaseline {
    let workdir = resolve_workdir(req.workdir.as_deref());
    let semantic_max = req.max_files.unwrap_or(200);
    let performance_max = req.max_files.unwrap_or(300);

    let semantic = req.semantic.or_else(|| {
        Some(semantic_verification::semantic_consistency(
            &workdir,
            semantic_max,
        ))
    });
    let performance = req.performance.or_else(|| {
        Some(performance_verification::performance_baseline(
            &workdir,
            performance_max,
        ))
    });
    let consistency = req.consistency.or_else(|| {
        Some(consistency_check::run_consistency_check(
            consistency_check::ConsistencyCheckRequest {
                workdir: Some(workdir.to_string_lossy().to_string()),
            },
        ))
    });
    let launch_ops = db::get_launch_ops_metrics(200).ok();
    let launch_eval_candidate_snapshot_refresh_error =
        maybe_refresh_launch_eval_candidate_snapshot(
            &workdir,
            req.refresh_launch_eval_candidates,
            req.launch_eval_candidate_limit,
            req.launch_eval_snapshot_output_path.as_deref(),
        )
        .err()
        .map(|error| error.to_string());
    let launch_eval = load_launch_eval_summary(
        &workdir,
        req.launch_eval_config_path.as_deref(),
        req.launch_eval_report_path.as_deref(),
    );
    let launch_eval_candidate_snapshot =
        Some(launch_eval::read_launch_eval_candidate_snapshot_info(
            &workdir,
            req.launch_eval_snapshot_output_path.as_deref(),
        ));
    let quality = req.quality.or_else(|| {
        Some(derive_release_quality_score(
            consistency.as_ref(),
            semantic.as_ref(),
            performance.as_ref(),
            launch_ops.as_ref(),
            launch_eval.as_ref(),
        ))
    });
    let nl_run_metrics = db::get_release_nl_run_metrics(200).ok();
    let exec_approval_metrics = db::get_exec_approval_metrics(200)
        .ok()
        .map(normalize_release_exec_approval_metrics);
    let recommendation_metrics = db::get_recommendation_metrics().ok().map(|mut metrics| {
        metrics.pending = visible_auto_work_pending_count().unwrap_or(metrics.pending);
        metrics
    });
    let recommendation_review_metrics = db::get_recommendation_review_metrics(100).ok();

    ReleaseBaseline {
        created_at: chrono::Utc::now().to_rfc3339(),
        consistency,
        semantic,
        performance,
        quality,
        launch_ops,
        nl_run_metrics,
        exec_approval_metrics,
        recommendation_metrics,
        recommendation_review_metrics,
        launch_eval,
        launch_eval_candidate_snapshot,
        launch_eval_candidate_snapshot_refresh_error,
    }
}

pub(super) fn save_baseline(baseline: &ReleaseBaseline) {
    if let Ok(json) = serde_json::to_string(baseline) {
        let _ = db::upsert_release_baseline_json(&baseline.created_at, &json);
    }
}

pub(super) fn load_baseline() -> Option<ReleaseBaseline> {
    db::get_release_baseline_json()
        .ok()
        .and_then(|record| record)
        .and_then(|record| serde_json::from_str::<ReleaseBaseline>(&record.baseline_json).ok())
}
