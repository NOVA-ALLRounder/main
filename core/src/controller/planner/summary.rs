use super::*;

impl Planner {
    pub(super) fn summarize_execution(
        goal: &str,
        session: &Session,
        history: &[String],
        planner_complete: bool,
        timing: &PlannerTimingStats,
    ) -> RunGoalExecutionSummary {
        let cleanup_dialog_closed_count = history
            .iter()
            .filter(|h| h.starts_with("CLEANUP_DIALOG_CLOSED:"))
            .count();
        let cleanup_app_ready_count = history
            .iter()
            .filter(|h| h.starts_with("CLEANUP_APP_READY:"))
            .count();
        let cleanup_mail_outgoing_hidden_count = history
            .iter()
            .filter_map(|h| h.strip_prefix("CLEANUP_MAIL_OUTGOING_HIDDEN:"))
            .filter_map(|raw| raw.trim().parse::<usize>().ok())
            .sum::<usize>();
        let preflight_permissions_ok = history.iter().any(|h| h == "PREFLIGHT_PERMISSIONS_OK");
        let preflight_screen_capture_ok =
            history.iter().any(|h| h == "PREFLIGHT_SCREEN_CAPTURE_OK");

        let step_count = session.steps.len();
        let failed_steps = session
            .steps
            .iter()
            .filter(|s| s.status != "success")
            .count();
        let blocking_failed_steps = session
            .steps
            .iter()
            .filter(|s| s.status != "success" && !Self::is_benign_failed_step(s, goal, history))
            .count();
        let blocking_failure_details: Vec<String> = session
            .steps
            .iter()
            .filter(|s| s.status != "success" && !Self::is_benign_failed_step(s, goal, history))
            .take(3)
            .map(|s| format!("{}: {}", s.action_type, s.description))
            .collect();
        let execution_complete = planner_complete && blocking_failed_steps == 0;
        let mail_send_required = Self::goal_requires_mail_send(goal);
        let notes_write_required = Self::goal_requires_notes_write(goal);
        let textedit_write_required = Self::goal_requires_textedit_write(goal);
        let textedit_save_required = Self::goal_requires_textedit_save(goal);
        let evidence = Self::collect_business_evidence(session, history);
        let mail_send_confirmed = evidence.mail_send_confirmed;
        let notes_write_confirmed = evidence.notes_write_confirmed;
        let textedit_write_confirmed = evidence.textedit_write_confirmed;
        let textedit_save_confirmed = evidence.textedit_save_confirmed;

        let (business_complete, business_note) = if !execution_complete {
            (
                false,
                format!(
                    "action execution had blocking failures (blocking_failed_steps={} / failed_steps={} / total_steps={})",
                    blocking_failed_steps, failed_steps, step_count
                ),
            )
        } else {
            let mut missing_checks: Vec<&str> = Vec::new();
            if mail_send_required && !mail_send_confirmed {
                missing_checks.push("mail_send_confirmation");
            }
            if notes_write_required && !notes_write_confirmed {
                missing_checks.push("notes_write_evidence");
            }
            if textedit_write_required && !textedit_write_confirmed {
                missing_checks.push("textedit_write_evidence");
            }
            if textedit_save_required && !textedit_save_confirmed {
                missing_checks.push("textedit_save_confirmation");
            }

            if missing_checks.is_empty() {
                (
                    true,
                    "planner/execution/business checks passed from run evidence".to_string(),
                )
            } else {
                (
                    false,
                    format!("business evidence missing: {}", missing_checks.join(", ")),
                )
            }
        };

        RunGoalExecutionSummary {
            planner_complete,
            execution_complete,
            business_complete,
            business_note,
            approval_required: false,
            preflight_permissions_ok,
            preflight_screen_capture_ok,
            cleanup_dialog_closed_count,
            cleanup_app_ready_count,
            cleanup_mail_outgoing_hidden_count,
            step_count,
            failed_steps,
            blocking_failed_steps,
            blocking_failure_details,
            mail_send_required,
            mail_send_confirmed,
            notes_write_required,
            notes_write_confirmed,
            textedit_write_required,
            textedit_write_confirmed,
            textedit_save_required,
            textedit_save_confirmed,
            capture_total_ms: timing.capture_total_ms,
            capture_max_ms: timing.capture_max_ms,
            capture_count: timing.capture_count,
            plan_total_ms: timing.plan_total_ms,
            plan_max_ms: timing.plan_max_ms,
            plan_count: timing.plan_count,
            supervisor_total_ms: timing.supervisor_total_ms,
            supervisor_max_ms: timing.supervisor_max_ms,
            supervisor_count: timing.supervisor_count,
            execute_total_ms: timing.execute_total_ms,
            execute_max_ms: timing.execute_max_ms,
            execute_count: timing.execute_count,
        }
    }
}
