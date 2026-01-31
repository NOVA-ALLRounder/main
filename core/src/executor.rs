use anyhow::{Result, Context};
use crate::llm_gateway::LLMClient;
use crate::{command_queue, db, replanning_config};
use crate::visual_driver::{VisualDriver, SmartStep, UiAction};
use std::sync::Arc;
use tokio::sync::Mutex; 

pub struct AgentExecutor {
    llm: Arc<LLMClient>,
    driver: Arc<Mutex<VisualDriver>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanStep {
    pub description: String,
    pub action_type: String, // "CLICK", "TYPE", "URL", "WAIT"
    pub target: Option<String>,
    pub value: Option<String>,
    pub verification: String, // Post-check
    pub pre_check: Option<String>, // [NEW] Pre-check
}

impl AgentExecutor {
    pub fn new(llm: LLMClient) -> Self {
        Self {
            llm: Arc::new(llm),
            driver: Arc::new(Mutex::new(VisualDriver::new())),
        }
    }

    /// Primary OODA Loop
    pub async fn execute_goal(&self, goal: &str) -> Result<String> {
        println!("🧠 [OODA] Goal received: '{}'", goal);

        // 1. OBSERVE: Capture current state (omitted for MVP start, assuming start state)
        
        // 2. ORIENT & DECIDE: Generate Plan
        let mut plan = self.generate_plan(goal).await?;
        println!("🧠 [OODA] Plan generated with {} steps.", plan.len());

        let mut step_index: usize = 0;
        let mut replan_attempts: u32 = 0;
        let max_replans = env_u32("EXECUTOR_MAX_REPLANS", 1);

        // 3. ACT: Execute each step with SmartDriver
        'outer: while step_index < plan.len() {
            let step = plan[step_index].clone();
            println!("🧠 [OODA] Executing Step {}: {}", step_index + 1, step.description);
            
            let _driver = self.driver.lock().await;
            // Clear previous steps to run one by one (or batch them if desired)
            // For OODA, running, verify, then next is safer.
            // driver.clear_steps(); // (Future: Implement clear_steps in VisualDriver)
            
            let action = match step.action_type.as_str() {
                "CLICK" => UiAction::Click(step.target.clone().unwrap_or_default()),
                "TYPE" => UiAction::Type(step.value.clone().unwrap_or_default()),
                "URL" => UiAction::OpenUrl(step.value.clone().unwrap_or_default()),
                "WAIT" => UiAction::Wait(step.value.as_ref().and_then(|v| v.parse().ok()).unwrap_or(2)),
                "SCROLL" => UiAction::Scroll(step.value.clone().unwrap_or_else(|| "down".to_string())),
                "ACTIVATE" => UiAction::ActivateApp(step.value.clone().unwrap_or_else(|| "frontmost".to_string())),
                _ => UiAction::Wait(1),
            };

            let smart_step = SmartStep::new(action, &step.description)
                .with_pre_check(&step.pre_check.clone().unwrap_or_default())
                .with_post_check(&step.verification);
                
            // [Self-Healing Loop]
            let mut attempts = 0;
            let max_retries = env_u32("EXECUTOR_MAX_RETRIES", 2);
            let mut last_error: Option<anyhow::Error> = None;
            let mut last_failure_type = "execution_error";
            
            while attempts <= max_retries {
                // Hack: Create a temporary mini-driver for this step to ensure isolation
                let mut step_driver = VisualDriver::new();
                step_driver.add_step(smart_step.clone());
                
                    match step_driver.execute(Some(&self.llm)).await {
                    Ok(_) => {
                        println!("✅ Step {} Success.", step_index + 1);
                        last_error = None;
                        last_failure_type = "Success";
                        break;
                    },
                    Err(e) => {
                        attempts += 1;
                        let failure_type = classify_failure(&e.to_string());
                        last_failure_type = failure_type;
                        println!("⚠️ Step {} Failed [{}] (Attempt {}/{}): {}", step_index + 1, failure_type, attempts, max_retries + 1, e);
                        last_error = Some(e);
                        
                        if failure_type == "permission_denied" {
                            println!("⛔️ Critical Permission Error. Aborting Self-Healing to prevent spamming OS.");
                            return Err(last_error.unwrap()); // Fail Fast on permissions
                        }

                        if attempts <= max_retries {
                            println!("🩹 [Self-Healing] Rerying...");
                            tokio::time::sleep(tokio::time::Duration::from_millis(500 * (attempts as u64))).await;
                        }
                    }
                }
            }

            if last_error.is_none() {
                step_index += 1;
                continue;
            }

            let strategy = replanning_config::get_replan_strategy(last_failure_type);
            if strategy.stop {
                println!("⛔️ Replan stopped: {}", strategy.reason);
                return Err(anyhow::anyhow!(strategy.reason));
            }

            if replan_attempts < max_replans {
                println!("🧭 [Replan] Attempting replanning after failure: {}", last_failure_type);
                let mut new_plan = crate::replan_templates::build_replan_steps(last_failure_type, &step);
                if new_plan.is_empty() {
                    if let Ok(llm_plan) = self.generate_plan_with_feedback(goal, &step, last_failure_type).await {
                        new_plan = llm_plan;
                    }
                }
                if !new_plan.is_empty() {
                    plan = new_plan;
                    step_index = 0;
                    replan_attempts += 1;
                    continue 'outer;
                }
            }

            println!("❌ Step {} Failed permanently.", step_index + 1);
            return Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Executor loop terminated without specific error")));
        }

