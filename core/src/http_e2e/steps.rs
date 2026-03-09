use anyhow::Context;
use serde_json::json;
use std::path::Path;

use super::{HttpE2EReport, HttpE2EStepResult};
use crate::release_readiness::ReleaseReadinessReport;

pub(super) async fn check_health(
    client: &reqwest::Client,
    api_base_url: &str,
    steps: &mut Vec<HttpE2EStepResult>,
) {
    match client
        .get(format!("{api_base_url}/api/health"))
        .send()
        .await
        .context("health request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            super::support::push_step(
                steps,
                "health",
                status.is_success() && body.trim() == "ok",
                format!("status={} body={}", status, body.trim()),
            );
        }
        Err(error) => super::support::push_step(steps, "health", false, error.to_string()),
    }
}

pub(super) async fn check_db_scope(
    client: &reqwest::Client,
    api_base_url: &str,
    expected_db_path: &str,
    steps: &mut Vec<HttpE2EStepResult>,
) {
    match client
        .get(format!("{api_base_url}/api/system/db-paths"))
        .send()
        .await
        .context("db-path request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!({}));
            let runtime_db_path = body["core_db_path"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            super::support::push_step(
                steps,
                "db_scope",
                status.is_success() && runtime_db_path == expected_db_path,
                format!("status={} core_db_path={}", status, runtime_db_path),
            );
        }
        Err(error) => super::support::push_step(steps, "db_scope", false, error.to_string()),
    }
}

pub(super) async fn check_chat_local_help(
    client: &reqwest::Client,
    api_base_url: &str,
    steps: &mut Vec<HttpE2EStepResult>,
) {
    let help_payload = json!({
        "message": "도움말",
        "channel": "web",
        "chat_type": "direct",
        "sender": "http-e2e"
    });

    match client
        .post(format!("{api_base_url}/api/chat"))
        .json(&help_payload)
        .send()
        .await
        .context("first help request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!({}));
            let route_kind = body["route_meta"]["route_kind"]
                .as_str()
                .unwrap_or_default();
            let command = body["command"].as_str().unwrap_or_default();
            let local_only = body["route_meta"]["local_only"].as_bool().unwrap_or(false);
            super::support::push_step(
                steps,
                "chat_local_help",
                status.is_success()
                    && command == "help_local"
                    && route_kind == "local_command"
                    && local_only,
                format!(
                    "status={} command={} route_kind={} local_only={}",
                    status, command, route_kind, local_only
                ),
            );
        }
        Err(error) => super::support::push_step(steps, "chat_local_help", false, error.to_string()),
    }
}

pub(super) async fn check_chat_request_memory_reuse(
    client: &reqwest::Client,
    api_base_url: &str,
    steps: &mut Vec<HttpE2EStepResult>,
) {
    let help_payload = json!({
        "message": "도움말",
        "channel": "web",
        "chat_type": "direct",
        "sender": "http-e2e"
    });

    match client
        .post(format!("{api_base_url}/api/chat"))
        .json(&help_payload)
        .send()
        .await
        .context("repeat help request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!({}));
            let route_kind = body["route_meta"]["route_kind"]
                .as_str()
                .unwrap_or_default();
            let request_memory_hit = body["route_meta"]["request_memory_hit"]
                .as_bool()
                .unwrap_or(false);
            super::support::push_step(
                steps,
                "chat_request_memory_reuse",
                status.is_success() && route_kind == "request_memory" && request_memory_hit,
                format!(
                    "status={} route_kind={} request_memory_hit={}",
                    status, route_kind, request_memory_hit
                ),
            );
        }
        Err(error) => {
            super::support::push_step(steps, "chat_request_memory_reuse", false, error.to_string())
        }
    }
}

pub(super) async fn check_ai_digest_auto_route(
    client: &reqwest::Client,
    api_base_url: &str,
    steps: &mut Vec<HttpE2EStepResult>,
) {
    let digest_payload = json!({
        "message": "AI 뉴스 5개 요약해서 노션에 정리해줘",
        "channel": "web",
        "chat_type": "direct",
        "sender": "http-e2e"
    });

    match client
        .post(format!("{api_base_url}/api/chat"))
        .json(&digest_payload)
        .send()
        .await
        .context("ai digest request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!({}));
            let route_kind = body["route_meta"]["route_kind"]
                .as_str()
                .unwrap_or_default();
            let ai_digest_used = body["route_meta"]["ai_digest_used"]
                .as_bool()
                .unwrap_or(false);
            let response_text = body["response"].as_str().unwrap_or_default();
            super::support::push_step(
                steps,
                "ai_digest_auto_route",
                status.is_success()
                    && route_kind == "ai_digest_auto"
                    && ai_digest_used
                    && response_text.contains("https://www.notion.so/http-e2e-digest"),
                format!(
                    "status={} route_kind={} ai_digest_used={} response_has_notion={}",
                    status,
                    route_kind,
                    ai_digest_used,
                    response_text.contains("https://www.notion.so/http-e2e-digest")
                ),
            );
        }
        Err(error) => {
            super::support::push_step(steps, "ai_digest_auto_route", false, error.to_string())
        }
    }
}

