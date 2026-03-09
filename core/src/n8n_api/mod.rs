#![allow(dead_code)]
use anyhow::Result;
use reqwest::{Client, Response};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStatus {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub id: String,
    pub finished: bool,
    pub status: String,
    pub started_at: String,
    pub stopped_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: String,
    pub name: String,
    pub type_name: String,
}

#[allow(dead_code)]
pub struct N8nApi {
    base_url: String,
    api_key: String,
    client: Client,
}

mod support;

#[cfg(test)]
mod tests;

use self::support::*;
pub use self::support::{build_orchestrator_fallback_workflow, normalize_workflow_for_create};

mod runtime;
mod workflows;