        Ok("Goal Completed".to_string())
    }

    async fn generate_plan_with_feedback(&self, goal: &str, failed_step: &PlanStep, failure_type: &str) -> Result<Vec<PlanStep>> {
        let strategy = replanning_config::get_replan_strategy(failure_type);
        let hint = strategy.fix_hint.unwrap_or("");
        let prompt = format!(
            "You are an autonomous GUI Agent. The previous plan failed.\n\
            Goal: '{}'.\n\
            Failed step: '{}' (type: {}, target: {:?}, value: {:?}).\n\
            Failure type: {}.\n\
            Strategy hint: {}.\n\
            Replan with safer, simpler steps that avoid the failure.\n\
            Available Actions: CLICK(target), TYPE(text), URL(link), WAIT(seconds), SCROLL(direction), ACTIVATE(app).\n\
            Pre-Check: Visual cue to verify action is possible.\n\
            Verification: Key visual cue to check success.\n\n\
            Output ONLY valid JSON array of objects:\n\
            [{{ \"description\": \"...\", \"action_type\": \"CLICK\", \"target\": \"Login Button\", \"pre_check\": \"Login page visible\", \"verification\": \"Login form appears\" }}, ...]",
            goal,
            failed_step.description,
            failed_step.action_type,
            failed_step.target,
            failed_step.value,
            failure_type,
            hint
        );

        let response = self.llm.analyze_tendency(&[prompt]).await?;
        let start = response.find('[').unwrap_or(0);
        let end = response.rfind(']').map(|i| i + 1).unwrap_or(response.len());
        let sliced = if start < end { response[start..end].to_string() } else { response };
        let cleaned = sliced.replace("```json", "").replace("```", "").trim().to_string();
        let steps: Vec<PlanStep> = serde_json::from_str(&cleaned)
            .context(format!("Failed to parse replan JSON: {}", cleaned))?;
        Ok(steps)
    }

    async fn generate_plan(&self, goal: &str) -> Result<Vec<PlanStep>> {
        let prompt = format!(
            "You are an autonomous GUI Agent. Your goal is: '{}'.\n\
            Break this goal down into a linear sequence of concrete computer actions for macOS.\n\
            Available Actions: CLICK(target), TYPE(text), URL(link), WAIT(seconds), SCROLL(direction), ACTIVATE(app).\n\
            Pre-Check: Visual cue to verify action is possible (e.g. 'Search bar visible').\n\
            Verification: Key visual cue to check success (e.g. 'Results appeared').\n\n\
            Output ONLY valid JSON array of objects:\n\
            [{{ \"description\": \"...\", \"action_type\": \"CLICK\", \"target\": \"Login Button\", \"pre_check\": \"Login page visible\", \"verification\": \"Login form appears\" }}, ...]",
            goal
        );

        // Mock JSON return for MVP fallback or real LLM call
        // Here we call the LLM
        // We reuse the build_n8n_workflow method's logic or add a new general JSON method.
        // For now, let's use a specialized method call or just generic completion.
        // Assuming we need to add `generate_plan_json` to LLMClient, or parse it here.
        
        // Using existing generic analyze method? No, let's assume we implement a helper.
        // For MVP, implementing a dummy plan for testing if LLM not connected optimally.
        
        let response = match self.llm.analyze_tendency(&[prompt]).await {
            Ok(json_str) => {
                // Extract JSON array from Markdown or raw text
                let start = json_str.find('[').unwrap_or(0);
                let end = json_str.rfind(']').map(|i| i + 1).unwrap_or(json_str.len());
                if start < end {
                    json_str[start..end].to_string()
                } else {
                    json_str
                }
            },
            Err(_) => return Err(anyhow::anyhow!("Plan generation failed")),
        };
        
        // Clean JSON formatting (remove markdown blocks)
        let cleaned = response.replace("```json", "").replace("```", "").trim().to_string();
        
        let steps: Vec<PlanStep> = serde_json::from_str(&cleaned)
            .context(format!("Failed to parse plan JSON: {}", cleaned))?;
            
        Ok(steps)
    }
}

