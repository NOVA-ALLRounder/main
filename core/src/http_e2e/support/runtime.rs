use anyhow::{Context, Result};
use axum::{routing::post, Json, Router};
use serde_json::json;
use std::path::Path;

use super::super::HttpE2EStepResult;

pub(super) struct EnvVarGuard {
    entries: Option<Vec<(String, Option<String>)>>,
}

impl EnvVarGuard {
    pub(super) fn capture(keys: &[&str]) -> Self {
        let entries = keys
            .iter()
            .map(|key| (key.to_string(), std::env::var(key).ok()))
            .collect();
        Self {
            entries: Some(entries),
        }
    }

    fn restore_in_place(&mut self) {
        if let Some(entries) = self.entries.take() {
            for (key, value) in entries {
                match value {
                    Some(value) => std::env::set_var(&key, value),
                    None => std::env::remove_var(&key),
                }
            }
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        self.restore_in_place();
    }
}

pub(in crate::http_e2e) struct DbRuntimeIsolationGuard {
    env_guard: EnvVarGuard,
}

impl DbRuntimeIsolationGuard {
    pub(in crate::http_e2e) fn capture(keys: &[&str]) -> Self {
        Self {
            env_guard: EnvVarGuard::capture(keys),
        }
    }
}

impl Drop for DbRuntimeIsolationGuard {
    fn drop(&mut self) {
        crate::db::reset_connection();
        self.env_guard.restore_in_place();
        crate::db::reset_connection();
        let _ = crate::db::init();
    }
}

pub(in crate::http_e2e) struct ServerHandle {
    join: Option<tokio::task::JoinHandle<()>>,
}

impl ServerHandle {
    pub(in crate::http_e2e) fn new(join: tokio::task::JoinHandle<()>) -> Self {
        Self { join: Some(join) }
    }

    pub(in crate::http_e2e) async fn shutdown(mut self) {
        if let Some(join) = self.join.take() {
            join.abort();
            let _ = join.await;
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        if let Some(join) = self.join.take() {
            join.abort();
        }
    }
}

pub(in crate::http_e2e) fn push_step(
    steps: &mut Vec<HttpE2EStepResult>,
    name: &str,
    ok: bool,
    detail: impl Into<String>,
) {
    steps.push(HttpE2EStepResult {
        name: name.to_string(),
        ok,
        detail: detail.into(),
    });
}

pub(in crate::http_e2e) async fn spawn_digest_stub() -> Result<(String, ServerHandle)> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .context("failed to bind digest stub listener")?;
    let addr = listener
        .local_addr()
        .context("failed to read digest stub addr")?;
    let app = Router::new().route(
        "/",
        post(|| async {
            Json(json!({
                "status": "ok",
                "notion_url": "https://www.notion.so/http-e2e-digest",
                "top_headlines_text": "1. 헤드라인 A\n2. 헤드라인 B"
            }))
        }),
    );
    let join = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok((format!("http://{addr}/"), ServerHandle::new(join)))
}

pub(in crate::http_e2e) fn canonical_string(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}
