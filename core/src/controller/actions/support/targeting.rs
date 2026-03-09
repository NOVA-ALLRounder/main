use anyhow::Result;

use crate::controller::heuristics;

use crate::controller::actions::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) fn goal_mentions_downloads(goal: &str) -> bool {
        let lower = goal.to_lowercase();
        lower.contains("downloads")
            || lower.contains("downloads folder")
            || lower.contains("다운로드")
    }

    pub(in crate::controller::actions) fn finder_open_downloads() -> Result<()> {
        let lines = [
            "tell application \"Finder\"",
            "activate",
            "set targetFolder to (path to downloads folder)",
            "if (count of Finder windows) = 0 then",
            "set newWin to make new Finder window",
            "set target of newWin to targetFolder",
            "else",
            "set target of front Finder window to targetFolder",
            "end if",
            "end tell",
            "return \"ok\"",
        ];
        crate::applescript::run_with_args(&lines, &Vec::<String>::new())?;
        Ok(())
    }

    pub(in crate::controller::actions) fn normalize_shortcut_parts(
        key_raw: &str,
        modifiers_raw: &[String],
    ) -> (String, Vec<String>) {
        let mut key = key_raw.trim().to_lowercase();
        let mut modifiers: Vec<String> = Vec::new();

        let mut push_modifier = |token: &str| {
            let normalized = match token.trim().to_lowercase().as_str() {
                "cmd" | "command" => Some("command".to_string()),
                "shift" => Some("shift".to_string()),
                "option" | "alt" => Some("option".to_string()),
                "control" | "ctrl" => Some("control".to_string()),
                _ => None,
            };
            if let Some(m) = normalized {
                if !modifiers.contains(&m) {
                    modifiers.push(m);
                }
            }
        };

        if key.contains('+') {
            let mut combo_key: Option<String> = None;
            for part in key.split('+').filter(|p| !p.trim().is_empty()) {
                if matches!(
                    part.trim().to_lowercase().as_str(),
                    "cmd" | "command" | "shift" | "option" | "alt" | "control" | "ctrl"
                ) {
                    push_modifier(part);
                } else {
                    combo_key = Some(part.trim().to_lowercase());
                }
            }
            if let Some(k) = combo_key {
                key = k;
            }
        }

        for modifier in modifiers_raw {
            push_modifier(modifier);
        }

        match key.as_str() {
            "paste" | "붙여넣기" => {
                key = "v".to_string();
                push_modifier("command");
            }
            "copy" | "복사" => {
                key = "c".to_string();
                push_modifier("command");
            }
            "select_all" | "selectall" | "전체선택" => {
                key = "a".to_string();
                push_modifier("command");
            }
            "new" | "새로만들기" => {
                key = "n".to_string();
                push_modifier("command");
            }
            _ => {}
        }

        (key, modifiers)
    }

    pub(in crate::controller::actions) fn sanitize_evidence_value(value: &str) -> String {
        value
            .replace(['\n', '\r'], " ")
            .replace('|', "/")
            .trim()
            .to_string()
    }

    pub(in crate::controller::actions) fn log_evidence(
        target: &str,
        event: &str,
        fields: &[(&str, String)],
    ) {
        let mut line = format!(
            "EVIDENCE|target={}|event={}",
            Self::sanitize_evidence_value(target),
            Self::sanitize_evidence_value(event)
        );
        for (key, value) in fields {
            line.push('|');
            line.push_str(key);
            line.push('=');
            line.push_str(&Self::sanitize_evidence_value(value));
        }
        println!("{}", line);
    }

    pub(in crate::controller::actions) fn preferred_target_app_from_history(
        action_type: &str,
        plan: &serde_json::Value,
        history: &[String],
    ) -> Option<String> {
        let mut target = Self::last_opened_app_from_history(history)?;
        if target.eq_ignore_ascii_case("Calculator") {
            if action_type == "type" {
                let text = plan["text"].as_str().unwrap_or("");
                if !Self::looks_like_calc_expression(text) {
                    if let Some(text_app) = Self::last_text_app_from_history(history) {
                        target = text_app;
                    }
                }
            } else if action_type == "paste" {
                if let Some(text_app) = Self::last_text_app_from_history(history) {
                    target = text_app;
                }
            } else if action_type == "shortcut" || action_type == "key" {
                let key = plan["key"].as_str().unwrap_or("").to_lowercase();
                if key == "v" || key == "n" {
                    if let Some(text_app) = Self::last_text_app_from_history(history) {
                        target = text_app;
                    }
                }
            }
        }
        Some(target)
    }

    pub(in crate::controller::actions) fn resolve_shortcut_target_app(
        action_type: &str,
        plan: &serde_json::Value,
        history: &[String],
        goal: &str,
        front_app: &str,
    ) -> String {
        if let Some(app) = plan
            .get("app")
            .and_then(|v| v.as_str())
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
        {
            return app.to_string();
        }

        if let Some(app) = Self::preferred_target_app_from_history(action_type, plan, history) {
            let trimmed = app.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

        let front_trimmed = front_app.trim();
        if !front_trimmed.is_empty() && !Self::is_focus_noise_app(front_trimmed) {
            return front_trimmed.to_string();
        }

        if let Some(app) = heuristics::goal_primary_app(goal) {
            return app.to_string();
        }

        if let Some(app) = Self::last_opened_app_from_history(history) {
            let trimmed = app.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

        if !front_trimmed.is_empty() {
            return front_trimmed.to_string();
        }

        "unknown".to_string()
    }
}