pub(super) async fn check_recommendation_later_action(
    client: &reqwest::Client,
    api_base_url: &str,
    recommendation_id: i64,
    steps: &mut Vec<HttpE2EStepResult>,
) {
    match client
        .post(format!(
            "{api_base_url}/api/recommendations/{recommendation_id}/later"
        ))
        .json(&json!({
            "actor": "http_e2e",
            "note": "live http smoke snooze"
        }))
        .send()
        .await
        .context("later recommendation request failed")
    {
        Ok(response) => {
            let status = response.status();
            super::support::push_step(
                steps,
                "recommendation_later_action",
                status.is_success(),
                format!("status={}", status),
            );
        }
        Err(error) => super::support::push_step(
            steps,
            "recommendation_later_action",
            false,
            error.to_string(),
        ),
    }
}

pub(super) async fn check_recommendation_review_audit(
    client: &reqwest::Client,
    api_base_url: &str,
    steps: &mut Vec<HttpE2EStepResult>,
) {
    match client
        .get(format!(
            "{api_base_url}/api/recommendations/review-events?limit=10"
        ))
        .send()
        .await
        .context("review events request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!([]));
            let first = body
                .as_array()
                .and_then(|rows| rows.first())
                .cloned()
                .unwrap_or_else(|| json!({}));
            let action = first["action"].as_str().unwrap_or_default();
            let actor = first["actor"].as_str().unwrap_or_default();
            let ok = first["ok"].as_bool().unwrap_or(false);
            super::support::push_step(
                steps,
                "recommendation_review_audit",
                status.is_success() && action == "later" && actor == "http_e2e" && ok,
                format!(
                    "status={} action={} actor={} ok={}",
                    status, action, actor, ok
                ),
            );
        }
        Err(error) => super::support::push_step(
            steps,
            "recommendation_review_audit",
            false,
            error.to_string(),
        ),
    }
}

pub(super) async fn check_launch_ops_metrics(
    client: &reqwest::Client,
    api_base_url: &str,
    steps: &mut Vec<HttpE2EStepResult>,
) {
    match client
        .get(format!("{api_base_url}/api/launch/ops?limit=20"))
        .send()
        .await
        .context("launch ops request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!({}));
            let total_requests = body["chat_metrics"]["total_requests"]
                .as_i64()
                .unwrap_or_default();
            let request_memory_hits = body["chat_metrics"]["request_memory_hits"]
                .as_i64()
                .unwrap_or_default();
            let ai_digest_auto_routes = body["chat_metrics"]["ai_digest_auto_routes"]
                .as_i64()
                .unwrap_or_default();
            let later_actions = body["recommendation_review_metrics"]["later_actions"]
                .as_i64()
                .unwrap_or_default();
            super::support::push_step(
                steps,
                "launch_ops_metrics",
                status.is_success()
                    && total_requests >= 3
                    && request_memory_hits >= 1
                    && ai_digest_auto_routes >= 1
                    && later_actions >= 1,
                format!(
                    "status={} total_requests={} request_memory_hits={} ai_digest_auto_routes={} later_actions={}",
                    status, total_requests, request_memory_hits, ai_digest_auto_routes, later_actions
                ),
            );
        }
        Err(error) => {
            super::support::push_step(steps, "launch_ops_metrics", false, error.to_string())
        }
    }
}

pub(super) async fn check_http_e2e_latest_endpoint(
    client: &reqwest::Client,
    api_base_url: &str,
    workdir: &Path,
    report: &HttpE2EReport,
    steps: &mut Vec<HttpE2EStepResult>,
) {
    match client
        .get(format!("{api_base_url}/api/http-e2e/latest"))
        .query(&[("workdir", workdir.display().to_string())])
        .send()
        .await
        .context("http e2e latest request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!(null));
            let passed = body["passed"].as_u64().unwrap_or_default() as usize;
            let total = body["total"].as_u64().unwrap_or_default() as usize;
            let ok = body["ok"].as_bool().unwrap_or(false);
            super::support::push_step(
                steps,
                "http_e2e_latest_endpoint",
                status.is_success() && ok && passed == report.passed && total == report.total,
                format!(
                    "status={} ok={} passed={} total={}",
                    status, ok, passed, total
                ),
            );
        }
        Err(error) => {
            super::support::push_step(steps, "http_e2e_latest_endpoint", false, error.to_string())
        }
    }
}

