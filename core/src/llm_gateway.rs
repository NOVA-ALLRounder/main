use crate::mcp_client;
use crate::recommendation::AutomationProposal;
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;

use async_trait::async_trait;
use std::time::Duration;

mod core_impl;
mod provider_impl;
mod streaming;

// =====================================================
// Phase 30: Intelligence Upgrade (Supervisor + Thinking)
// =====================================================

// =====================================================
// Phase 29: Robust JSON Recovery (Advanced CLI)
// =====================================================

/// Attempt to recover valid JSON from malformed LLM responses
/// Handles: markdown blocks, partial JSON, common syntax errors
pub fn recover_json(raw: &str) -> Option<Value> {
    // Step 1: Clean markdown code blocks
    let clean = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    // Step 2: Try direct parse
    if let Ok(v) = serde_json::from_str::<Value>(&clean) {
        return Some(v);
    }

    // Step 3: Find first { and last } and extract
    if let (Some(start), Some(end)) = (clean.find('{'), clean.rfind('}')) {
        if start < end {
            let json_candidate = &clean[start..=end];
            if let Ok(v) = serde_json::from_str::<Value>(json_candidate) {
                return Some(v);
            }

            // Step 4: Try fixing common errors
            // 4a: Trailing comma before }
            let fixed = json_candidate.replace(",}", "}").replace(",]", "]");
            if let Ok(v) = serde_json::from_str::<Value>(&fixed) {
                return Some(v);
            }

            // 4b: Unquoted keys (simple cases)
            let re_unquoted = regex::Regex::new(r#"(\{|,)\s*(\w+)\s*:"#).ok()?;
            let with_quotes = re_unquoted.replace_all(&fixed, r#"$1"$2":"#);
            if let Ok(v) = serde_json::from_str::<Value>(&with_quotes) {
                return Some(v);
            }
        }
    }

    // Step 5: Look for action pattern in text
    // e.g., "I will click" -> {"action": "click_visual", "description": "..."}
    let lower = clean.to_lowercase();
    if lower.contains("done") || lower.contains("goal achieved") || lower.contains("completed") {
        return Some(json!({"action": "done"}));
    }

    None
}

#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn plan_next_step(
        &self,
        goal: &str,
        ui_tree: &Value,
        action_history: &[String],
    ) -> Result<Value>;
    async fn chat_completion(&self, messages: Vec<Value>) -> Result<String>;
    async fn plan_vision_step(
        &self,
        goal: &str,
        image_b64: &str,
        history: &[String],
    ) -> Result<Value>;
    async fn analyze_routine(&self, logs: &[String]) -> Result<String>;
    async fn recommend_automation(&self, logs: &[String]) -> Result<String>;
    async fn build_n8n_workflow(&self, user_prompt: &str) -> Result<String>;
    async fn fix_n8n_workflow(
        &self,
        user_prompt: &str,
        bad_json: &str,
        error_msg: &str,
    ) -> Result<String, Box<dyn std::error::Error>>;
    async fn get_embedding(&self, text: &str) -> Result<Vec<f32>>;
    async fn propose_workflow(
        &self,
        logs: &[String],
    ) -> Result<AutomationProposal, Box<dyn std::error::Error>>;
    async fn analyze_tendency(&self, logs: &[String]) -> Result<String>;
    async fn parse_intent(&self, user_input: &str) -> Result<Value>;
    async fn parse_intent_with_history(
        &self,
        user_input: &str,
        history: &[crate::db::ChatMessage],
    ) -> Result<Value>;
    async fn generate_recommendation_from_pattern(
        &self,
        pattern_description: &str,
        sample_events: &[String],
    ) -> Result<AutomationProposal>;
    async fn analyze_screen(
        &self,
        prompt: &str,
        image_b64: &str,
    ) -> Result<String, Box<dyn std::error::Error>>;
    async fn find_element_coordinates(
        &self,
        element_description: &str,
        image_b64: &str,
    ) -> Result<Option<(i32, i32)>>;
    async fn score_quality(
        &self,
        system_prompt: &str,
        payload: &serde_json::Value,
    ) -> Result<String>;
    async fn propose_solution_stack(&self, goal: &str) -> Result<Value>;
    async fn inference_local(&self, prompt: &str, model: Option<&str>) -> Result<String>;
    fn route_task(&self, task_description: &str, pii_detected: bool) -> (bool, String);
    async fn analyze_user_feedback(
        &self,
        feedback: &str,
        history_summary: &str,
    ) -> Result<FeedbackAnalysis>;
}

#[derive(Clone)]
pub struct OpenAILLMClient {
    pub client: reqwest::Client,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackAnalysis {
    pub action: String,
    pub new_goal: Option<String>,
}
