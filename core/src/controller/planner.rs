use crate::action_schema;
use crate::controller::actions::ActionRunner;
use crate::controller::heuristics;
use crate::controller::loop_detector::LoopDetector;
use crate::controller::supervisor::Supervisor;
use crate::db;
use crate::llm_gateway::LLMClient;
use crate::schema::EventEnvelope;
use crate::session_store::{Session, SessionStatus, SessionStep};
use crate::visual_driver::{SmartStep, VisualDriver};
use anyhow::Result;
use chrono::Utc;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use uuid::Uuid;

#[path = "planner/fallback.rs"]
mod fallback;
#[path = "planner/fallback_domains.rs"]
mod fallback_domains;
#[path = "planner/fallback_general.rs"]
mod fallback_general;
#[path = "planner/fallback_history.rs"]
mod fallback_history;
#[path = "planner/fallback_review.rs"]
mod fallback_review;
#[path = "planner/fallback_support.rs"]
mod fallback_support;
#[path = "planner/fallback_text.rs"]
mod fallback_text;
#[path = "planner/rewrites.rs"]
mod rewrites;
#[path = "planner/run.rs"]
mod run;
#[path = "planner/run_execution.rs"]
mod run_execution;
#[path = "planner/run_supervisor.rs"]
mod run_supervisor;
#[path = "planner/runtime.rs"]
mod runtime;
#[path = "planner/setup.rs"]
mod setup;
#[path = "planner/step.rs"]
mod step;
#[path = "planner/summary.rs"]
mod summary;
#[path = "planner/summary_evidence.rs"]
mod summary_evidence;
#[path = "planner/summary_failures.rs"]
mod summary_failures;
#[path = "planner/tracking.rs"]
mod tracking;
#[path = "planner/util.rs"]
mod util;

static GUI_RUN_SERIAL_LOCK: Lazy<AsyncMutex<()>> = Lazy::new(|| AsyncMutex::new(()));

pub struct Planner {
    pub llm: Arc<dyn LLMClient>,
    pub max_steps: usize,
    pub tx: Option<mpsc::Sender<String>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RunGoalOutcome {
    pub run_id: String,
    pub planner_complete: bool,
    pub execution_complete: bool,
    pub business_complete: bool,
    pub status: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone)]
struct RunGoalExecutionSummary {
    planner_complete: bool,
    execution_complete: bool,
    business_complete: bool,
    business_note: String,
    approval_required: bool,
    preflight_permissions_ok: bool,
    preflight_screen_capture_ok: bool,
    cleanup_dialog_closed_count: usize,
    cleanup_app_ready_count: usize,
    cleanup_mail_outgoing_hidden_count: usize,
    step_count: usize,
    failed_steps: usize,
    blocking_failed_steps: usize,
    blocking_failure_details: Vec<String>,
    mail_send_required: bool,
    mail_send_confirmed: bool,
    notes_write_required: bool,
    notes_write_confirmed: bool,
    textedit_write_required: bool,
    textedit_write_confirmed: bool,
    textedit_save_required: bool,
    textedit_save_confirmed: bool,
    capture_total_ms: u128,
    capture_max_ms: u128,
    capture_count: usize,
    plan_total_ms: u128,
    plan_max_ms: u128,
    plan_count: usize,
    supervisor_total_ms: u128,
    supervisor_max_ms: u128,
    supervisor_count: usize,
    execute_total_ms: u128,
    execute_max_ms: u128,
    execute_count: usize,
}

#[derive(Debug, Default, Clone)]
struct RunGoalBusinessEvidence {
    mail_send_confirmed: bool,
    notes_write_confirmed: bool,
    textedit_write_confirmed: bool,
    textedit_save_confirmed: bool,
}

#[derive(Debug, Default, Clone)]
struct PlannerTimingStats {
    capture_total_ms: u128,
    capture_max_ms: u128,
    capture_count: usize,
    plan_total_ms: u128,
    plan_max_ms: u128,
    plan_count: usize,
    supervisor_total_ms: u128,
    supervisor_max_ms: u128,
    supervisor_count: usize,
    execute_total_ms: u128,
    execute_max_ms: u128,
    execute_count: usize,
}

struct PlannerStepObservation {
    image_b64: String,
    plan_key: String,
}

impl Planner {}

#[cfg(test)]
#[path = "planner/tests.rs"]
mod tests;
