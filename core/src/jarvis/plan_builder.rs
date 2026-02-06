// Plan Builder - Multi-step task planning
// Phase 4: Enhanced orchestrator

use crate::jarvis::intent_classifier::Intent;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub description: String,
    pub steps: Vec<PlanStep>,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub step_number: usize,
    pub description: String,
    pub skill: String,
    pub action: String,
    pub params: serde_json::Value,
    pub requires_approval: bool,
    pub status: StepStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StepStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

pub struct PlanBuilder {
    llm: Option<Arc<crate::domains::intelligence::llm_gateway::LLMClient>>,
}

impl PlanBuilder {
    pub fn new() -> Self {
        Self { llm: None }
    }

    pub fn with_llm(llm: Arc<crate::domains::intelligence::llm_gateway::LLMClient>) -> Self {
        Self { llm: Some(llm) }
    }

    /// Build execution plan from intent
    pub async fn build(&self, intent: Intent, original_text: &str) -> Result<ExecutionPlan> {
        match intent {
            Intent::SimpleTask {
                skill,
                action,
                confidence,
            } => {
                // Single-step plan
                Ok(ExecutionPlan {
                    description: format!("Execute {} action on {}", action, skill),
                    steps: vec![PlanStep {
                        step_number: 1,
                        description: original_text.to_string(),
                        skill: skill.clone(),
                        action: action.clone(),
                        params: self.extract_params(original_text, &skill, &action)?,
                        requires_approval: self.requires_approval(&skill, &action),
                        status: StepStatus::Pending,
                    }],
                    requires_approval: self.requires_approval(&skill, &action),
                })
            }

            Intent::ComplexTask {
                description,
                estimated_steps,
                ..
            } => {
                // Multi-step plan - Use LLM if available
                let steps = self.decompose_complex_task(original_text).await?;

                Ok(ExecutionPlan {
                    description: description.clone(),
                    steps,
                    requires_approval: true, // Always require approval for complex tasks
                })
            }

            Intent::Query { topic, .. } => {
                // Query plan - typically just web search or knowledge retrieval
                Ok(ExecutionPlan {
                    description: format!("Answer query about: {}", topic),
                    steps: vec![PlanStep {
                        step_number: 1,
                        description: "Search for information".to_string(),
                        skill: "web_search".to_string(),
                        action: "search".to_string(),
                        params: serde_json::json!({ "query": original_text }),
                        requires_approval: false,
                        status: StepStatus::Pending,
                    }],
                    requires_approval: false,
                })
            }

            Intent::Conversation | Intent::Unknown => {
                // No plan needed
                anyhow::bail!("No execution plan needed for conversation/unknown intent")
            }
        }
    }

    fn extract_params(
        &self,
        text: &str,
        skill: &str,
        action: &str,
    ) -> Result<serde_json::Value> {
        // Simple parameter extraction
        let mut params = serde_json::Map::new();

        match (skill, action) {
            ("computer_use", "open_app") => {
                // Extract app name
                if text.contains("메모장") || text.contains("notepad") {
                    params.insert("app".to_string(), serde_json::Value::String("notepad".to_string()));
                } else if text.contains("크롬") || text.contains("chrome") {
                    params.insert("app".to_string(), serde_json::Value::String("chrome".to_string()));
                }
            }

            ("email", "send") => {
                // Extract recipient and subject (simple pattern matching)
                // TODO: Better NER for email extraction
                params.insert("to".to_string(), serde_json::Value::String("".to_string()));
                params.insert("subject".to_string(), serde_json::Value::String(text.to_string()));
            }

            ("telegram", "send") => {
                // Extract message content
                params.insert("message".to_string(), serde_json::Value::String(text.to_string()));
            }

            _ => {
                // Default: pass full text as content
                params.insert("content".to_string(), serde_json::Value::String(text.to_string()));
            }
        }

        Ok(serde_json::Value::Object(params))
    }

    fn requires_approval(&self, skill: &str, action: &str) -> bool {
        // Define which actions require approval
        match (skill, action) {
            ("email", "send") => true,
            ("telegram", "send") => true,
            ("computer_use", "execute") => true,
            ("computer_use", "delete") => true,
            _ => false,
        }
    }

    async fn decompose_complex_task(&self, text: &str) -> Result<Vec<PlanStep>> {
        // Try LLM-based decomposition first (if available)
        if let Some(llm) = &self.llm {
            if let Ok(steps) = self.decompose_with_llm(llm, text).await {
                return Ok(steps);
            }
        }

        // Fallback to rule-based decomposition
        let mut steps = Vec::new();

        // Example: "이메일 확인하고 요약해줘"
        if text.contains("이메일") && text.contains("확인") {
            steps.push(PlanStep {
                step_number: 1,
                description: "Check emails".to_string(),
                skill: "email".to_string(),
                action: "list".to_string(),
                params: serde_json::json!({"limit": 10}),
                requires_approval: false,
                status: StepStatus::Pending,
            });
        }

        if text.contains("요약") {
            steps.push(PlanStep {
                step_number: steps.len() + 1,
                description: "Summarize content".to_string(),
                skill: "llm".to_string(),
                action: "summarize".to_string(),
                params: serde_json::json!({}),
                requires_approval: false,
                status: StepStatus::Pending,
            });
        }

        // If no steps extracted, create a generic plan
        if steps.is_empty() {
            steps.push(PlanStep {
                step_number: 1,
                description: text.to_string(),
                skill: "llm".to_string(),
                action: "general".to_string(),
                params: serde_json::json!({"query": text}),
                requires_approval: false,
                status: StepStatus::Pending,
            });
        }

        Ok(steps)
    }

