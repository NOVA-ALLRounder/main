mod normalize;
mod scan;

use serde::{Deserialize, Serialize};

pub use scan::run_consistency_check;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendEndpoint {
    pub path: String,
    pub method: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendCall {
    pub path: String,
    pub method: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyIssue {
    pub path: String,
    pub reason: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyCheckResult {
    pub ok: bool,
    pub issues: Vec<ConsistencyIssue>,
    pub backend_paths: Vec<String>,
    pub frontend_calls: Vec<FrontendCall>,
    pub summary: String,
    pub template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyCheckRequest {
    pub workdir: Option<String>,
}

#[cfg(test)]
#[path = "consistency_check/tests.rs"]
mod tests;