pub(super) async fn check_http_e2e_history_endpoint(
    client: &reqwest::Client,
    api_base_url: &str,
    workdir: &Path,
    report: &HttpE2EReport,
    steps: &mut Vec<HttpE2EStepResult>,
) {
    match client
        .get(format!("{api_base_url}/api/http-e2e/history"))
        .query(&[
            ("workdir", workdir.display().to_string()),
            ("limit", "5".to_string()),
        ])
        .send()
        .await
        .context("http e2e history request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!([]));
            let first = body
                .as_array()
                .and_then(|rows| rows.first())
                .cloned()
                .unwrap_or_else(|| json!({}));
            let passed = first["passed"].as_u64().unwrap_or_default() as usize;
            let total = first["total"].as_u64().unwrap_or_default() as usize;
            let ok = first["ok"].as_bool().unwrap_or(false);
            super::support::push_step(
                steps,
                "http_e2e_history_endpoint",
                status.is_success() && ok && passed == report.passed && total == report.total,
                format!(
                    "status={} ok={} passed={} total={}",
                    status, ok, passed, total
                ),
            );
        }
        Err(error) => {
            super::support::push_step(steps, "http_e2e_history_endpoint", false, error.to_string())
        }
    }
}

pub(super) async fn check_release_readiness_latest_endpoint(
    client: &reqwest::Client,
    api_base_url: &str,
    readiness_fixture_root: &Path,
    readiness_fixture: &ReleaseReadinessReport,
    steps: &mut Vec<HttpE2EStepResult>,
) {
    match client
        .get(format!("{api_base_url}/api/release/readiness"))
        .query(&[("workdir", readiness_fixture_root.display().to_string())])
        .send()
        .await
        .context("release readiness latest request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!(null));
            let ready = body["ready_for_launch"].as_bool().unwrap_or(false);
            let report_status = body["status"].as_str().unwrap_or_default();
            let passed = body["http_e2e"]["passed"].as_u64().unwrap_or_default() as usize;
            super::support::push_step(
                steps,
                "release_readiness_latest_endpoint",
                status.is_success()
                    && ready
                    && report_status == "ready"
                    && passed
                        == readiness_fixture
                            .http_e2e
                            .as_ref()
                            .map(|value| value.passed)
                            .unwrap_or_default(),
                format!(
                    "status={} ready_for_launch={} report_status={} http_e2e_passed={}",
                    status, ready, report_status, passed
                ),
            );
        }
        Err(error) => super::support::push_step(
            steps,
            "release_readiness_latest_endpoint",
            false,
            error.to_string(),
        ),
    }
}

pub(super) async fn check_release_readiness_history_endpoint(
    client: &reqwest::Client,
    api_base_url: &str,
    readiness_fixture_root: &Path,
    readiness_fixture: &ReleaseReadinessReport,
    steps: &mut Vec<HttpE2EStepResult>,
) {
    match client
        .get(format!("{api_base_url}/api/release/readiness/history"))
        .query(&[
            ("workdir", readiness_fixture_root.display().to_string()),
            ("limit", "5".to_string()),
        ])
        .send()
        .await
        .context("release readiness history request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!([]));
            let first = body
                .as_array()
                .and_then(|rows| rows.first())
                .cloned()
                .unwrap_or_else(|| json!({}));
            let launch_eval_passed =
                first["launch_eval_passed"].as_u64().unwrap_or_default() as usize;
            let http_e2e_passed = first["http_e2e_passed"].as_u64().unwrap_or_default() as usize;
            let ready = first["ready_for_launch"].as_bool().unwrap_or(false);
            super::support::push_step(
                steps,
                "release_readiness_history_endpoint",
                status.is_success()
                    && ready
                    && launch_eval_passed == readiness_fixture.launch_eval.passed
                    && http_e2e_passed
                        == readiness_fixture
                            .http_e2e
                            .as_ref()
                            .map(|value| value.passed)
                            .unwrap_or_default(),
                format!(
                    "status={} ready_for_launch={} launch_eval_passed={} http_e2e_passed={}",
                    status, ready, launch_eval_passed, http_e2e_passed
                ),
            );
        }
        Err(error) => super::support::push_step(
            steps,
            "release_readiness_history_endpoint",
            false,
            error.to_string(),
        ),
    }
}
