use super::*;

pub(super) enum SupervisorDisposition {
    Accept(serde_json::Value),
    Review(String),
    Escalate(String),
}

impl Planner {
    pub(super) async fn review_plan_with_supervisor(
        &self,
        step_index: usize,
        goal: &str,
        mut plan: serde_json::Value,
        history: &mut Vec<String>,
        timing: &mut PlannerTimingStats,
        allow_deterministic_fallback: bool,
        allow_review_loop_override: bool,
    ) -> Result<SupervisorDisposition> {
        let retry_config = util::planner_retry_config();
        let supervisor_started = Instant::now();
        let bypass_supervisor = Self::supervisor_safe_bypass_enabled()
            && Self::is_low_risk_action_for_supervisor(&plan);
        let (mut supervisor_action, supervisor_reason, supervisor_notes) = if bypass_supervisor {
            println!("   🕵️ Supervisor: bypass (safe action)");
            (
                "accept".to_string(),
                "safe_action_bypass".to_string(),
                "Low-risk action bypassed supervisor gate".to_string(),
            )
        } else {
            let supervisor_timeout =
                Duration::from_secs(Self::env_u64("STEER_PLANNER_SUPERVISOR_TIMEOUT_SEC", 6));
            let supervisor_result =
                crate::retry_logic::with_retry(&retry_config, "Supervisor", || async {
                    tokio::time::timeout(
                        supervisor_timeout,
                        Supervisor::consult(&*self.llm, goal, &plan, history),
                    )
                    .await
                    .map_err(|_| {
                        anyhow::anyhow!(
                            "planner supervisor timeout after {}s",
                            supervisor_timeout.as_secs()
                        )
                    })?
                })
                .await;

            match supervisor_result {
                Ok(supervisor_decision) => {
                    println!(
                        "   🕵️ Supervisor: {} ({})",
                        supervisor_decision.action, supervisor_decision.reason
                    );
                    (
                        supervisor_decision.action,
                        supervisor_decision.reason,
                        supervisor_decision.notes,
                    )
                }
                Err(e) => {
                    let err_text = e.to_string();
                    let fail_open = Self::env_truthy_default("STEER_SUPERVISOR_FAIL_OPEN", true);
                    if fail_open && Self::is_soft_planner_failure(&err_text) {
                        println!("   🧯 Supervisor fail-open: {}", err_text);
                        history.push(format!("SUPERVISOR_FAIL_OPEN: {}", err_text));
                        (
                            "accept".to_string(),
                            "supervisor_fail_open".to_string(),
                            err_text,
                        )
                    } else {
                        return Err(e);
                    }
                }
            }
        };
        let supervisor_elapsed = supervisor_started.elapsed();
        timing.record_supervisor(supervisor_elapsed);
        history.push(format!(
            "TIMING|step={}|phase=supervisor|ms={}",
            step_index,
            supervisor_elapsed.as_millis()
        ));

        if supervisor_action == "review"
            && Self::can_force_done_for_simple_goal(goal, &plan, history)
        {
            println!("   ✅ Force-complete override: simple note creation goal already satisfied.");
            supervisor_action = "accept".to_string();
        }

        if supervisor_action == "review"
            && Self::should_accept_text_flow_after_type(
                &plan,
                history,
                &supervisor_reason,
                &supervisor_notes,
            )
        {
            println!(
                "   ✅ Review override: proceeding with text-flow action after prior typing evidence."
            );
            supervisor_action = "accept".to_string();
        }

        if supervisor_action == "review"
            && Self::should_accept_typing_after_new_item_shortcut(
                &plan,
                history,
                &supervisor_reason,
                &supervisor_notes,
            )
        {
            println!("   ✅ Review override: allowing typing step after Cmd+N creation evidence.");
            supervisor_action = "accept".to_string();
        }

        if supervisor_action == "review"
            && !Self::goal_has_multi_app(goal)
            && !Self::goal_has_explicit_sequence(goal)
            && allow_deterministic_fallback
            && Self::should_relax_review(&supervisor_reason, &supervisor_notes)
        {
            if let Some(fallback_plan) = Self::fallback_plan_from_goal(goal, history) {
                Self::record_fallback_action(history, "relaxed_review", &fallback_plan);
                plan = fallback_plan;
                supervisor_action = "accept".to_string();
            }
        }

        if supervisor_action == "review" {
            let recent_rejections = history
                .iter()
                .rev()
                .take(16)
                .filter(|h| h.starts_with("PLAN_REJECTED:"))
                .count();
            if recent_rejections >= 4 {
                let review_text = format!(
                    "{} {}",
                    supervisor_reason.to_lowercase(),
                    supervisor_notes.to_lowercase()
                );
                let hard_blockers = [
                    "danger",
                    "unsafe",
                    "impossible",
                    "not related",
                    "does not relate",
                    "wrong app",
                ];
                let has_hard_blocker = hard_blockers.iter().any(|s| review_text.contains(s));
                let has_notes_content_issue = review_text.contains("notes")
                    && (review_text.contains("content")
                        || review_text.contains("exact")
                        || review_text.contains("불일치"));

                if !has_hard_blocker && !has_notes_content_issue {
                    if allow_deterministic_fallback {
                        if allow_review_loop_override {
                            if let Some(loop_break_plan) =
                                Self::fallback_plan_from_goal(goal, history)
                            {
                                Self::record_fallback_action(
                                    history,
                                    &format!("review_loop_{}rejections", recent_rejections),
                                    &loop_break_plan,
                                );
                                plan = loop_break_plan;
                                supervisor_action = "accept".to_string();
                            } else {
                                let action_name = plan["action"].as_str().unwrap_or("unknown");
                                if matches!(
                                    action_name,
                                    "open_app"
                                        | "open_url"
                                        | "shortcut"
                                        | "type"
                                        | "paste"
                                        | "copy"
                                        | "select_all"
                                        | "read"
                                        | "read_clipboard"
                                        | "click_visual"
                                ) {
                                    println!(
                                        "   🔁 Review-loop override: forcing '{}' after {} rejections.",
                                        action_name, recent_rejections
                                    );
                                    supervisor_action = "accept".to_string();
                                }
                            }
                        } else {
                            history.push(
                                "FALLBACK_BLOCKED: review-loop override requires STEER_ALLOW_REVIEW_LOOP_OVERRIDE=1"
                                    .to_string(),
                            );
                            let msg =
                                "Supervisor review loop: deterministic override disabled by policy";
                            return Ok(SupervisorDisposition::Escalate(msg.to_string()));
                        }
                    } else {
                        history.push(
                            "FALLBACK_BLOCKED: review-loop deterministic fallback disabled"
                                .to_string(),
                        );
                        let msg =
                            "Supervisor review loop: deterministic fallback disabled by policy";
                        return Ok(SupervisorDisposition::Escalate(msg.to_string()));
                    }
                }
            }
        }

        if supervisor_action == "escalate" {
            let recent_rejections = history
                .iter()
                .rev()
                .take(16)
                .filter(|h| h.starts_with("PLAN_REJECTED:"))
                .count();
            let reason_lc = supervisor_reason.to_lowercase();
            let notes_lc = supervisor_notes.to_lowercase();
            let repeated_content_escalation = (reason_lc.contains("repeated")
                || notes_lc.contains("repeated"))
                && (reason_lc.contains("content")
                    || notes_lc.contains("content")
                    || reason_lc.contains("notes")
                    || notes_lc.contains("notes"));
            if repeated_content_escalation && recent_rejections >= 3 {
                println!(
                    "   🔁 Escalation override: retrying content-repair path before hard fail."
                );
                supervisor_action = "review".to_string();
            }
        }

        Ok(match supervisor_action.as_str() {
            "accept" => SupervisorDisposition::Accept(plan),
            "review" => SupervisorDisposition::Review(supervisor_notes),
            "escalate" => SupervisorDisposition::Escalate(format!(
                "Supervisor escalated: {}",
                supervisor_reason
            )),
            _ => SupervisorDisposition::Accept(plan),
        })
    }
}
