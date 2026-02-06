// Intent Classification - Understand user intent from messages
// Phase 4: Enhanced orchestrator

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Intent {
    /// Simple greeting or small talk
    Conversation,

    /// Single-action task (open app, send message, etc.)
    SimpleTask {
        skill: String,
        action: String,
        confidence: f32,
    },

    /// Multi-step complex task requiring planning
    ComplexTask {
        description: String,
        estimated_steps: usize,
        confidence: f32,
    },

    /// Question or information request
    Query {
        topic: String,
        confidence: f32,
    },

    /// Unknown or unclear intent
    Unknown,
}

pub struct IntentClassifier {
    llm: Option<Arc<crate::domains::intelligence::llm_gateway::LLMClient>>,
}

impl IntentClassifier {
    pub fn new() -> Self {
        Self { llm: None }
    }

    pub fn with_llm(llm: Arc<crate::domains::intelligence::llm_gateway::LLMClient>) -> Self {
        Self { llm: Some(llm) }
    }

    /// Classify user message intent (Hybrid: Rules + LLM)
    pub async fn classify(&self, text: &str) -> Result<Intent> {
        log::debug!("Classifying intent for text: {}", text);
        let lower = text.to_lowercase().trim().to_string();

        // 1. Check for greetings/conversation (fast path)
        if self.is_conversation(&lower) {
            return Ok(Intent::Conversation);
        }

        // 2. Check for simple task patterns (fast path)
        if let Some(intent) = self.classify_simple_task(&lower) {
            return Ok(intent);
        }

        // 3. Use LLM for complex classification (if available)
        if let Some(llm) = &self.llm {
            if let Ok(intent) = self.classify_with_llm(llm, text).await {
                return Ok(intent);
            }
        }

        // 4. Fallback to rule-based
        if self.is_query(&lower) {
            return Ok(Intent::Query {
                topic: text.to_string(),
                confidence: 0.7,
            });
        }

        if self.is_complex_task(&lower) {
            return Ok(Intent::ComplexTask {
                description: text.to_string(),
                estimated_steps: 3,
                confidence: 0.6,
            });
        }

        // Default: Unknown
        log::warn!("Could not classify intent for text: '{}', falling back to Unknown", text);
        Ok(Intent::Unknown)
    }

