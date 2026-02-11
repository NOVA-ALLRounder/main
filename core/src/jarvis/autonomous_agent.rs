// Autonomous Agent - Self-directed AI assistant
// Based on ReAct pattern: Reason ??Act ??Observe

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::sleep;

use crate::domains::intelligence::llm_gateway::LLMClient;
use crate::jarvis::observers::ObserverManager;

/// Agent's current state in the autonomous loop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentState {
    Idle,
    Observing,
    Reasoning,
    Planning,
    Executing,
    Reflecting,
    WaitingForApproval,
}

/// Observation from the environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub source: String,      // "email", "file", "system"
    pub event_type: String,  // "new_email", "file_changed", etc.
    pub data: Value,
    pub timestamp: String,
}

/// Agent's reasoning about what to do
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reasoning {
    pub observation: String,
    pub analysis: String,
    pub decision: String,     // "take_action", "ignore", "wait"
    pub confidence: f32,      // 0.0 - 1.0
}

/// Multi-step plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub goal: String,
    pub steps: Vec<PlanStep>,
    pub estimated_duration: String,
    pub risk_level: String,   // "low", "medium", "high"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub step_number: usize,
    pub description: String,
    pub tool: String,
    pub action: String,
    pub params: Value,
    pub requires_approval: bool,
}

/// Result of plan execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub steps_completed: usize,
    pub steps_failed: usize,
    pub logs: Vec<String>,
    pub final_state: String,
}

/// Autonomous Agent - Runs continuously in background
pub struct AutonomousAgent {
    llm: LLMClient,
    state: AgentState,
    current_plan: Option<Plan>,
    observation_buffer: Vec<Observation>,
    observer_manager: ObserverManager,

    // Guardrails
    _max_cost_per_hour: f32,
    requires_approval_for: Vec<String>,  // e.g., ["send_email", "delete_file"]
    allowed_tools: Vec<String>,
}

impl AutonomousAgent {
    pub fn new() -> Result<Self> {
        let llm = LLMClient::new()?;

        // Initialize observers
        let mut observer_manager = ObserverManager::new();

        // Register email observer
        if let Ok(email_obs) = crate::jarvis::observers::email_observer::EmailObserver::new() {
            observer_manager.register(Box::new(email_obs));
        }

        // Register file observer
        let watch_paths = vec![
            std::env::var("USERPROFILE").unwrap_or_default() + "\\Downloads",
            std::env::var("USERPROFILE").unwrap_or_default() + "\\Documents",
        ];
        let file_obs = crate::jarvis::observers::file_observer::FileObserver::new(watch_paths);
        observer_manager.register(Box::new(file_obs));

        // Register system observer
        let system_obs = crate::jarvis::observers::system_observer::SystemObserver::new();
        observer_manager.register(Box::new(system_obs));

        Ok(Self {
            llm,
            state: AgentState::Idle,
            current_plan: None,
            observation_buffer: Vec::new(),
            observer_manager,
            _max_cost_per_hour: 1.0,  // $1/hour limit
            requires_approval_for: vec![
                "send_email".to_string(),
                "delete_file".to_string(),
                "execute_script".to_string(),
            ],
            allowed_tools: vec![
                "windows".to_string(),
                "email".to_string(),
                "excel".to_string(),
            ],
        })
    }

    /// Main autonomous loop (ReAct pattern)
    pub async fn run_loop(&mut self) -> Result<()> {
        log::info!("?夷?Autonomous Agent starting...");

        loop {
            match self.state {
                AgentState::Idle => {
                    // Wait for observations
                    sleep(Duration::from_secs(30)).await;
                    self.state = AgentState::Observing;
                }

                AgentState::Observing => {
                    // Collect observations from environment
                    let observations = self.observe_environment().await?;

                    if observations.is_empty() {
                        log::debug!("No new observations, returning to idle");
                        self.state = AgentState::Idle;
                    } else {
                        log::info!("?諭?Collected {} observations", observations.len());
                        self.observation_buffer = observations;
                        self.state = AgentState::Reasoning;
                    }
                }

                AgentState::Reasoning => {
                    // Use LLM to reason about observations
                    let reasoning = self.reason_about_observations().await?;

                    log::info!("?裕?Decision: {}", reasoning.decision);
                    log::info!("   Confidence: {:.0}%", reasoning.confidence * 100.0);

                    match reasoning.decision.as_str() {
                        "take_action" => {
                            self.state = AgentState::Planning;
                        }
                        "ignore" | "wait" => {
                            self.observation_buffer.clear();
                            self.state = AgentState::Idle;
                        }
                        _ => {
                            log::warn!("Unknown decision: {}", reasoning.decision);
                            self.state = AgentState::Idle;
                        }
                    }
                }

                AgentState::Planning => {
                    // Create multi-step plan
                    let plan = self.create_plan().await?;

                    log::info!("?諭?Created plan: {}", plan.goal);
                    log::info!("   Steps: {}", plan.steps.len());
                    log::info!("   Risk: {}", plan.risk_level);

                    self.current_plan = Some(plan.clone());

                    // Check if any step requires approval
                    let needs_approval = plan.steps.iter().any(|s| s.requires_approval);

                    if needs_approval {
                        log::warn!("?醫묓닔  Plan requires user approval");
                        self.state = AgentState::WaitingForApproval;
                    } else {
                        self.state = AgentState::Executing;
                    }
                }

                AgentState::Executing => {
                    // Execute the plan step by step
                    if let Some(plan) = &self.current_plan {
                        let result = self.execute_plan(plan).await?;

                        log::info!("??Execution complete: {}/{} steps succeeded",
                            result.steps_completed,
                            plan.steps.len()
                        );

                        self.state = AgentState::Reflecting;
                    } else {
                        self.state = AgentState::Idle;
                    }
                }

                AgentState::Reflecting => {
                    // Reflect on what happened and learn
                    self.reflect_on_execution().await?;

                    // Clear state
                    self.current_plan = None;
                    self.observation_buffer.clear();
                    self.state = AgentState::Idle;
                }

                AgentState::WaitingForApproval => {
                    // Wait for user to approve via UI
                    log::info!("?紐뚰닔  Waiting for user approval...");
                    sleep(Duration::from_secs(10)).await;
                    // TODO: Check approval status from database
                }
            }
        }
    }

