use crate::{collector_pipeline, db, recommendation_policy};
use anyhow::{anyhow, Result};
use std::path::Path;

use super::manual::insert_or_get_recommendation_id;
use super::support::{
    allow_collector_db_mismatch, build_proposal_from_handoff, config_path, handoff_lease_secs,
    handoff_max_attempts, handoff_retry_base_secs, normalize_abs_path, validate_handoff_schema,
};

#[derive(Debug, Clone)]
pub struct CollectorHandoffIngestOutcome {
    pub status: String,
    pub detail: String,
    pub package_id: Option<String>,
    pub recommendation_id: Option<i64>,
    pub inserted: bool,
}

pub fn ingest_latest_collector_handoff(
    config_override: Option<&str>,
) -> Result<CollectorHandoffIngestOutcome> {
    let cfg_path = config_path(config_override);
    let collector_db = collector_pipeline::resolve_db_path(Some(&cfg_path));
    if !allow_collector_db_mismatch() {
        if let Some(core_db_path) = db::current_db_path() {
            let collector_norm = normalize_abs_path(&collector_db);
            let core_norm = normalize_abs_path(Path::new(&core_db_path));
            if collector_norm != core_norm {
                return Err(anyhow!(
                    "collector/core db path mismatch (collector={}, core={}). \
Set STEER_ALLOW_COLLECTOR_DB_MISMATCH=1 only when intentional.",
                    collector_norm,
                    core_norm
                ));
            }
        }
    }
    let mut conn = collector_pipeline::open_connection(&collector_db)?;
    collector_pipeline::ensure_pipeline_tables(&conn)?;
    let max_attempts = handoff_max_attempts();
    let retry_base_secs = handoff_retry_base_secs();
    let lease_secs = handoff_lease_secs();
    let consumer_id = format!("intake-{}", std::process::id());

    let Some(row) = collector_pipeline::claim_retryable_handoff(
        &mut conn,
        max_attempts,
        &consumer_id,
        lease_secs,
    )?
    else {
        return Ok(CollectorHandoffIngestOutcome {
            status: "noop".to_string(),
            detail: "no retryable collector handoff".to_string(),
            package_id: None,
            recommendation_id: None,
            inserted: false,
        });
    };

    let package_id = row.package_id.clone();
    if let Err(schema_err) = validate_handoff_schema(&row.payload) {
        let detail = format!("handoff schema invalid: {}", schema_err);
        collector_pipeline::update_handoff_status(&conn, row.id, "invalid", Some(&detail))?;
        let _ = db::record_collector_handoff_receipt(
            &package_id,
            Some(row.id),
            "invalid",
            None,
            Some(&detail),
        );
        return Ok(CollectorHandoffIngestOutcome {
            status: "invalid".to_string(),
            detail,
            package_id: Some(package_id),
            recommendation_id: None,
            inserted: false,
        });
    }

    let mut proposal = build_proposal_from_handoff(&row)?;
    let decision = recommendation_policy::apply_mvp_policy(&mut proposal, None);
    if !decision.accepted {
        let detail = format!(
            "collector handoff skipped by recommendation policy (category={} score={:.2})",
            decision.category, decision.business_score
        );
        collector_pipeline::update_handoff_status(&conn, row.id, "skipped", Some(&detail))?;
        let _ = db::record_collector_handoff_receipt(
            &package_id,
            Some(row.id),
            "skipped",
            None,
            Some(&detail),
        );
        return Ok(CollectorHandoffIngestOutcome {
            status: "skipped".to_string(),
            detail,
            package_id: Some(package_id),
            recommendation_id: None,
            inserted: false,
        });
    }
    let preference_history =
        db::get_recent_recommendations(recommendation_policy::auto_recommendation_history_limit())
            .unwrap_or_default();
    recommendation_policy::apply_recommendation_preferences(&mut proposal, &preference_history);

    let ingest_result = insert_or_get_recommendation_id(&proposal);

    match ingest_result {
        Ok((rec_id, inserted)) => {
            collector_pipeline::mark_handoff_consumed(&conn, row.id)?;
            let _ = db::record_collector_handoff_receipt(
                &package_id,
                Some(row.id),
                "consumed",
                Some(rec_id),
                Some(&format!(
                    "inserted={} attempts={}",
                    inserted,
                    row.attempt_count + 1
                )),
            );
            Ok(CollectorHandoffIngestOutcome {
                status: "consumed".to_string(),
                detail: format!(
                    "collector handoff consumed -> recommendation {} (inserted={}, attempts={})",
                    rec_id,
                    inserted,
                    row.attempt_count + 1
                ),
                package_id: Some(package_id),
                recommendation_id: Some(rec_id),
                inserted,
            })
        }
        Err(e) => {
            let update = collector_pipeline::mark_handoff_failed_with_backoff(
                &conn,
                row.id,
                &e.to_string(),
                max_attempts,
                retry_base_secs,
            )?;
            let status = if update.terminal {
                "failed".to_string()
            } else {
                "retry_scheduled".to_string()
            };
            let detail = if let Some(next_retry_at) = update.next_retry_at {
                format!(
                    "handoff ingest failed (attempt {}/{}): {} | next_retry_at={}",
                    update.attempt_count, update.max_attempts, e, next_retry_at
                )
            } else {
                format!(
                    "handoff ingest failed (attempt {}/{}): {} | no more retries",
                    update.attempt_count, update.max_attempts, e
                )
            };
            let _ = db::record_collector_handoff_receipt(
                &package_id,
                Some(row.id),
                &status,
                None,
                Some(&detail),
            );
            Ok(CollectorHandoffIngestOutcome {
                status,
                detail,
                package_id: Some(package_id),
                recommendation_id: None,
                inserted: false,
            })
        }
    }
}
