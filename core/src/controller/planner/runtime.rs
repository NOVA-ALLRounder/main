use super::{Planner, PlannerTimingStats};
use crate::controller::heuristics;
use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

impl PlannerTimingStats {
    pub(super) fn record_capture(&mut self, elapsed: Duration) {
        let ms = elapsed.as_millis();
        self.capture_total_ms += ms;
        self.capture_max_ms = self.capture_max_ms.max(ms);
        self.capture_count += 1;
    }

    pub(super) fn record_plan(&mut self, elapsed: Duration) {
        let ms = elapsed.as_millis();
        self.plan_total_ms += ms;
        self.plan_max_ms = self.plan_max_ms.max(ms);
        self.plan_count += 1;
    }

    pub(super) fn record_supervisor(&mut self, elapsed: Duration) {
        let ms = elapsed.as_millis();
        self.supervisor_total_ms += ms;
        self.supervisor_max_ms = self.supervisor_max_ms.max(ms);
        self.supervisor_count += 1;
    }

    pub(super) fn record_execute(&mut self, elapsed: Duration) {
        let ms = elapsed.as_millis();
        self.execute_total_ms += ms;
        self.execute_max_ms = self.execute_max_ms.max(ms);
        self.execute_count += 1;
    }
}

impl Planner {
    pub(super) fn env_truthy(name: &str) -> bool {
        matches!(
            std::env::var(name).as_deref(),
            Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
        )
    }

