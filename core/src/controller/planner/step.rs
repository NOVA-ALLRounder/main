use super::*;

impl Planner {
    pub(super) fn observe_step(
        step_index: usize,
        goal: &str,
        max_attempts_per_plan_key: usize,
        timing: &mut PlannerTimingStats,
        history: &mut Vec<String>,
        plan_attempts: &mut HashMap<String, usize>,
    ) -> Result<PlannerStepObservation> {
        let capture_started = Instant::now();
        let (image_b64, _) = VisualDriver::capture_screen()?;
        let capture_elapsed = capture_started.elapsed();
        timing.record_capture(capture_elapsed);
        history.push(format!(
            "TIMING|step={}|phase=capture|ms={}",
            step_index,
            capture_elapsed.as_millis()
        ));

        let plan_key = heuristics::compute_plan_key(goal, &image_b64);
        let attempt = plan_attempts
            .entry(plan_key.clone())
            .and_modify(|value| *value += 1)
            .or_insert(1);
        if *attempt > max_attempts_per_plan_key {
            let msg = format!(
                "Planner exceeded max attempts for same screen state: attempts={} limit={} plan_key={}",
                *attempt, max_attempts_per_plan_key, plan_key
            );
            history.push(format!("PLAN_ATTEMPT_LIMIT: {}", msg));
            return Err(anyhow::anyhow!(msg));
        }

        Ok(PlannerStepObservation {
            image_b64,
            plan_key,
        })
    }

    pub(super) async fn plan_step(
        &self,
        step_index: usize,
        goal: &str,
        image_b64: &str,
        plan_key: &str,
        attempt: usize,
        consecutive_failures: usize,
        scenario_mode: bool,
        deterministic_goal_mode: bool,
        history: &mut Vec<String>,
        last_action_by_plan: &HashMap<String, String>,
        timing: &mut PlannerTimingStats,
    ) -> Result<serde_json::Value> {
        let retry_config = util::planner_retry_config();
        let mut history_with_context = history.clone();
        if attempt > 1 || consecutive_failures > 0 {
            let last_action = last_action_by_plan
                .get(plan_key)
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            let last_error = history
                .iter()
                .rev()
                .find(|entry| entry.starts_with("FAILED") || entry.starts_with("BLOCKED"))
                .cloned()
                .unwrap_or_else(|| "none".to_string());
            let context = format!(
                "RETRY_CONTEXT: attempt={} plan_key={} last_action={} last_error={}",
                attempt, plan_key, last_action, last_error
            );
            history_with_context.push(context);
        }

        let plan_started = Instant::now();
        let plan = if scenario_mode || deterministic_goal_mode {
            Self::fallback_plan_from_goal(goal, &history_with_context)
                .unwrap_or_else(|| serde_json::json!({ "action": "done" }))
        } else {
            let plan_timeout =
                Duration::from_secs(Self::env_u64("STEER_PLANNER_PLAN_TIMEOUT_SEC", 10));
            let primary_result =
                crate::retry_logic::with_retry(&retry_config, "LLM Vision", || async {
                    tokio::time::timeout(
                        plan_timeout,
                        self.llm
                            .plan_vision_step(goal, image_b64, &history_with_context),
                    )
                    .await
                    .map_err(|_| {
                        anyhow::anyhow!(
                            "planner plan_vision_step timeout after {}s",
                            plan_timeout.as_secs()
                        )
                    })?
                })
                .await;

            match primary_result {
                Ok(value) => value,
                Err(error) => {
                    let err_text = error.to_string();
                    history.push(format!("PLAN_PRIMARY_FAILED: {}", err_text));
                    if let Some(recovered) = self
                        .recover_plan_after_primary_failure(goal, &history_with_context, &err_text)
                        .await
                    {
                        if let Some(action) = recovered["action"].as_str() {
                            history.push(format!("PLAN_RECOVERY_ACTION: {}", action));
                        } else {
                            history.push("PLAN_RECOVERY_ACTION: unknown".to_string());
                        }
                        recovered
                    } else {
                        return Err(error);
                    }
                }
            }
        };

        let plan_elapsed = plan_started.elapsed();
        timing.record_plan(plan_elapsed);
        history.push(format!(
            "TIMING|step={}|phase=plan|ms={}",
            step_index,
            plan_elapsed.as_millis()
        ));

        Ok(plan)
    }
}
