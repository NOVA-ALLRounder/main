use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Duration, SecondsFormat, Utc};
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const KEY_P1_TYPES: &[&str] = &[
    "outlook.compose_started",
    "outlook.attachment_added_meta",
    "excel.refresh_pivot",
];

const DEFAULT_COLLECTOR_DB_FILE: &str = "collector.db";
const DEFAULT_WINDOW_HINT_LIMIT: usize = 64;
const DEFAULT_MAX_RESOURCES: usize = 20;

#[path = "collector_pipeline/support.rs"]
mod support;

use support::*;

#[cfg(test)]
#[path = "collector_pipeline/tests.rs"]
mod tests;

#[derive(Debug, Deserialize, Default)]
struct ConfigYaml {
    db_path: Option<String>,
    privacy_rules_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SessionEventRow {
    pub ts: DateTime<Utc>,
    pub event_type: String,
    pub priority: String,
    pub app: String,
    pub resource_type: String,
    pub resource_id: String,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct SessionRecord {
    pub session_id: String,
    pub start_ts: String,
    pub end_ts: String,
    pub duration_sec: i64,
    pub summary: Value,
}

#[derive(Debug, Clone)]
pub struct RoutineSession {
    pub session_id: String,
    pub start_ts: DateTime<Utc>,
    pub end_ts: DateTime<Utc>,
    pub key_events: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RoutineCandidate {
    pub pattern_id: String,
    pub pattern_json: String,
    pub support: i64,
    pub confidence: f64,
    pub last_seen_ts: String,
    pub evidence_session_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HandoffBuildOptions {
    pub max_size_bytes: usize,
    pub recent_sessions: usize,
    pub recent_routines: usize,
    pub max_resources: usize,
    pub max_evidence: usize,
    pub redaction_scan_limit: usize,
}

#[derive(Debug, Clone)]
pub struct HandoffPayload {
    pub payload: Value,
    pub size_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct PendingHandoffRow {
    pub id: i64,
    pub package_id: String,
    pub created_at: String,
    pub status: String,
    pub attempt_count: i64,
    pub next_retry_at: Option<String>,
    pub lease_until: Option<String>,
    pub claimed_by: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct HandoffFailureUpdate {
    pub attempt_count: i64,
    pub max_attempts: i64,
    pub next_retry_at: Option<String>,
    pub terminal: bool,
}

#[derive(Debug, Clone)]
pub struct HandoffPrivacyRules {
    pub denylist_apps: HashSet<String>,
    pub redaction_patterns: Vec<Regex>,
    pub window_title_limit: usize,
}

impl Default for HandoffPrivacyRules {
    fn default() -> Self {
        Self {
            denylist_apps: HashSet::new(),
            redaction_patterns: Vec::new(),
            window_title_limit: DEFAULT_WINDOW_HINT_LIMIT,
        }
    }
}

#[path = "collector_pipeline/handoff_queue.rs"]
mod handoff_queue;
#[path = "collector_pipeline/routines_api.rs"]
mod routines_api;
#[path = "collector_pipeline/runtime.rs"]
mod runtime;
#[path = "collector_pipeline/sessions_api.rs"]
mod sessions_api;
#[path = "collector_pipeline/time.rs"]
mod time;

pub use handoff_queue::*;
pub use routines_api::*;
pub use runtime::*;
pub use sessions_api::*;
pub use time::*;