    /// LLM-based intent classification
    async fn classify_with_llm(
        &self,
        llm: &crate::domains::intelligence::llm_gateway::LLMClient,
        text: &str,
    ) -> Result<Intent> {
        let prompt = json!([
            {
                "role": "system",
                "content": "You are an intent classifier. Analyze the user's message and classify it into one of these intents:
1. Conversation - greetings, small talk
2. SimpleTask - single action (open app, send message, etc.) - specify skill and action
3. ComplexTask - multi-step task requiring planning - estimate steps
4. Query - question or information request

Available skills: computer_use (open_app, click, type, screenshot), email (send, list, search), telegram (send, get_updates)

Respond in JSON format:
{\"intent\": \"SimpleTask\", \"skill\": \"computer_use\", \"action\": \"open_app\", \"confidence\": 0.9}
OR
{\"intent\": \"ComplexTask\", \"description\": \"...\", \"steps\": 3, \"confidence\": 0.8}
OR
{\"intent\": \"Query\", \"topic\": \"...\", \"confidence\": 0.7}
OR
{\"intent\": \"Conversation\", \"confidence\": 1.0}"
            },
            {
                "role": "user",
                "content": text
            }
        ]);

        let prompt_array = prompt.as_array()
            .ok_or_else(|| anyhow::anyhow!("Invalid prompt format: expected array"))?;

        let response = llm.chat_completion_json(
            prompt_array.clone(),
            Some("gpt-4o-mini")
        ).await
        .map_err(|e| {
            log::error!("LLM intent classification failed: {}", e);
            anyhow::anyhow!("Intent classification error: {}", e)
        })?;

        let parsed: Value = serde_json::from_str(&response)?;

        // Parse LLM response into Intent
        match parsed["intent"].as_str() {
            Some("Conversation") => Ok(Intent::Conversation),
            Some("SimpleTask") => Ok(Intent::SimpleTask {
                skill: parsed["skill"].as_str().unwrap_or("").to_string(),
                action: parsed["action"].as_str().unwrap_or("").to_string(),
                confidence: parsed["confidence"].as_f64().unwrap_or(0.5) as f32,
            }),
            Some("ComplexTask") => Ok(Intent::ComplexTask {
                description: parsed["description"].as_str().unwrap_or(text).to_string(),
                estimated_steps: parsed["steps"].as_u64().unwrap_or(3) as usize,
                confidence: parsed["confidence"].as_f64().unwrap_or(0.6) as f32,
            }),
            Some("Query") => Ok(Intent::Query {
                topic: parsed["topic"].as_str().unwrap_or(text).to_string(),
                confidence: parsed["confidence"].as_f64().unwrap_or(0.7) as f32,
            }),
            _ => Ok(Intent::Unknown),
        }
    }

    fn is_conversation(&self, text: &str) -> bool {
        let greetings = vec![
            "안녕", "hello", "hi", "hey", "좋은 아침", "굿모닝", "헬로", "하이",
            "어때", "how are you", "어떄",
        ];
        greetings.iter().any(|&g| text.starts_with(g) || text == g)
    }

    fn classify_simple_task(&self, text: &str) -> Option<Intent> {
        // Windows tasks
        if text.contains("메모장") || text.contains("notepad") {
            return Some(Intent::SimpleTask {
                skill: "computer_use".to_string(),
                action: "open_app".to_string(),
                confidence: 0.9,
            });
        }

        if text.contains("크롬") || text.contains("chrome") {
            return Some(Intent::SimpleTask {
                skill: "computer_use".to_string(),
                action: "open_app".to_string(),
                confidence: 0.9,
            });
        }

        // Email tasks
        if text.contains("이메일") && text.contains("보내") {
            return Some(Intent::SimpleTask {
                skill: "email".to_string(),
                action: "send".to_string(),
                confidence: 0.8,
            });
        }

        // Check if it's actually a complex task (multiple actions)
        if text.contains("이메일") && text.contains("확인") && !self.is_complex_task(text) {
            return Some(Intent::SimpleTask {
                skill: "email".to_string(),
                action: "list".to_string(),
                confidence: 0.8,
            });
        }

        // Telegram tasks
        if text.contains("텔레그램") && text.contains("보내") {
            return Some(Intent::SimpleTask {
                skill: "telegram".to_string(),
                action: "send".to_string(),
                confidence: 0.8,
            });
        }

        None
    }

    fn is_query(&self, text: &str) -> bool {
        let query_markers = vec!["무엇", "뭐", "what", "어디", "where", "언제", "when", "왜", "why", "어떻게", "how"];
        query_markers.iter().any(|&m| text.contains(m))
    }

    fn is_complex_task(&self, text: &str) -> bool {
        // Detect complex tasks by looking for multiple verbs/actions
        let conjunctions = vec!["그리고", "and", "다음", "then", "후에", "after", "하고"];
        let has_multiple_actions = conjunctions.iter().any(|&c| text.contains(c));

        // Or tasks with multiple targets or multiple action verbs
        let action_verbs = vec!["확인", "보내", "열어", "만들", "삭제", "요약"];
        let action_count = action_verbs.iter().filter(|&&v| text.contains(v)).count();
        let has_multiple_targets = action_count >= 2;

        has_multiple_actions || has_multiple_targets
    }
}

impl Default for IntentClassifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_conversation_intent() {
        let classifier = IntentClassifier::new();
        let intent = classifier.classify("안녕하세요").await.unwrap();
        assert_eq!(intent, Intent::Conversation);
    }

    #[tokio::test]
    async fn test_simple_task_intent() {
        let classifier = IntentClassifier::new();
        let intent = classifier.classify("메모장 열어").await.unwrap();
        match intent {
            Intent::SimpleTask { skill, .. } => {
                assert_eq!(skill, "computer_use");
            }
            _ => panic!("Expected SimpleTask"),
        }
    }

    #[tokio::test]
    async fn test_complex_task_intent() {
        let classifier = IntentClassifier::new();
        let intent = classifier.classify("이메일 확인하고 요약해줘").await.unwrap();
        match intent {
            Intent::ComplexTask { .. } => {
                // Success
            }
            _ => panic!("Expected ComplexTask"),
        }
    }
}
