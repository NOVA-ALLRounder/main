#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskRunRecord {
    pub run_id: String,
    pub created_at: String,
    pub finished_at: Option<String>,
    pub intent: String,
    pub prompt: String,
    pub planner_complete: bool,
    pub execution_complete: bool,
    pub business_complete: bool,
    pub status: String,
    pub summary: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskStageRunRecord {
    pub id: i64,
    pub run_id: String,
    pub stage_name: String,
    pub stage_order: i64,
    pub status: String,
    pub started_at: String,
    pub finished_at: String,
    pub details: Option<String>,
    pub retry_count: i64,
    pub max_retries: i64,
    pub next_retry_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskStageAssertionRecord {
    pub id: i64,
    pub run_id: String,
    pub stage_name: String,
    pub assertion_key: String,
    pub expected: String,
    pub actual: String,
    pub passed: bool,
    pub evidence: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskRunArtifactRecord {
    pub id: i64,
    pub run_id: String,
    pub artifact_type: String,
    pub artifact_key: String,
    pub value: String,
    pub metadata: Option<String>,
    pub created_at: String,
}