    /// Observe environment for new events
    async fn observe_environment(&self) -> Result<Vec<Observation>> {
        // Collect from all registered observers
        let obs_observations = self.observer_manager.collect_all().await?;

        // Convert to our Observation type
        let observations: Vec<Observation> = obs_observations
            .into_iter()
            .map(|obs| Observation {
                source: obs.source,
                event_type: obs.event_type,
                data: obs.data,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
            .collect();

        if !observations.is_empty() {
            log::info!("?諭?Collected {} observations", observations.len());
        }

        Ok(observations)
    }

    /// Use LLM to reason about observations
    async fn reason_about_observations(&self) -> Result<Reasoning> {
        let observations_json = serde_json::to_string_pretty(&self.observation_buffer)?;

        let system_prompt = r#"You are an autonomous AI assistant. You observe events and decide whether to take action.

Your role:
- Analyze observations from email, files, and system events
- Decide if action is needed
- Be conservative - only act when confident

Respond with JSON:
{
  "observation": "brief summary of what you observed",
  "analysis": "your reasoning about what this means",
  "decision": "take_action" | "ignore" | "wait",
  "confidence": 0.0-1.0
}

Guidelines:
- "take_action" if this clearly needs a response (e.g., important email, urgent task)
- "ignore" if trivial or spam
- "wait" if you need more information
- confidence < 0.7 ??wait for more data
"#;

        let user_message = format!("Here are the observations:\n\n{}", observations_json);

        let messages = vec![
            json!({"role": "system", "content": system_prompt}),
            json!({"role": "user", "content": user_message})
        ];

        let response = self.llm.chat_completion_json(messages, Some("gpt-4o")).await?;
        let reasoning: Reasoning = serde_json::from_str(&response)?;

        Ok(reasoning)
    }

    /// Create multi-step plan based on observations
    async fn create_plan(&self) -> Result<Plan> {
        let observations_json = serde_json::to_string_pretty(&self.observation_buffer)?;

        let system_prompt = format!(r#"You are an autonomous AI assistant creating execution plans.

Available tools: {}
Actions requiring approval: {}

Create a plan with concrete steps. Each step must specify:
- step_number
- description
- tool (from available tools)
- action (e.g., "open_app", "send_email", "process_excel")
- params (JSON object)
- requires_approval (true if action is in approval list)

Respond with JSON:
{{
  "goal": "what you're trying to achieve",
  "steps": [...],
  "estimated_duration": "e.g., 2 minutes",
  "risk_level": "low" | "medium" | "high"
}}

Keep plans simple. 3-5 steps maximum.
"#,
            self.allowed_tools.join(", "),
            self.requires_approval_for.join(", ")
        );

        let user_message = format!("Create a plan based on:\n\n{}", observations_json);

        let messages = vec![
            json!({"role": "system", "content": system_prompt}),
            json!({"role": "user", "content": user_message})
        ];

        let response = self.llm.chat_completion_json(messages, Some("gpt-4o")).await?;
        let plan: Plan = serde_json::from_str(&response)?;

        Ok(plan)
    }

    /// Execute plan step by step
    async fn execute_plan(&self, plan: &Plan) -> Result<ExecutionResult> {
        let mut logs = Vec::new();
        let mut steps_completed = 0;
        let steps_failed = 0;

        for step in &plan.steps {
            log::info!("??고닔  Step {}: {}", step.step_number, step.description);

            // TODO: Execute actual tool calls
            // For now, just simulate
            logs.push(format!("Executed: {} - {}", step.tool, step.action));
            steps_completed += 1;

            sleep(Duration::from_millis(500)).await;
        }

        Ok(ExecutionResult {
            success: steps_failed == 0,
            steps_completed,
            steps_failed,
            logs,
            final_state: "completed".to_string(),
        })
    }

    /// Reflect on execution and learn
    async fn reflect_on_execution(&self) -> Result<()> {
        // TODO: Implement reflection pattern
        // - What worked well?
        // - What could be improved?
        // - Update internal knowledge base

        log::info!("?夷?Reflecting on execution...");

        Ok(())
    }
}
