use super::*;

impl Planner {
    pub async fn run_goal_tracked_with_run_id(
        &self,
        run_id: &str,
        goal: &str,
        session_key: Option<&str>,
    ) -> Result<RunGoalOutcome> {
        let _serial_guard = if Self::env_truthy_default("STEER_SERIALIZE_GUI_RUNS", true) {
            Some(GUI_RUN_SERIAL_LOCK.lock().await)
        } else {
            None
        };
        let run_id = run_id.trim().to_string();
        if run_id.is_empty() {
            return Err(anyhow::anyhow!("run_id_empty"));
        }
        if let Ok(cleaned) = db::mark_stale_running_task_runs_finished() {
            if cleaned > 0 {
                println!(
                    "🧹 Auto-cleaned stale running task runs before new execution: {}",
                    cleaned
                );
            }
        }
        let normalized_goal = goal.trim();
        let _ = db::create_task_run(&run_id, "surf_goal", normalized_goal, "running");
        let _ = db::record_task_stage_run(
            &run_id,
            "planner",
            1,
            "running",
            Some("planner.run_goal started"),
        );

        match self.run_goal_with_summary(goal, session_key).await {
            Ok(exec_summary) => {
                let planner_complete = exec_summary.planner_complete;
                let execution_complete = exec_summary.execution_complete;
                let business_complete = exec_summary.business_complete;
                let approval_required = exec_summary.approval_required;
                let status = if approval_required {
                    "approval_required"
                } else if business_complete {
                    "business_completed"
                } else {
                    "business_failed"
                };
                let task_run_status = if approval_required {
                    "business_incomplete"
                } else {
                    status
                };
                let summary = if approval_required {
                    Some(format!(
                        "surf goal paused: approval checkpoint required ({})",
                        exec_summary.business_note
                    ))
                } else if business_complete {
                    Some("surf goal execution and business checks completed".to_string())
                } else {
                    Some(format!(
                        "surf goal completed planner/execution but business check failed: {}",
                        exec_summary.business_note
                    ))
                };
                let details = serde_json::json!({
                    "source": "planner.run_goal_tracked",
                    "goal": normalized_goal,
                    "status": status,
                    "business_complete": business_complete,
                    "approval_required": approval_required,
                    "business_note": exec_summary.business_note,
                    "preflight_permissions_ok": exec_summary.preflight_permissions_ok,
                    "preflight_screen_capture_ok": exec_summary.preflight_screen_capture_ok,
                    "cleanup_dialog_closed_count": exec_summary.cleanup_dialog_closed_count,
                    "cleanup_app_ready_count": exec_summary.cleanup_app_ready_count,
                    "cleanup_mail_outgoing_hidden_count": exec_summary.cleanup_mail_outgoing_hidden_count,
                    "step_count": exec_summary.step_count,
                    "failed_steps": exec_summary.failed_steps,
                    "blocking_failed_steps": exec_summary.blocking_failed_steps,
                    "blocking_failure_details": exec_summary.blocking_failure_details.clone(),
                    "mail_send_required": exec_summary.mail_send_required,
                    "mail_send_confirmed": exec_summary.mail_send_confirmed,
                    "notes_write_required": exec_summary.notes_write_required,
                    "notes_write_confirmed": exec_summary.notes_write_confirmed,
                    "textedit_write_required": exec_summary.textedit_write_required,
                    "textedit_write_confirmed": exec_summary.textedit_write_confirmed,
                    "textedit_save_required": exec_summary.textedit_save_required,
                    "textedit_save_confirmed": exec_summary.textedit_save_confirmed,
                    "timing": {
                        "capture_total_ms": exec_summary.capture_total_ms,
                        "capture_max_ms": exec_summary.capture_max_ms,
                        "capture_count": exec_summary.capture_count,
                        "plan_total_ms": exec_summary.plan_total_ms,
                        "plan_max_ms": exec_summary.plan_max_ms,
                        "plan_count": exec_summary.plan_count,
                        "supervisor_total_ms": exec_summary.supervisor_total_ms,
                        "supervisor_max_ms": exec_summary.supervisor_max_ms,
                        "supervisor_count": exec_summary.supervisor_count,
                        "execute_total_ms": exec_summary.execute_total_ms,
                        "execute_max_ms": exec_summary.execute_max_ms,
                        "execute_count": exec_summary.execute_count
                    }
                })
                .to_string();

                let _ = db::record_task_stage_run(
                    &run_id,
                    "planner",
                    1,
                    if planner_complete {
                        "completed"
                    } else if approval_required {
                        "blocked"
                    } else {
                        "failed"
                    },
                    Some(if planner_complete {
                        "planner produced done"
                    } else if approval_required {
                        "planner paused by approval checkpoint"
                    } else {
                        "planner did not reach done"
                    }),
                );
                let _ = db::record_task_stage_assertion(
                    &run_id,
                    "planner",
                    "planner_complete",
                    "true",
                    if planner_complete { "true" } else { "false" },
                    planner_complete,
                    Some(if planner_complete {
                        "Goal completed by planner"
                    } else {
                        "Planner did not complete goal"
                    }),
                );
                let _ = db::record_task_stage_assertion(
                    &run_id,
                    "planner",
                    "planner.approval_checkpoint_required",
                    "false",
                    if approval_required { "true" } else { "false" },
                    !approval_required,
                    Some("Fallback checkpoint should not be hit in autonomous run"),
                );
                let _ = db::record_task_stage_assertion(
                    &run_id,
                    "planner",
                    "planner.preflight_permissions_ok",
                    "true",
                    if exec_summary.preflight_permissions_ok {
                        "true"
                    } else {
                        "false"
                    },
                    exec_summary.preflight_permissions_ok,
                    Some("Accessibility/automation permission preflight"),
                );
                let _ = db::record_task_stage_assertion(
                    &run_id,
                    "planner",
                    "planner.preflight_screen_capture_ok",
                    "true",
                    if exec_summary.preflight_screen_capture_ok {
                        "true"
                    } else {
                        "false"
                    },
                    exec_summary.preflight_screen_capture_ok,
                    Some("Screen capture permission preflight"),
                );
                let _ = db::record_task_stage_run(
                    &run_id,
                    "execution",
                    2,
                    if approval_required {
                        "blocked"
                    } else if execution_complete {
                        "completed"
                    } else {
                        "failed"
                    },
                    Some(&format!(
                        "step_count={} failed_steps={} blocking_failed_steps={}",
                        exec_summary.step_count,
                        exec_summary.failed_steps,
                        exec_summary.blocking_failed_steps
                    )),
                );
                let _ = db::record_task_stage_assertion(
                    &run_id,
                    "execution",
                    "execution_complete",
                    "true",
                    if execution_complete { "true" } else { "false" },
                    execution_complete,
                    Some(&{
                        let mut evidence = String::from(
                            "All blocking action steps must be successful (mail send pending/no_draft retries + shortcut permission denials are non-blocking)"
                        );
                        if !exec_summary.blocking_failure_details.is_empty() {
                            evidence.push_str(" | failures=");
                            evidence.push_str(&exec_summary.blocking_failure_details.join(" || "));
                        }
                        evidence
                    }),
                );
                let _ = db::record_task_stage_assertion(
                    &run_id,
                    "execution",
                    "execution.cleanup_dialog_closed_count",
                    ">=0",
                    &exec_summary.cleanup_dialog_closed_count.to_string(),
                    true,
                    Some("Count of cleanup dialog closures before/during run"),
                );
                let _ = db::record_task_stage_assertion(
                    &run_id,
                    "execution",
                    "execution.cleanup_app_ready_count",
                    ">=0",
                    &exec_summary.cleanup_app_ready_count.to_string(),
                    true,
                    Some("Count of app readiness cleanup markers"),
                );
                let _ = db::record_task_stage_assertion(
                    &run_id,
                    "execution",
                    "execution.cleanup_mail_outgoing_hidden_count",
                    ">=0",
                    &exec_summary.cleanup_mail_outgoing_hidden_count.to_string(),
                    true,
                    Some("Count of hidden outgoing draft windows during cleanup"),
                );
                let _ = db::record_task_stage_run(
                    &run_id,
                    "business",
                    3,
                    if approval_required {
                        "blocked"
                    } else if business_complete {
                        "completed"
                    } else {
                        "failed"
                    },
                    Some(&exec_summary.business_note),
                );
                let _ = db::record_task_stage_assertion(
                    &run_id,
                    "business",
                    "business_complete",
                    "true",
                    if business_complete { "true" } else { "false" },
                    business_complete,
                    Some(&format!(
                        "mail_send_required={} mail_send_confirmed={} notes_write_required={} notes_write_confirmed={} textedit_write_required={} textedit_write_confirmed={} textedit_save_required={} textedit_save_confirmed={}",
                        exec_summary.mail_send_required,
                        exec_summary.mail_send_confirmed,
                        exec_summary.notes_write_required,
                        exec_summary.notes_write_confirmed,
                        exec_summary.textedit_write_required,
                        exec_summary.textedit_write_confirmed,
                        exec_summary.textedit_save_required,
                        exec_summary.textedit_save_confirmed
                    )),
                );
                let _ = db::update_task_run_outcome(
                    &run_id,
                    planner_complete,
                    execution_complete,
                    business_complete,
                    task_run_status,
                    summary.as_deref(),
                    Some(&details),
                );

                Ok(RunGoalOutcome {
                    run_id,
                    planner_complete,
                    execution_complete,
                    business_complete,
                    status: status.to_string(),
                    summary,
                })
            }
            Err(e) => {
                let error_text = e.to_string();
                let summary = Some(format!("surf goal failed: {}", error_text));
                let details = serde_json::json!({
                    "source": "planner.run_goal_tracked",
                    "goal": normalized_goal,
                    "status": "failed",
                    "error": error_text
                })
                .to_string();

                let _ = db::record_task_stage_run(&run_id, "planner", 1, "failed", Some(&details));
                let _ = db::record_task_stage_assertion(
                    &run_id,
                    "planner",
                    "planner_complete",
                    "true",
                    "false",
                    false,
                    Some("planner did not reach done"),
                );
                let _ = db::record_task_stage_run(
                    &run_id,
                    "execution",
                    2,
                    "failed",
                    Some("execution/biz completion unavailable due planner failure"),
                );
                let _ = db::record_task_stage_assertion(
                    &run_id,
                    "execution",
                    "execution_complete",
                    "true",
                    "false",
                    false,
                    Some("run_goal returned error"),
                );
                let _ = db::record_task_stage_run(
                    &run_id,
                    "business",
                    3,
                    "failed",
                    Some("business completion failed"),
                );
                let _ = db::record_task_stage_assertion(
                    &run_id,
                    "business",
                    "business_complete",
                    "true",
                    "false",
                    false,
                    Some("run_goal returned error"),
                );
                let _ = db::update_task_run_outcome(
                    &run_id,
                    false,
                    false,
                    false,
                    "business_failed",
                    summary.as_deref(),
                    Some(&details),
                );

                Err(anyhow::anyhow!("{} (run_id={})", e, run_id))
            }
        }
    }
}