fn env_u32(key: &str, default_val: u32) -> u32 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default_val)
}

fn classify_failure(err: &str) -> &'static str {
    let msg = err.to_lowercase();
    if msg.contains("timeout") { "timeout" }
    else if msg.contains("permission") || msg.contains("access") { "permission_denied" }
    else if msg.contains("network") || msg.contains("connection") || msg.contains("dns") { "network_error" }
    else if msg.contains("not found") { "element_missing" }
    else if msg.contains("verification failed") { classify_verify_failure(&msg) }
    else if msg.contains("lint") && msg.contains("fail") { "lint_fail" }
    else if msg.contains("build") && msg.contains("fail") { "build_fail" }
    else if msg.contains("test") && msg.contains("fail") { "tests_fail" }
    else { "execution_error" }
}

fn classify_verify_failure(msg: &str) -> &'static str {
    if msg.contains("tests_pass") { "tests_fail" }
    else if msg.contains("lint_pass") { "lint_fail" }
    else if msg.contains("build_success") { "build_fail" }
    else { "verify_fail" }
}

// --- Utility Functions (Legacy Support) ---

pub fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .with_context(|| format!("Failed to open URL: {}", url))?;
    Ok(())
}

pub async fn run_shell(cmd: &str) -> Result<String> {
    let workdir = std::env::current_dir()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());
    let mut action = crate::shell_actions::ShellAction {
        instruction: cmd.to_string(),
        targets: Vec::new(),
        verify: Vec::new(),
    };
    action = crate::shell_actions::sanitize_shell_action(action, &workdir);
    let cmd = action.instruction.clone();

    let allow_composites = env_bool("SHELL_ALLOW_COMPOSITES", false);
    let allow_substitution = env_bool("SHELL_ALLOW_SUBSTITUTION", false);
    let analysis = crate::shell_analysis::analyze_shell_command(&cmd);
    if analysis.has_substitution && !allow_substitution {
        return Err(anyhow::anyhow!("❌ Command substitution is blocked for safety."));
    }
    if analysis.has_composites && !allow_composites {
        return Err(anyhow::anyhow!("❌ Composite commands are blocked for safety."));
    }

    let exec_record = db::create_exec_result(&cmd, Some(&workdir)).ok();

    let cmd_clone = cmd.clone();
    let workdir_clone = workdir.clone();
    let action_clone = action.clone();

    let result = command_queue::enqueue_command_in_lane(
        "shell",
        Box::new(move || {
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd_clone)
                .output()
                .with_context(|| format!("Failed to run command: {}", cmd_clone))?;

            if output.status.success() {
                let result = String::from_utf8_lossy(&output.stdout).to_string();
                if !action_clone.verify.is_empty() {
                    let verify = crate::shell_actions::verify_shell_action(&action_clone, &result, &workdir_clone);
                    if !verify.success {
                        let reasons = verify
                            .verdicts
                            .iter()
                            .filter(|v| !v.ok)
                            .map(|v| v.reason.clone())
                            .collect::<Vec<_>>()
                            .join(", ");
                        return Err(anyhow::anyhow!("Verification failed: {}", reasons));
                    }
                }
                Ok(result)
            } else {
                Err(anyhow::anyhow!("Command failed: {}", String::from_utf8_lossy(&output.stderr)))
            }
        }),
        None,
    )
    .await;

    if let Some(record) = exec_record {
        match &result {
            Ok(output) => {
                let _ = db::update_exec_result(&record.id, "success", Some(output), None);
            }
            Err(err) => {
                let _ = db::update_exec_result(&record.id, "error", None, Some(&err.to_string()));
            }
        }
    }

    result
}

fn env_bool(key: &str, default_val: bool) -> bool {
    match std::env::var(key) {
        Ok(v) => {
            let v = v.trim().to_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        }
        Err(_) => default_val,
    }
}
