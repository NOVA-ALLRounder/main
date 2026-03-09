use crate::{db, env_flag, llm_gateway::LLMClient, n8n_api, recommendation_policy};
use anyhow::{anyhow, Result};
use reqwest::Method;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApprovalExecutionOutcome {
    pub workflow_id: String,
    pub approved_now: bool,
    pub reused_existing: bool,
}

#[derive(Debug, Clone)]
pub struct PreclaimedProvisioning {
    pub claim_token: Option<String>,
    pub provision_op_id: i64,
    pub force_recreate: bool,
}

mod support;
use support::*;

mod execution;
pub use execution::{
    approve_and_execute_recommendation, execute_approved_recommendation,
    execute_approved_recommendation_with_preclaim, maybe_assume_approved_for_test,
    precreate_async_provisioning,
};

#[cfg(test)]
mod tests;
