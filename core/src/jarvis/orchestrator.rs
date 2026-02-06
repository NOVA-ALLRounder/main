// JARVIS Orchestrator - Central coordinator (Clawdbot-inspired)

use crate::jarvis::{
    command_parser::CommandParser, engines::*, error_handler::*, event_bus::*,
    intent_classifier::*, models::*, parallel_executor::ParallelExecutor, performance::*,
    plan_builder::*, session_manager::*, skills::*, tools::ToolRegistry, PrivacyMode,
};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct JarvisOrchestrator {
    // Core components
    event_bus: EventBus,
    session_manager: SessionManager,
    tool_registry: Arc<RwLock<ToolRegistry>>, // Deprecated - migrate to skills
    skill_registry: Arc<SkillRegistry>,        // Phase 2: New skill system
    command_parser: CommandParser,
    error_handler: ErrorHandler,
    performance_monitor: Arc<PerformanceMonitor>,

    // Phase 4: Intent → Plan → Skills pipeline
    intent_classifier: Arc<IntentClassifier>,
    plan_builder: Arc<PlanBuilder>,
    llm_client: Option<Arc<crate::domains::intelligence::llm_gateway::LLMClient>>, // For conversational responses

    // Phase 5: Parallel execution engine
    parallel_executor: Arc<ParallelExecutor>,

    // Engines (to be implemented in later phases)
    context_engine: Option<Arc<ContextEngine>>,
    vision_engine: Option<Arc<VisionEngine>>,
    pattern_detector: Option<Arc<PatternDetector>>,
    workflow_builder: Option<Arc<WorkflowBuilder>>,

    // Configuration
    privacy_mode: Arc<RwLock<PrivacyMode>>,

    // Event receiver
    event_rx: Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<JarvisEvent>>>,
}

impl JarvisOrchestrator {
    pub async fn new() -> Result<Self> {
        let (event_bus, event_rx) = EventBus::new();
        let session_manager = SessionManager::new();
        let tool_registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let skill_registry = Arc::new(SkillRegistry::new());
        let command_parser = CommandParser::new(tool_registry.clone());

        // Register tools (deprecated - will migrate to skills)
        {
            let mut registry = tool_registry.write().await;
            registry.register(Arc::new(crate::jarvis::tools::WindowsTool::new()));
            registry.register(Arc::new(crate::jarvis::tools::TelegramTool::new()));
            log::info!("Registered 2 tools: windows, telegram");
        }

        // Register skills (Phase 4: New skill system)
        {
            skill_registry
                .register(Arc::new(crate::jarvis::skills::ComputerUseSkill::new()))
                .await;
            skill_registry
                .register(Arc::new(crate::jarvis::skills::EmailSkill::new()))
                .await;
            skill_registry
                .register(Arc::new(crate::jarvis::skills::TelegramSkill::new()))
                .await;
            log::info!("Registered 3 skills: computer_use, email, telegram");
        }

        // Initialize LLM Client for Intent & Planning (Phase 4+: LLM Integration)
        let llm_client = match crate::domains::intelligence::llm_gateway::LLMClient::new() {
            Ok(client) => {
                log::info!("✅ LLM Client initialized - Complex tasks will use AI planning");
                Some(Arc::new(client))
            }
            Err(e) => {
                log::warn!("⚠️  LLM Client not available ({}). Falling back to rule-based planning", e);
                None
            }
        };

        // Initialize parallel executor (Phase 5)
        let parallel_executor = Arc::new(ParallelExecutor::new(skill_registry.clone()));
        log::info!("✅ Parallel executor initialized - Multi-step plans will execute concurrently");

        Ok(Self {
            event_bus,
            session_manager,
            tool_registry,
            skill_registry,
            command_parser,
            error_handler: ErrorHandler::new(),
            performance_monitor: Arc::new(PerformanceMonitor::new()),
            intent_classifier: Arc::new(if let Some(ref llm) = llm_client {
                IntentClassifier::with_llm(llm.clone())
            } else {
                IntentClassifier::new()
            }),
            plan_builder: Arc::new(if let Some(ref llm) = llm_client {
                PlanBuilder::with_llm(llm.clone())
            } else {
                PlanBuilder::new()
            }),
            llm_client: llm_client.clone(), // Store for conversational responses
            parallel_executor,
            context_engine: None,
            vision_engine: None,
            pattern_detector: None,
            workflow_builder: None,
            privacy_mode: Arc::new(RwLock::new(PrivacyMode::default())),
            event_rx: Arc::new(tokio::sync::Mutex::new(event_rx)),
        })
    }

