use super::*;

#[path = "provider_impl/analysis.rs"]
mod analysis;
#[path = "provider_impl/intent.rs"]
mod intent;
#[path = "provider_impl/planning.rs"]
mod planning;
#[path = "provider_impl/vision.rs"]
mod vision;
#[path = "provider_impl/workflow.rs"]
mod workflow;

#[async_trait]
impl LLMClient for OpenAILLMClient {
    #[allow(dead_code)]
    async fn plan_next_step(
        &self,
        goal: &str,
        ui_tree: &Value,
        action_history: &[String],
    ) -> Result<Value> {
        planning::plan_next_step(self, goal, ui_tree, action_history).await
    }

    /// Generic Chat Completion (for Architect/Chat features)
    /// Falls back to CLI LLM chain (Gemini→Codex→llama-server) on 429.
    async fn chat_completion(&self, messages: Vec<Value>) -> Result<String> {
        if Self::cli_first_enabled() {
            eprintln!(
                "ℹ️ [chat_completion] CLI-first mode enabled (STEER_LLM_PRIMARY={}).",
                Self::llm_primary_mode().unwrap_or_else(|| "cli".to_string())
            );
            return crate::cli_llm::fallback_chat_completion(&messages).await;
        }

        let body = json!({
            "model": self.model,
            "messages": messages
        });

        let res_json = match self
            .post_with_retry("https://api.openai.com/v1/chat/completions", &body)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "⚠️ [chat_completion] OpenAI error/rate-limit: {}. Invoking fallback chain...",
                    e
                );
                return crate::cli_llm::fallback_chat_completion(&messages).await;
            }
        };

        let content = res_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(content)
    }

    /// Plan the next step using Vision (Screenshots) instead of DOM tree
    async fn plan_vision_step(
        &self,
        goal: &str,
        image_b64: &str,
        history: &[String],
    ) -> Result<Value> {
        planning::plan_vision_step(self, goal, image_b64, history).await
    }

    async fn analyze_routine(&self, logs: &[String]) -> Result<String> {
        analysis::analyze_routine(self, logs).await
    }

    async fn recommend_automation(&self, logs: &[String]) -> Result<String> {
        analysis::recommend_automation(self, logs).await
    }

    async fn build_n8n_workflow(&self, user_prompt: &str) -> Result<String> {
        workflow::build_n8n_workflow(self, user_prompt).await
    }

    async fn fix_n8n_workflow(
        &self,
        user_prompt: &str,
        bad_json: &str,
        error_msg: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        workflow::fix_n8n_workflow(self, user_prompt, bad_json, error_msg).await
    }

    /// Analyze screen content using Vision API
    async fn analyze_screen(
        &self,
        prompt: &str,
        image_b64: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        vision::analyze_screen(self, prompt, image_b64).await
    }

    /// Find coordinates of a UI element using Vision API
    async fn find_element_coordinates(
        &self,
        element_description: &str,
        image_b64: &str,
    ) -> Result<Option<(i32, i32)>> {
        vision::find_element_coordinates(self, element_description, image_b64).await
    }

    async fn score_quality(
        &self,
        system_prompt: &str,
        payload: &serde_json::Value,
    ) -> Result<String> {
        vision::score_quality(self, system_prompt, payload).await
    }

    async fn propose_workflow(
        &self,
        logs: &[String],
    ) -> Result<AutomationProposal, Box<dyn std::error::Error>> {
        workflow::propose_workflow(self, logs).await
    }

    async fn analyze_tendency(&self, logs: &[String]) -> Result<String> {
        analysis::analyze_tendency(self, logs).await
    }

    async fn parse_intent(&self, user_input: &str) -> Result<Value> {
        intent::parse_intent(self, user_input).await
    }

    async fn parse_intent_with_history(
        &self,
        user_input: &str,
        history: &[crate::db::ChatMessage],
    ) -> Result<Value> {
        intent::parse_intent_with_history(self, user_input, history).await
    }

    /// Generate embeddings for RAG
    async fn get_embedding(&self, text: &str) -> Result<Vec<f32>> {
        intent::get_embedding(self, text).await
    }

    /// Generate workflow recommendation from detected pattern
    async fn generate_recommendation_from_pattern(
        &self,
        pattern_description: &str,
        sample_events: &[String],
    ) -> Result<AutomationProposal> {
        workflow::generate_recommendation_from_pattern(self, pattern_description, sample_events)
            .await
    }

    /// Proactively suggest a tech stack or approach for a goal (Transformers7 feature)
    #[allow(dead_code)]
    async fn propose_solution_stack(&self, goal: &str) -> Result<Value> {
        analysis::propose_solution_stack(self, goal).await
    }

    async fn inference_local(&self, prompt: &str, model: Option<&str>) -> Result<String> {
        intent::inference_local(self, prompt, model).await
    }

    /// Smart Router: Decide between Cloud (OpenAI) and Local (Ollama)
    /// Returns: (use_local: bool, model_name: &str)
    fn route_task(&self, task_description: &str, pii_detected: bool) -> (bool, String) {
        intent::route_task(self, task_description, pii_detected)
    }

    async fn analyze_user_feedback(
        &self,
        feedback: &str,
        history_summary: &str,
    ) -> Result<FeedbackAnalysis> {
        analysis::analyze_user_feedback(self, feedback, history_summary).await
    }
}
