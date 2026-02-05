use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use anyhow::Result;
use crate::llm_gateway::LLMClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisorDecision {
    pub action: String, // "accept", "review", "escalate"
    pub reason: String,
    pub focus_keywords: Vec<String>,
    pub notes: String,
}

pub struct Supervisor;

impl Supervisor {
    pub async fn consult(
        llm: &dyn crate::llm_gateway::LLMClient,
        goal: &str,
        plan: &Value,
        history: &[String]
    ) -> Result<SupervisorDecision> {
        let system_prompt = crate::prompts::SUPERVISOR_SYSTEM_PROMPT;

        // Plan might be complex, simplify for prompt if needed
        let plan_str = serde_json::to_string_pretty(plan).unwrap_or_default();
        let history_str = history.join("\n");

        let user_msg = format!(
            "GOAL: {}\n\nHISTORY:\n{}\n\nPROPOSED ACTION:\n{}",
            goal,
            history_str,
            plan_str
        );

        let messages = vec![
            json!({ "role": "system", "content": system_prompt }),
            json!({ "role": "user", "content": user_msg })
        ];

        let content = llm.chat_completion(messages).await?;

        let decision: SupervisorDecision = serde_json::from_str(&content)?;
        Ok(decision)
    }
}
