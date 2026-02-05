use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use crate::llm_gateway::LLMClient;
use crate::visual_driver::{VisualDriver, SmartStep};
use crate::controller::supervisor::Supervisor;
use crate::controller::loop_detector::LoopDetector;
use crate::controller::heuristics;
use crate::controller::actions::ActionRunner;
use crate::session_store::Session;
use crate::schema::EventEnvelope;
use chrono::Utc;
use uuid::Uuid;
use tokio::sync::mpsc;
use std::sync::Arc;
use crate::action_schema;

pub struct Planner {
    pub llm: Arc<dyn LLMClient>,
    pub max_steps: usize,
    pub tx: Option<mpsc::Sender<String>>,
}

impl Planner {
    pub fn new(llm: Arc<dyn LLMClient>, tx: Option<mpsc::Sender<String>>) -> Self {
        Self {
            llm,
            max_steps: 25,
            tx,
        }
    }

    pub async fn run_goal(&self, goal: &str, session_key: Option<&str>) -> Result<()> {
        println!("🌊 Starting Planned Surf: '{}'", goal);

        // [Session]
        let _ = crate::session_store::init_session_store();
        let mut session = Session::new(goal, session_key);
        session.add_message("user", goal);

        // [Preflight]
        if let Err(e) = heuristics::preflight_permissions() {
             println!("❌ Preflight failed: {}", e);
             return Err(e);
        }
        if let Err(e) = heuristics::verify_screen_capture() {
             return Err(e);
        }

        let mut history: Vec<String> = Vec::new();
        let mut action_history: Vec<String> = Vec::new(); // For loop detection
        let mut plan_attempts: HashMap<String, usize> = HashMap::new();
        let mut consecutive_failures = 0;
        let mut last_read_number: Option<String> = None;
        let mut session_steps: Vec<SmartStep> = Vec::new();
        let mut last_action_by_plan: HashMap<String, String> = HashMap::new();

        for i in 1..=self.max_steps {
            println!("\n🔄 [Step {}/{}] Observing...", i, self.max_steps);
            
            // 1. Capture Screen
            let (image_b64, _) = VisualDriver::capture_screen()?;
            let plan_key = heuristics::compute_plan_key(goal, &image_b64);
            let attempt = plan_attempts.entry(plan_key.clone()).and_modify(|v| *v += 1).or_insert(1);
            
            // Preflight: close blocking dialogs
            if heuristics::try_close_front_dialog() {
                history.push("Closed blocking dialog".to_string());
                continue;
            }

            // 2. Plan (Think)
            let retry_config = crate::retry_logic::RetryConfig::default();
            let mut history_with_context = history.clone();
             if *attempt > 1 || consecutive_failures > 0 {
                let last_action = last_action_by_plan.get(&plan_key).cloned().unwrap_or_else(|| "unknown".to_string());
                let last_error = history.iter().rev().find(|h| h.starts_with("FAILED") || h.starts_with("BLOCKED"))
                    .cloned().unwrap_or_else(|| "none".to_string());
                let context = format!(
                    "RETRY_CONTEXT: attempt={} plan_key={} last_action={} last_error={}",
                    attempt, plan_key, last_action, last_error
                );
                history_with_context.push(context);
            }

            // Call LLM for Vision Planning
            let mut plan = crate::retry_logic::with_retry(&retry_config, "LLM Vision", || async {
                self.llm.plan_vision_step(goal, &image_b64, &history_with_context).await
            }).await?;

             // Flatten nested JSON
            if plan["action"].is_object() {
                plan = plan["action"].clone();
            }
            
            // Validate Schema
            let validation = action_schema::normalize_action(&plan);
            if let Some(err) = validation.error {
                 let msg = format!("SCHEMA_ERROR: {}", err);
                 println!("   ⚠️ {}", msg);
                 history.push(msg);
                 consecutive_failures += 1;
                 continue;
            }
             plan = validation.normalized;

            // 3. Supervisor Check
            let supervisor_decision = crate::retry_logic::with_retry(&retry_config, "Supervisor", || async {
                 Supervisor::consult(&*self.llm, goal, &plan, &history).await
            }).await?;

             println!("   🕵️ Supervisor: {} ({})", supervisor_decision.action, supervisor_decision.reason);

            match supervisor_decision.action.as_str() {
                "accept" => { /* Proceed */ },
                "review" => {
                    history.push(format!("PLAN_REJECTED: {}", supervisor_decision.notes));
                    continue; 
                },
                "escalate" => {
                    println!("      🚨 Supervisor ESCALATED: {}", supervisor_decision.reason);
                    break;
                },
                _ => {}
            }

            // 4. Anti-Loop Check
            let action_str = plan.to_string();
            if LoopDetector::detect_action_loop(&action_history, &action_str) {
                 println!("   🔄 LOOP DETECTED. Supervisor/Heuristics should handle this next.");
                 plan = serde_json::json!({"action": "report", "message": "Loop detected. Halting."});
            }
            action_history.push(action_str.clone());
            last_action_by_plan.insert(plan_key.clone(), plan["action"].as_str().unwrap_or("unknown").to_string());

            // 5. Execute via ActionRunner
            println!("   🚀 Executing Action...");
            if let Err(e) = ActionRunner::execute(
                &plan,
                &mut VisualDriver::new(), // In real scenario, might want to reuse driver or pass it
                &mut session_steps,
                &mut session,
                &mut history,
                &mut consecutive_failures,
                &mut last_read_number,
                goal
            ).await {
                println!("   ❌ Execution Error: {}", e);
                // logic to handle specific errors or break
            }
            
            // Broadcast event if tx available
             if let Some(tx) = &self.tx {
                let event = EventEnvelope {
                    schema_version: "1.0".to_string(),
                    event_id: Uuid::new_v4().to_string(),
                    ts: Utc::now().to_rfc3339(),
                    source: "dynamic_agent".to_string(),
                    app: "Agent".to_string(),
                    event_type: "action".to_string(),
                    priority: "P1".to_string(),
                    resource: None,
                    payload: serde_json::json!({
                        "goal": goal,
                        "step": i,
                        "plan": plan
                    }),
                    privacy: None,
                    pid: None,
                    window_id: None,
                    window_title: None,
                    browser_url: None,
                    raw: None,
                };
                if let Ok(json) = serde_json::to_string(&event) {
                    let _ = tx.try_send(json);
                }
            }
        }

        Ok(())
    }
}
