use super::*;

pub(super) enum RunExecutionDisposition {
    Continue,
    GoalCompleted,
    Abort(String),
}

impl Planner {
    pub(super) fn prepare_action_execution(
        step_index: usize,
        plan_key: &str,
        plan: &serde_json::Value,
        history: &mut Vec<String>,
        action_history: &mut Vec<String>,
        last_action_by_plan: &mut HashMap<String, String>,
        repeated_loop_hits: &mut usize,
        max_repeat_loop_hits: usize,
        node_capture_enabled: bool,
        node_capture_dir: Option<&PathBuf>,
        node_capture_seq: &mut usize,
    ) -> Result<RunExecutionDisposition> {
        let action_str = plan.to_string();
        if LoopDetector::detect_high_risk_repetition(action_history, &action_str) {
            println!("   🛑 LOOP BLOCKED. High-risk repeated plan suppressed before execution.");
            history.push(format!(
                "LOOP_BLOCKED: high_risk_repeated_plan={}",
                action_str
            ));
            return Ok(RunExecutionDisposition::Continue);
        }
        if LoopDetector::detect_action_loop(action_history, &action_str) {
            let action_preview: String = action_str.chars().take(220).collect();
            println!(
                "   🔄 LOOP DETECTED. Recording context and retrying with same action family. action={}",
                action_preview
            );
            history.push(format!("LOOP_DETECTED: repeated_plan={}", action_str));
            *repeated_loop_hits += 1;
            if *repeated_loop_hits >= max_repeat_loop_hits {
                let msg = format!(
                    "Planner aborted due to repeated loop detections (hits={} limit={}).",
                    repeated_loop_hits, max_repeat_loop_hits
                );
                history.push(format!("LOOP_ABORTED: {}", msg));
                return Ok(RunExecutionDisposition::Abort(msg));
            }
        } else {
            *repeated_loop_hits = 0;
        }
        action_history.push(action_str);
        last_action_by_plan.insert(
            plan_key.to_string(),
            plan["action"].as_str().unwrap_or("unknown").to_string(),
        );

        if plan["action"].as_str() == Some("wait") && Self::has_quota_exhaustion_marker(history) {
            let msg =
                "Planner aborted: provider quota/rate-limit detected and wait-loop suppressed.";
            history.push(format!("QUOTA_ABORTED: {}", msg));
            return Ok(RunExecutionDisposition::Abort(msg.to_string()));
        }

        if plan["action"].as_str() == Some("done") {
            if node_capture_enabled {
                if let Some(dir) = node_capture_dir {
                    *node_capture_seq += 1;
                    let _ = util::capture_node_evidence(
                        dir,
                        *node_capture_seq,
                        step_index,
                        "goal_done",
                        plan,
                        "planner_done",
                    );
                }
            }
            println!("✅ Goal completed by planner.");
            return Ok(RunExecutionDisposition::GoalCompleted);
        }

        Ok(RunExecutionDisposition::Continue)
    }

    pub(super) async fn execute_planned_action(
        &self,
        step_index: usize,
        goal: &str,
        plan: &serde_json::Value,
        session_steps: &mut Vec<SmartStep>,
        session: &mut Session,
        history: &mut Vec<String>,
        consecutive_failures: &mut usize,
        last_read_number: &mut Option<String>,
        timing: &mut PlannerTimingStats,
        node_capture_enabled: bool,
        node_capture_all: bool,
        node_capture_dir: Option<&PathBuf>,
        node_capture_seq: &mut usize,
        last_opened_app: &mut Option<String>,
    ) -> RunExecutionDisposition {
        println!("   🚀 Executing Action...");
        let execute_started = Instant::now();
        let execute_result = ActionRunner::execute(
            plan,
            &mut VisualDriver::new(),
            Some(&*self.llm),
            session_steps,
            session,
            history,
            consecutive_failures,
            last_read_number,
            goal,
        )
        .await;
        let execute_elapsed = execute_started.elapsed();
        timing.record_execute(execute_elapsed);
        history.push(format!(
            "TIMING|step={}|phase=execute|ms={}",
            step_index,
            execute_elapsed.as_millis()
        ));

        let mut abort_due_to_execution_error: Option<String> = None;
        if let Err(e) = &execute_result {
            println!("   ❌ Execution Error: {}", e);
            history.push(format!("EXECUTION_ERROR: {}", e));
            if Self::should_abort_on_execution_error(e) {
                let action_name = plan["action"].as_str().unwrap_or("unknown");
                abort_due_to_execution_error = Some(format!(
                    "Planner aborted after execution error at step {} (action={}): {}",
                    step_index, action_name, e
                ));
            }
        }

        if node_capture_enabled {
            if let Some(dir) = node_capture_dir {
                let action_name = plan["action"].as_str().unwrap_or("unknown");
                let mut should_capture = node_capture_all;
                let mut phase = "post_action";
                let mut note = "action_executed".to_string();

                if action_name == "open_app" {
                    should_capture = true;
                    phase = "app_node";
                    let current_app = plan["name"]
                        .as_str()
                        .or_else(|| plan["app"].as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    note = if let Some(prev_app) = last_opened_app.as_ref() {
                        if prev_app.eq_ignore_ascii_case(&current_app) {
                            format!("app_reopen_{}", current_app)
                        } else {
                            format!("transition_{}_to_{}", prev_app, current_app)
                        }
                    } else {
                        format!("app_entry_{}", current_app)
                    };
                    *last_opened_app = Some(current_app);
                } else if execute_result.is_err() {
                    should_capture = true;
                    phase = "execution_error";
                    note = "action_failed".to_string();
                }

                if should_capture {
                    *node_capture_seq += 1;
                    let _ = util::capture_node_evidence(
                        dir,
                        *node_capture_seq,
                        step_index,
                        phase,
                        plan,
                        &note,
                    );
                }
            }
        }

        if let Some(abort_msg) = abort_due_to_execution_error {
            return RunExecutionDisposition::Abort(abort_msg);
        }

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
                    "step": step_index,
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

        RunExecutionDisposition::Continue
    }
}