    pub(super) fn env_truthy_default(name: &str, default_value: bool) -> bool {
        match std::env::var(name) {
            Ok(raw) => matches!(raw.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"),
            Err(_) => default_value,
        }
    }

    pub(super) fn should_abort_on_execution_error(err: &anyhow::Error) -> bool {
        if !Self::env_truthy_default("STEER_ABORT_ON_EXECUTION_ERROR", true) {
            return false;
        }

        let msg = err.to_string().to_lowercase();
        let recoverable_type_permission_error = msg.contains("critical type action failed")
            && (msg.contains("keystroke")
                || msg.contains("키스트로크")
                || msg.contains("(1002)")
                || msg.contains("native fallback timed out"));
        if recoverable_type_permission_error {
            return false;
        }

        msg.contains("critical ")
            || msg.contains("focus_recovery_failed")
            || msg.contains("open app failed")
            || msg.contains("mail send")
    }

    pub(super) fn supervisor_safe_bypass_enabled() -> bool {
        Self::env_truthy_default("STEER_SUPERVISOR_BYPASS_SAFE", true)
    }

    pub(super) fn is_low_risk_action_for_supervisor(plan: &serde_json::Value) -> bool {
        matches!(
            plan["action"].as_str().unwrap_or(""),
            "open_app"
                | "switch_app"
                | "shortcut"
                | "key"
                | "type"
                | "paste"
                | "copy"
                | "select_all"
                | "read"
                | "read_clipboard"
                | "transfer"
        )
    }

    pub(super) fn record_fallback_action(
        history: &mut Vec<String>,
        reason: &str,
        plan: &serde_json::Value,
    ) {
        println!("   🧯 Fallback action [{}]: {}", reason, plan);
        history.push(format!("FALLBACK_ACTION: {} => {}", reason, plan));
    }

    pub(super) fn fallback_action_count(history: &[String]) -> usize {
        history
            .iter()
            .filter(|entry| entry.starts_with("FALLBACK_ACTION:"))
            .count()
    }

    pub(super) fn fallback_checkpoint_limit() -> usize {
        Self::env_usize("STEER_FALLBACK_CHECKPOINT_LIMIT", 3)
    }

    pub(super) fn enforce_fallback_checkpoint(history: &mut Vec<String>) -> Result<()> {
        if !Self::env_truthy_default("STEER_ENFORCE_FALLBACK_CHECKPOINT", true) {
            return Ok(());
        }
        let count = Self::fallback_action_count(history);
        let limit = Self::fallback_checkpoint_limit();
        if count < limit {
            return Ok(());
        }
        let msg = format!(
            "Fallback checkpoint reached (count={} limit={})",
            count, limit
        );
        println!("   ⛔ {}", msg);
        history.push(format!("APPROVAL_CHECKPOINT_REQUIRED: {}", msg));
        Err(anyhow::anyhow!(msg))
    }

    pub(super) async fn run_standard_cleanup_preset(goal: &str, history: &mut Vec<String>) {
        if !Self::env_truthy_default("STEER_STANDARD_CLEANUP_PRESET", true) {
            return;
        }

        if heuristics::try_close_front_dialog() {
            history.push("CLEANUP_DIALOG_CLOSED: front dialog".to_string());
        }

        let lower = goal.to_lowercase();
        let mut targets: Vec<&str> = Vec::new();
        if lower.contains("mail") || lower.contains("메일") || lower.contains("이메일") {
            targets.push("Mail");
        }
        if lower.contains("notes") || lower.contains("메모") {
            targets.push("Notes");
        }
        if lower.contains("textedit") {
            targets.push("TextEdit");
        }

        for app in targets {
            let _ = heuristics::ensure_app_focus(app, 2).await;
            if heuristics::try_close_front_dialog() {
                history.push(format!("CLEANUP_DIALOG_CLOSED: {}", app));
            }
            history.push(format!("CLEANUP_APP_READY: {}", app));
        }

        if lower.contains("mail") || lower.contains("메일") || lower.contains("이메일") {
            let lines = [
                "tell application \"Mail\"",
                "set _count to (count of outgoing messages)",
                "if _count = 0 then return \"0\"",
                "repeat with _msg in outgoing messages",
                "try",
                "set visible of _msg to false",
                "end try",
                "end repeat",
                "return (_count as text)",
                "end tell",
            ];
            if let Ok(out) = crate::applescript::run_with_args(&lines, &Vec::<String>::new()) {
                let count = out.trim().to_string();
                if !count.is_empty() {
                    history.push(format!("CLEANUP_MAIL_OUTGOING_HIDDEN: {}", count));
                }
            }
        }
    }

    pub(super) fn env_usize(name: &str, default_value: usize) -> usize {
        std::env::var(name)
            .ok()
            .and_then(|raw| raw.parse::<usize>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(default_value)
    }

    pub(super) fn env_u64(name: &str, default_value: u64) -> u64 {
        std::env::var(name)
            .ok()
            .and_then(|raw| raw.parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(default_value)
    }

    pub(super) fn is_soft_planner_failure(message: &str) -> bool {
        let lower = message.to_lowercase();
        lower.contains("timeout")
            || lower.contains("timed out")
            || lower.contains("rate limit")
            || lower.contains("429")
            || lower.contains("temporar")
            || lower.contains("network")
            || lower.contains("connection")
            || lower.contains("http 5")
            || lower.contains("service unavailable")
    }

    pub(super) fn has_quota_exhaustion_marker(history: &[String]) -> bool {
        history.iter().any(|line| {
            let lower = line.to_lowercase();
            lower.contains("insufficient_quota")
                || lower.contains("quota")
                || lower.contains("exhausted your capacity")
                || lower.contains("rate limit")
                || lower.contains("429")
        })
    }

    pub(super) async fn recover_plan_after_primary_failure(
        &self,
        goal: &str,
        history: &[String],
        failure_reason: &str,
    ) -> Option<serde_json::Value> {
        let recovery_enabled = Self::env_truthy_default("STEER_PLANNER_TIMEOUT_RECOVERY", true);
        if !recovery_enabled {
            return None;
        }

        let allow_hard_error_recovery = Self::env_truthy("STEER_PLANNER_RECOVER_HARD_ERRORS");
        if !allow_hard_error_recovery && !Self::is_soft_planner_failure(failure_reason) {
            return None;
        }

        let compact_history = history
            .iter()
            .rev()
            .take(16)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n- ");

        let system_prompt = "You are a resilient desktop automation planner.
Return exactly one JSON object for the next single action.
Allowed actions: click_visual, click_ref, type, shortcut, read, scroll, open_app, open_url, select_all, copy, paste, read_clipboard, n8n_create_workflow, n8n_execute_workflow, done, wait.
Rules:
- Never output markdown or explanation text.
- If using open_app, include a non-empty name.
- Prefer the safest action that still moves toward the goal.";
        let user_prompt = format!(
            "GOAL: {}\nFAILURE_REASON: {}\nRECENT_HISTORY:\n- {}",
            goal,
            failure_reason,
            if compact_history.is_empty() {
                "None"
            } else {
                &compact_history
            }
        );

        let recovery_timeout =
            Duration::from_secs(Self::env_u64("STEER_PLANNER_RECOVERY_TIMEOUT_SEC", 14));
        let messages = vec![
            serde_json::json!({"role": "system", "content": system_prompt}),
            serde_json::json!({"role": "user", "content": user_prompt}),
        ];

        match tokio::time::timeout(recovery_timeout, self.llm.chat_completion(messages)).await {
            Ok(Ok(raw)) => {
                if let Some(parsed) = crate::llm_gateway::recover_json(&raw) {
                    println!("   🧯 Planner recovery: text-only LLM plan accepted.");
                    return Some(parsed);
                }
                println!("   ⚠️ Planner recovery: text-only output was not valid JSON.");
            }
            Ok(Err(e)) => {
                println!("   ⚠️ Planner recovery: text-only LLM failed: {}", e);
            }
            Err(_) => {
                println!(
                    "   ⚠️ Planner recovery: text-only LLM timeout after {}s.",
                    recovery_timeout.as_secs()
                );
            }
        }

        if let Some(fallback_plan) = Self::fallback_plan_from_goal(goal, history) {
            println!("   🧯 Planner recovery: deterministic fallback plan selected.");
            return Some(fallback_plan);
        }

        if let Some(app) = Self::extract_known_app_from_text(goal) {
            println!(
                "   🧯 Planner recovery: inferred open_app fallback selected ({}).",
                app
            );
            return Some(serde_json::json!({"action":"open_app","name":app}));
        }

        println!("   🧯 Planner recovery: default wait action selected.");
        Some(serde_json::json!({"action":"wait","seconds":1}))
    }

    pub fn new(
        llm: Arc<dyn crate::llm_gateway::LLMClient>,
        tx: Option<mpsc::Sender<String>>,
    ) -> Self {
        Self {
            llm,
            max_steps: Self::env_usize("STEER_MAX_STEPS", 30),
            tx,
        }
    }

    pub async fn run_goal_tracked(
        &self,
        goal: &str,
        session_key: Option<&str>,
    ) -> Result<super::RunGoalOutcome> {
        let run_id = format!(
            "surf_{}_{}",
            Utc::now().format("%Y%m%d_%H%M%S"),
            Uuid::new_v4().simple()
        );
        self.run_goal_tracked_with_run_id(&run_id, goal, session_key)
            .await
    }
}