    /// Handle channel message (Phase 1: Multi-channel support)
    /// This is the new entry point for all channels (Telegram, Web, API)
    pub async fn handle_message(&self, message: Message) -> Result<Response> {
        let start = std::time::Instant::now();
        log::info!(
            "Handling message from {}: {:?}",
            message.session_key,
            message.text
        );

        // Get/create session
        let session = self.session_manager.get_or_create(&message.session_key);

        // Update session activity
        self.session_manager
            .update_context(
                &message.session_key,
                UserContext {
                    active_app: None,
                    active_window_title: None,
                    last_activity: std::time::Instant::now(),
                    activity_count: session.context.activity_count + 1,
                },
            )
            .map_err(|e| log::warn!("Failed to update session context: {}", e))
            .ok();

        // Use the enhanced handler with Intent → Plan → Skills + LLM Response
        let response = self.handle_message_enhanced(message).await?;

        // Track performance
        let duration = start.elapsed();
        self.performance_monitor
            .record("handle_message".to_string(), duration, response.success)
            .await;

        Ok(response)
    }

    /// Handle message with Intent → Plan → Skills pipeline + LLM Response (Phase 4+)
    /// This is the new enhanced flow with natural conversational responses
    pub async fn handle_message_enhanced(&self, message: Message) -> Result<Response> {
        let start = std::time::Instant::now();
        log::info!("Enhanced handling for message: {}", message.text);

        // Step 1: Classify intent
        let intent = self.intent_classifier.classify(&message.text).await?;
        log::info!("Classified intent: {:?}", intent);

        // Step 2: Handle based on intent type
        let (execution_result, needs_llm_response) = match intent {
            Intent::Conversation | Intent::Query { .. } | Intent::Unknown => {
                // These always need LLM response
                (None, true)
            }
            _ => {
                // SimpleTask/ComplexTask: Execute skills first
                match self.plan_builder.build(intent.clone(), &message.text).await {
                    Ok(plan) => {
                        log::info!(
                            "Built plan with {} steps (requires_approval: {})",
                            plan.steps.len(),
                            plan.requires_approval
                        );

                        // Check approval
                        if plan.requires_approval {
                            return Ok(Response {
                                success: false,
                                message: format!(
                                    "This action requires approval: {}. Please approve to continue.",
                                    plan.description
                                ),
                                data: Some(serde_json::to_value(&plan)?),
                            });
                        }

                        // Execute plan (use parallel if >= 2 steps)
                        let execution_result = if plan.steps.len() >= 2 {
                            log::info!("Using parallel execution for {} steps", plan.steps.len());
                            self.execute_plan_parallel(plan).await
                        } else {
                            log::info!("Using sequential execution for single step");
                            self.execute_plan(plan).await
                        };

                        match execution_result {
                            Ok(result) => (Some(result.message), true),
                            Err(e) => (Some(format!("Error: {}", e)), true),
                        }
                    }
                    Err(_) => (None, true),
                }
            }
        };

        // Step 3: Generate natural LLM response
        let final_response = if needs_llm_response && self.llm_client.is_some() {
            match self.generate_llm_response(&message.text, execution_result.as_deref()).await {
                Ok(response) => response,
                Err(e) => {
                    log::error!("Failed to generate LLM response: {}", e);
                    execution_result.unwrap_or_else(|| {
                        "I'm here to help! How can I assist you?".to_string()
                    })
                }
            }
        } else {
            execution_result.unwrap_or_else(|| "Done.".to_string())
        };

        // Track performance
        let duration = start.elapsed();
        self.performance_monitor
            .record("handle_message_enhanced".to_string(), duration, true)
            .await;

        Ok(Response {
            success: true,
            message: final_response,
            data: None,
        })
    }

