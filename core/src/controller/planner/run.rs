use super::*;

impl Planner {
    pub async fn run_goal(&self, goal: &str, session_key: Option<&str>) -> Result<()> {
        let _ = self.run_goal_with_summary(goal, session_key).await?;
        Ok(())
    }

    pub(super) async fn run_goal_with_summary(
        &self,
        goal: &str,
        session_key: Option<&str>,
    ) -> Result<RunGoalExecutionSummary> {
        println!("🌊 Starting Planned Surf: '{}'", goal);
        let setup = self.prepare_goal_run(goal, session_key).await?;
        let scenario_mode = setup.scenario_mode;
        let deterministic_goal_mode = setup.deterministic_goal_mode;
        let allow_deterministic_fallback = setup.allow_deterministic_fallback;
        let allow_review_loop_override = setup.allow_review_loop_override;
        let node_capture_enabled = setup.node_capture_enabled;
        let node_capture_all = setup.node_capture_all;
        let node_capture_dir = setup.node_capture_dir;
        let mut session = setup.session;
        let mut history = setup.history;
        let mut node_capture_seq: usize = 0;
        let mut last_opened_app: Option<String> = None;

        let mut action_history: Vec<String> = Vec::new();
        let mut plan_attempts: HashMap<String, usize> = HashMap::new();
        let mut consecutive_failures = 0;
        let mut last_read_number: Option<String> = None;
        let mut session_steps: Vec<SmartStep> = Vec::new();
        let mut last_action_by_plan: HashMap<String, String> = HashMap::new();
        let mut repeated_loop_hits: usize = 0;
        let mut goal_completed = false;
        let mut timing = PlannerTimingStats::default();
        let run_started = Instant::now();
        let max_wall_duration = setup.max_wall_duration;
        let max_repeat_loop_hits = setup.max_repeat_loop_hits;
        let max_attempts_per_plan_key = setup.max_attempts_per_plan_key;

        for i in 1..=self.max_steps {
            if run_started.elapsed() > max_wall_duration {
                let msg = format!(
                    "Planner wall timeout after {}s (goal stalled without completion).",
                    max_wall_duration.as_secs()
                );
                history.push(format!("PLANNER_WALL_TIMEOUT: {}", msg));
                return Err(anyhow::anyhow!(msg));
            }
            println!("\n🔄 [Step {}/{}] Observing...", i, self.max_steps);

            let PlannerStepObservation {
                image_b64,
                plan_key,
            } = Self::observe_step(
                i,
                goal,
                max_attempts_per_plan_key,
                &mut timing,
                &mut history,
                &mut plan_attempts,
            )?;
            let attempt = plan_attempts.get(&plan_key).copied().unwrap_or(1);

            if heuristics::try_close_front_dialog() {
                history.push("Closed blocking dialog".to_string());
                continue;
            }

            let mut plan = self
                .plan_step(
                    i,
                    goal,
                    &image_b64,
                    &plan_key,
                    attempt,
                    consecutive_failures,
                    scenario_mode,
                    deterministic_goal_mode,
                    &mut history,
                    &last_action_by_plan,
                    &mut timing,
                )
                .await?;

            if plan["action"].is_object() {
                plan = plan["action"].clone();
            }

            Self::maybe_repair_open_app_missing_name(goal, &history, &mut plan);

            let validation = action_schema::normalize_action(&plan);
            if let Some(err) = validation.error {
                let msg = format!("SCHEMA_ERROR: {}", err);
                println!("   ⚠️ {}", msg);
                history.push(msg);
                consecutive_failures += 1;
                continue;
            }
            plan = validation.normalized;
            Self::maybe_rewrite_click_visual_to_app_action(goal, &history, &mut plan);
            Self::maybe_rewrite_click_visual_mail_body(&history, &mut plan);
            Self::maybe_rewrite_shortcut_to_next_app(goal, &history, &mut plan);
            Self::maybe_rewrite_redundant_new_item_shortcut(goal, &history, &mut plan);
            Self::maybe_rewrite_mail_subject_before_paste(goal, &history, &mut plan);
            Self::maybe_rewrite_open_app_to_pending_text_action(goal, &history, &mut plan);
            Self::maybe_rewrite_snapshot_to_progress_action(goal, &history, &mut plan);

            if scenario_mode {
                if let Some(fallback_plan) = Self::fallback_plan_from_goal(goal, &history) {
                    Self::record_fallback_action(&mut history, "scenario_mode", &fallback_plan);
                    plan = fallback_plan;
                }
            } else if deterministic_goal_mode {
                if let Some(det_plan) = Self::fallback_plan_from_goal(goal, &history) {
                    if let Some(action) = det_plan["action"].as_str() {
                        history.push(format!("DETERMINISTIC_PLAN_ACTION: {}", action));
                    }
                    plan = det_plan;
                }
            } else {
                match self
                    .review_plan_with_supervisor(
                        i,
                        goal,
                        plan,
                        &mut history,
                        &mut timing,
                        allow_deterministic_fallback,
                        allow_review_loop_override,
                    )
                    .await?
                {
                    super::run_supervisor::SupervisorDisposition::Accept(reviewed_plan) => {
                        plan = reviewed_plan;
                    }
                    super::run_supervisor::SupervisorDisposition::Review(notes) => {
                        history.push(format!("PLAN_REJECTED: {}", notes));
                        continue;
                    }
                    super::run_supervisor::SupervisorDisposition::Escalate(msg) => {
                        println!("      🚨 {}", msg);
                        return Err(anyhow::anyhow!(msg));
                    }
                }
            }

            if let Err(e) = Self::enforce_fallback_checkpoint(&mut history) {
                let mut summary =
                    Self::summarize_execution(goal, &session, &history, false, &timing);
                summary.approval_required = true;
                summary.business_complete = false;
                summary.business_note = format!("approval checkpoint required: {}", e);
                session.status = SessionStatus::Paused;
                let _ = crate::session_store::save_session(&session);
                return Ok(summary);
            }

            match Self::prepare_action_execution(
                i,
                &plan_key,
                &plan,
                &mut history,
                &mut action_history,
                &mut last_action_by_plan,
                &mut repeated_loop_hits,
                max_repeat_loop_hits,
                node_capture_enabled,
                node_capture_dir.as_ref(),
                &mut node_capture_seq,
            )? {
                super::run_execution::RunExecutionDisposition::Continue => {}
                super::run_execution::RunExecutionDisposition::GoalCompleted => {
                    goal_completed = true;
                    break;
                }
                super::run_execution::RunExecutionDisposition::Abort(msg) => {
                    return Err(anyhow::anyhow!(msg));
                }
            }

            match self
                .execute_planned_action(
                    i,
                    goal,
                    &plan,
                    &mut session_steps,
                    &mut session,
                    &mut history,
                    &mut consecutive_failures,
                    &mut last_read_number,
                    &mut timing,
                    node_capture_enabled,
                    node_capture_all,
                    node_capture_dir.as_ref(),
                    &mut node_capture_seq,
                    &mut last_opened_app,
                )
                .await
            {
                super::run_execution::RunExecutionDisposition::Continue => {}
                super::run_execution::RunExecutionDisposition::GoalCompleted => {
                    goal_completed = true;
                    break;
                }
                super::run_execution::RunExecutionDisposition::Abort(abort_msg) => {
                    session.status = SessionStatus::Failed;
                    let _ = crate::session_store::save_session(&session);
                    return Err(anyhow::anyhow!(abort_msg));
                }
            }
        }
        if goal_completed {
            let summary = Self::summarize_execution(goal, &session, &history, true, &timing);
            session.status = if summary.business_complete {
                SessionStatus::Completed
            } else {
                SessionStatus::Failed
            };
            let _ = crate::session_store::save_session(&session);
            return Ok(summary);
        }

        session.status = SessionStatus::Failed;
        let _ = crate::session_store::save_session(&session);
        Err(anyhow::anyhow!(
            "Planner stopped without completion (max steps reached or unresolved review loop)."
        ))
    }
}