    /// LLM-based task decomposition
    async fn decompose_with_llm(
        &self,
        llm: &crate::domains::intelligence::llm_gateway::LLMClient,
        text: &str,
    ) -> Result<Vec<PlanStep>> {
        let prompt = json!([
            {
                "role": "system",
                "content": "You are a task planner. Break down the user's complex task into a sequence of steps.

Available skills and actions:
- computer_use: open_app, click, type, screenshot, key
- email: send, list, search, read
- telegram: send, get_updates

Respond with a JSON array of steps:
[
  {
    \"step_number\": 1,
    \"description\": \"Check email inbox\",
    \"skill\": \"email\",
    \"action\": \"list\",
    \"params\": {\"limit\": 10},
    \"requires_approval\": false
  },
  {
    \"step_number\": 2,
    \"description\": \"Summarize emails\",
    \"skill\": \"llm\",
    \"action\": \"summarize\",
    \"params\": {},
    \"requires_approval\": false
  }
]

Keep steps actionable and specific. Use appropriate skills for each step."
            },
            {
                "role": "user",
                "content": format!("Break down this task into steps: {}", text)
            }
        ]);

        let response = llm.chat_completion_json(prompt.as_array().unwrap().clone(), Some("gpt-4o-mini")).await?;
        let parsed: Value = serde_json::from_str(&response)?;

        let steps_array = parsed["steps"]
            .as_array()
            .ok_or_else(|| {
                log::error!("LLM response missing 'steps' array: {:?}", parsed);
                anyhow::anyhow!("Invalid LLM response format: missing steps array")
            })?;

        let mut steps = Vec::new();
        for (idx, step_val) in steps_array.iter().enumerate() {
            // Validate required fields with proper error handling
            let step_number = step_val["step_number"]
                .as_u64()
                .unwrap_or((idx + 1) as u64) as usize;

            let description = step_val["description"]
                .as_str()
                .ok_or_else(|| {
                    log::error!("Step {} missing description: {:?}", idx + 1, step_val);
                    anyhow::anyhow!("Step {} missing required field: description", idx + 1)
                })?
                .to_string();

            let skill = step_val["skill"]
                .as_str()
                .ok_or_else(|| {
                    log::error!("Step {} missing skill: {:?}", idx + 1, step_val);
                    anyhow::anyhow!("Step {} missing required field: skill", idx + 1)
                })?
                .to_string();

            let action = step_val["action"]
                .as_str()
                .ok_or_else(|| {
                    log::error!("Step {} missing action: {:?}", idx + 1, step_val);
                    anyhow::anyhow!("Step {} missing required field: action", idx + 1)
                })?
                .to_string();

            let params = step_val["params"].clone();
            let requires_approval = step_val["requires_approval"].as_bool().unwrap_or(false);

            let step = PlanStep {
                step_number,
                description,
                skill,
                action,
                params,
                requires_approval,
                status: StepStatus::Pending,
            };

            log::info!(
                "Parsed step {}: {} - {}.{}",
                step.step_number,
                step.description,
                step.skill,
                step.action
            );

            steps.push(step);
        }

        Ok(steps)
    }

    /// Update step status
    pub fn update_step_status(
        &self,
        plan: &mut ExecutionPlan,
        step_number: usize,
        status: StepStatus,
    ) {
        if let Some(step) = plan.steps.iter_mut().find(|s| s.step_number == step_number) {
            step.status = status;
        }
    }

    /// Get next pending step
    pub fn get_next_step<'a>(&self, plan: &'a ExecutionPlan) -> Option<&'a PlanStep> {
        plan.steps.iter().find(|s| s.status == StepStatus::Pending)
    }
}

impl Default for PlanBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jarvis::intent_classifier::Intent;

    #[tokio::test]
    async fn test_simple_plan() {
        let builder = PlanBuilder::new();
        let intent = Intent::SimpleTask {
            skill: "computer_use".to_string(),
            action: "open_app".to_string(),
            confidence: 0.9,
        };

        let plan = builder.build(intent, "메모장 열어").await.unwrap();
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].skill, "computer_use");
    }

    #[tokio::test]
    async fn test_complex_plan() {
        let builder = PlanBuilder::new();
        let intent = Intent::ComplexTask {
            description: "Check and summarize emails".to_string(),
            estimated_steps: 2,
            confidence: 0.7,
        };

        let plan = builder.build(intent, "이메일 확인하고 요약해줘").await.unwrap();
        assert!(plan.steps.len() >= 2);
        assert!(plan.requires_approval);
    }
}