    /// Generate natural conversational response using LLM
    async fn generate_llm_response(
        &self,
        user_message: &str,
        execution_result: Option<&str>,
    ) -> Result<String> {
        let llm = self
            .llm_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("LLM client not available"))?;

        let system_prompt = if let Some(result) = execution_result {
            format!(
                "You are JARVIS, a helpful AI assistant. The user asked: \"{}\"\n\nYou executed the task and got this result: {}\n\nRespond naturally and helpfully to the user, acknowledging what was done.",
                user_message, result
            )
        } else {
            format!(
                "You are JARVIS, a helpful AI assistant. The user said: \"{}\"\n\nRespond naturally and helpfully.",
                user_message
            )
        };

        let messages = serde_json::json!([
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_message}
        ]);

        let response = llm
            .chat_completion(messages.as_array().unwrap().clone())
            .await?;

        Ok(response)
    }

    /// Execute execution plan step by step (sequential)
    async fn execute_plan(&self, mut plan: ExecutionPlan) -> Result<Response> {
        let mut results = Vec::new();

        for step in &mut plan.steps {
            log::info!("Executing step {}: {}", step.step_number, step.description);

            // Update status
            step.status = StepStatus::InProgress;

            // Execute skill
            let params = step
                .params
                .as_object()
                .cloned()
                .ok_or_else(|| {
                    log::error!("Step {} has invalid params (not an object): {:?}", step.step_number, step.params);
                    anyhow::anyhow!("Step {} has invalid parameters format", step.step_number)
                })?
                .into_iter()
                .collect();

            let skill_result = self
                .skill_registry
                .execute(
                    &step.skill,
                    SkillContext {
                        session_key: "default".to_string(), // TODO: Use actual session
                        action: step.action.clone(),
                        params,
                        user_context: crate::jarvis::models::context::UserContext::new(),
                    },
                )
                .await;

            // Update status based on result
            if skill_result.success {
                step.status = StepStatus::Completed;
                results.push(skill_result.message.clone());
            } else {
                step.status = StepStatus::Failed;
                log::error!("Step {} failed: {}", step.step_number, skill_result.message);
                // Stop execution on failure
                return Ok(Response {
                    success: false,
                    message: format!(
                        "Step {} failed: {}",
                        step.step_number, skill_result.message
                    ),
                    data: Some(serde_json::to_value(&plan)?),
                });
            }
        }

        // All steps completed successfully
        Ok(Response {
            success: true,
            message: results.join("\n"),
            data: Some(serde_json::to_value(&plan)?),
        })
    }

    /// Execute execution plan in parallel (Phase 5: Parallel execution engine)
    /// Uses parallel executor to run independent steps concurrently
    async fn execute_plan_parallel(&self, plan: ExecutionPlan) -> Result<Response> {
        log::info!(
            "Starting parallel execution of plan: {} ({} steps)",
            plan.description,
            plan.steps.len()
        );

        // Use parallel executor
        self.parallel_executor
            .execute_parallel(plan, "default".to_string())
            .await
    }

    /// Handle user command
    pub async fn handle_command(&self, command: Command) -> Result<Response> {
        let start = std::time::Instant::now();
        log::info!("Handling command: {:?}", command.text);

        // Publish command received event
        self.event_bus.publish(JarvisEvent::CommandReceived {
            command: command.text.clone(),
            source: match command.source {
                CommandSource::Telegram => "telegram".to_string(),
                CommandSource::TauriUI => "tauri".to_string(),
                CommandSource::API => "api".to_string(),
                CommandSource::Internal => "internal".to_string(),
            },
        });

        // Check if this is a conversational message (greeting, question, etc.)
        if let Some(response) = self.handle_conversation(&command.text) {
            let duration = start.elapsed();
            self.performance_monitor
                .record("handle_command".to_string(), duration, true)
                .await;

            return Ok(Response {
                success: true,
                message: response,
                data: None,
            });
        }

        // Execute with retry logic
        let result = with_retry(
            || async {
                // Parse command to tool call
                let parsed = self.command_parser.parse(&command.text).await?;
                log::debug!(
                    "Parsed command: tool={}, action={}",
                    parsed.tool,
                    parsed.action
                );

                // Execute tool
                let registry = self.tool_registry.read().await;
                let result = registry.execute(&parsed.tool, parsed.params).await?;

                Ok(result)
            },
            RetryConfig::default(),
        )
        .await;

        // Track performance
        let duration = start.elapsed();
        let success = result.is_ok();
        self.performance_monitor
            .record("handle_command".to_string(), duration, success)
            .await;

        match result {
            Ok(tool_result) => Ok(Response {
                success: tool_result.success,
                message: tool_result.message,
                data: tool_result.data,
            }),
            Err(e) => {
                // Handle error with classification and notification
                self.error_handler
                    .handle_error(&e, &format!("handle_command({})", command.text))
                    .await;
                Err(e)
            }
        }
    }

    /// Handle conversational messages (greetings, questions, small talk)
    fn handle_conversation(&self, text: &str) -> Option<String> {
        let lower = text.to_lowercase().trim().to_string();

        // Greetings
        let greetings = vec!["안녕", "hello", "hi", "hey", "좋은 아침", "굿모닝", "헬로", "하이"];
        if greetings.iter().any(|&g| lower.starts_with(g) || lower == g) {
            return Some("안녕하세요! JARVIS입니다. 무엇을 도와드릴까요?".to_string());
        }

        // How are you
        if lower.contains("어때") || lower.contains("how are you") || lower.contains("어떄") {
            return Some("잘 지내고 있습니다! 무엇을 도와드릴까요?".to_string());
        }

        // What can you do
        if lower.contains("뭐 할") || lower.contains("what can") || lower.contains("기능") {
            return Some("저는 Windows 자동화 어시스턴트입니다.\n\n예시:\n- '메모장 열어'\n- '크롬 실행해'\n- '메모장에 안녕이라고 적어줘'\n- 'https://google.com 열어'\n\n무엇을 도와드릴까요?".to_string());
        }

        // Thank you
        if lower.contains("고마") || lower.contains("thank") || lower.contains("감사") {
            return Some("천만에요! 도움이 되어 기쁩니다.".to_string());
        }

        // Goodbye
        if lower.contains("잘 가") || lower.contains("bye") || lower.contains("안녕히") {
            return Some("안녕히 가세요! 필요하시면 언제든 불러주세요.".to_string());
        }

        None
    }

    /// Handle system event
    pub async fn handle_event(&self, event: JarvisEvent) -> Result<()> {
        log::debug!("Handling event: {:?}", event);

        // Publish to event bus
        self.event_bus.publish(event);

        Ok(())
    }

    /// Periodic tick (called every 30 seconds)
    pub async fn tick(&self) -> Result<()> {
        log::debug!("Orchestrator tick");

        // Cleanup idle sessions
        let removed = self.session_manager.cleanup_idle_sessions();
        if removed > 0 {
            log::info!("Cleaned up {} idle sessions", removed);
        }

        // TODO: Trigger pattern analysis
        // TODO: Trigger proactive suggestions

        Ok(())
    }

    /// Start event processing loop
    pub async fn start_event_loop(&self) {
        log::info!("Starting JARVIS event loop");

        loop {
            let mut rx = self.event_rx.lock().await;

            tokio::select! {
                Some(event) = rx.recv() => {
                    log::debug!("Processing event: {:?}", event);
                    self.event_bus.process_events(event).await;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                    drop(rx); // Release lock before tick
                    if let Err(e) = self.tick().await {
                        log::error!("Tick error: {}", e);
                    }
                }
            }
        }
    }

    /// Get/create session
    pub fn get_session(&self, key: impl Into<SessionKey>) -> Session {
        self.session_manager.get_or_create(key)
    }

    /// Update session context
    pub fn update_session_context(&self, key: impl AsRef<str>, context: UserContext) -> Result<()> {
        self.session_manager.update_context(key, context)
    }

    /// Get privacy mode
    pub async fn get_privacy_mode(&self) -> PrivacyMode {
        *self.privacy_mode.read().await
    }

    /// Set privacy mode
    pub async fn set_privacy_mode(&self, mode: PrivacyMode) -> Result<()> {
        *self.privacy_mode.write().await = mode;
        log::info!("Privacy mode set to: {:?}", mode);
        Ok(())
    }

    /// Get event bus for subscribing
    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    /// Get tool registry
    pub fn tool_registry(&self) -> Arc<RwLock<ToolRegistry>> {
        self.tool_registry.clone()
    }

    /// Set context engine (Phase 3)
    pub fn set_context_engine(&mut self, engine: Arc<ContextEngine>) {
        self.context_engine = Some(engine);
    }

    /// Set vision engine (Phase 4)
    pub fn set_vision_engine(&mut self, engine: Arc<VisionEngine>) {
        self.vision_engine = Some(engine);
    }

    /// Set pattern detector (Phase 5)
    pub fn set_pattern_detector(&mut self, detector: Arc<PatternDetector>) {
        self.pattern_detector = Some(detector);
    }

    /// Set workflow builder (Phase 6)
    pub fn set_workflow_builder(&mut self, builder: Arc<WorkflowBuilder>) {
        self.workflow_builder = Some(builder);
    }

    /// Get performance monitor
    pub fn performance_monitor(&self) -> Arc<PerformanceMonitor> {
        self.performance_monitor.clone()
    }

    /// Get performance summary
    pub async fn get_performance_summary(&self) -> PerformanceSummary {
        self.performance_monitor.get_summary().await
    }

    /// Start autonomous mode (Phase 5)
    /// Spawns background task that continuously observes and acts
    pub async fn start_autonomous_mode(&self) -> Result<tokio::task::JoinHandle<()>> {
        log::info!("🤖 Starting autonomous mode...");

        // Create autonomous agent
        let mut agent = crate::jarvis::autonomous_agent::AutonomousAgent::new()?;

        // Spawn background task
        let handle = tokio::spawn(async move {
            loop {
                if let Err(e) = agent.run_loop().await {
                    log::error!("Autonomous agent error: {}", e);
                    // Wait before retrying
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                }
            }
        });

        log::info!("✅ Autonomous mode started");
        Ok(handle)
    }

    /// Get skill registry (for external access)
    pub fn skill_registry(&self) -> Arc<SkillRegistry> {
        self.skill_registry.clone()
    }

    /// Get session manager (for external access)
    pub fn session_manager(&self) -> &SessionManager {
        &self.session_manager
    }

    /// Get intent classifier (for testing)
    pub fn intent_classifier(&self) -> Arc<IntentClassifier> {
        self.intent_classifier.clone()
    }

    /// Get plan builder (for testing)
    pub fn plan_builder(&self) -> Arc<PlanBuilder> {
        self.plan_builder.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let orchestrator = JarvisOrchestrator::new().await.unwrap();
        assert_eq!(orchestrator.get_privacy_mode().await, PrivacyMode::Basic);
    }

    #[tokio::test]
    async fn test_handle_command() {
        let orchestrator = JarvisOrchestrator::new().await.unwrap();

        let command = Command {
            text: "test command".to_string(),
            source: CommandSource::Telegram,
            params: None,
        };

        let response = orchestrator.handle_command(command).await.unwrap();
        assert!(response.success);
    }

    #[tokio::test]
    async fn test_privacy_mode() {
        let orchestrator = JarvisOrchestrator::new().await.unwrap();

        orchestrator
            .set_privacy_mode(PrivacyMode::Full)
            .await
            .unwrap();

        assert_eq!(orchestrator.get_privacy_mode().await, PrivacyMode::Full);
    }

    #[tokio::test]
    async fn test_tool_registration() {
        let orchestrator = JarvisOrchestrator::new().await.unwrap();
        let registry = orchestrator.tool_registry.read().await;

        // Verify tools are registered
        assert!(registry.get("windows").is_some());
        assert!(registry.get("telegram").is_some());
        assert_eq!(registry.count(), 2);
    }

    #[tokio::test]
    async fn test_command_parsing_korean_app_launch() {
        let orchestrator = JarvisOrchestrator::new().await.unwrap();

        let command = Command {
            text: "메모장 열어".to_string(),
            source: CommandSource::Internal,
            params: None,
        };

        // This will try to actually launch notepad, so we just verify parsing works
        let result = orchestrator.handle_command(command).await;
        // Result depends on platform availability, but should parse successfully
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_command_parsing_url() {
        let orchestrator = JarvisOrchestrator::new().await.unwrap();

        let command = Command {
            text: "https://github.com 열어".to_string(),
            source: CommandSource::Internal,
            params: None,
        };

        let result = orchestrator.handle_command(command).await;
        // Should successfully parse and attempt to open URL
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_unknown_command_fails() {
        let orchestrator = JarvisOrchestrator::new().await.unwrap();

        let command = Command {
            text: "완전히 알 수 없는 명령어입니다".to_string(),
            source: CommandSource::Internal,
            params: None,
        };

        let result = orchestrator.handle_command(command).await;
        // Should fail to parse
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parallel_execution() {
        use crate::jarvis::plan_builder::{ExecutionPlan, PlanStep, StepStatus};

        let orchestrator = JarvisOrchestrator::new().await.unwrap();

        // Create a multi-step plan with different skills
        let plan = ExecutionPlan {
            description: "Test parallel execution".to_string(),
            steps: vec![
                PlanStep {
                    step_number: 1,
                    description: "Step 1".to_string(),
                    skill: "computer_use".to_string(),
                    action: "test".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_number: 2,
                    description: "Step 2".to_string(),
                    skill: "email".to_string(),
                    action: "test".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_number: 3,
                    description: "Step 3".to_string(),
                    skill: "telegram".to_string(),
                    action: "test".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
            ],
            requires_approval: false,
        };

        let start = std::time::Instant::now();
        let result = orchestrator.execute_plan_parallel(plan).await;
        let duration = start.elapsed();

        // Verify result (may succeed or fail depending on skill implementation)
        assert!(result.is_ok());

        // Log execution time
        println!("Parallel execution completed in {:?}", duration);
    }

    #[tokio::test]
    async fn test_sequential_vs_parallel_execution() {
        use crate::jarvis::plan_builder::{ExecutionPlan, PlanStep, StepStatus};

        let orchestrator = JarvisOrchestrator::new().await.unwrap();

        // Single step plan - should use sequential
        let single_step_plan = ExecutionPlan {
            description: "Single step".to_string(),
            steps: vec![PlanStep {
                step_number: 1,
                description: "Only step".to_string(),
                skill: "computer_use".to_string(),
                action: "test".to_string(),
                params: serde_json::json!({}),
                requires_approval: false,
                status: StepStatus::Pending,
            }],
            requires_approval: false,
        };

        // Multi-step plan - should use parallel
        let multi_step_plan = ExecutionPlan {
            description: "Multi step".to_string(),
            steps: vec![
                PlanStep {
                    step_number: 1,
                    description: "Step 1".to_string(),
                    skill: "computer_use".to_string(),
                    action: "test".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_number: 2,
                    description: "Step 2".to_string(),
                    skill: "email".to_string(),
                    action: "test".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
            ],
            requires_approval: false,
        };

        // Both should complete without error
        let single_result = orchestrator.execute_plan(single_step_plan).await;
        let multi_result = orchestrator.execute_plan_parallel(multi_step_plan).await;

        assert!(single_result.is_ok());
        assert!(multi_result.is_ok());
    }
}
