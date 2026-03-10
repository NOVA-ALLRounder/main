use crate::controller::actions::ActionRunner;
use crate::platform::{app_matches_role, current_platform, AppRole};

impl ActionRunner {
    pub(in crate::controller::actions) fn is_focus_noise_app(app: &str) -> bool {
        let lower = app.to_lowercase();
        lower.contains("terminal")
            || lower.contains("iterm")
            || lower.contains("electron")
            || lower.contains("chatgpt")
            || lower.contains("atlas")
            || lower.contains("cursor")
            || lower.contains("code")
            || lower.contains("codex")
    }

    pub(in crate::controller::actions) fn is_text_app(app: &str) -> bool {
        let kind = current_platform().kind();
        app_matches_role(kind, AppRole::TextEditor, app)
            || app_matches_role(kind, AppRole::NotesApp, app)
            || app_matches_role(kind, AppRole::MailClient, app)
    }

    pub(in crate::controller::actions) fn looks_like_calc_expression(text: &str) -> bool {
        let compact = text.trim();
        if compact.is_empty() {
            return false;
        }
        if !compact.chars().any(|c| c.is_ascii_digit()) {
            return false;
        }

        let normalized = compact.replace(['×', 'x', 'X'], "*").replace(' ', "");
        if normalized.chars().any(|c| c.is_ascii_alphabetic()) {
            return false;
        }

        normalized.contains('*')
            || normalized.contains('+')
            || normalized.contains('-')
            || normalized.contains('/')
            || normalized.contains('=')
    }

    pub(in crate::controller::actions) fn last_text_app_from_history(
        history: &[String],
    ) -> Option<String> {
        for entry in history.iter().rev() {
            if let Some(app) = Self::opened_app_from_history_entry(entry) {
                if Self::is_text_app(app) {
                    return Some(app.to_string());
                }
            }
        }
        None
    }

    pub(in crate::controller::actions) fn preview_text(text: &str, limit: usize) -> String {
        let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if compact.chars().count() <= limit {
            compact
        } else {
            let mut out = String::new();
            for ch in compact.chars().take(limit) {
                out.push(ch);
            }
            format!("{}...", out)
        }
    }

    pub(in crate::controller::actions) fn extract_first_number(text: &str) -> Option<String> {
        let mut token = String::new();
        let mut started = false;

        for ch in text.chars() {
            let is_number_char =
                ch.is_ascii_digit() || ch == '.' || ch == ',' || ch == '-' || ch == '+';
            if !started {
                if ch.is_ascii_digit() || ch == '-' || ch == '+' {
                    started = true;
                    token.push(ch);
                }
                continue;
            }

            if is_number_char {
                token.push(ch);
            } else {
                break;
            }
        }

        let cleaned = token
            .trim_matches(|c: char| c == ',' || c == '.' || c == '+' || c == '-')
            .to_string();
        if cleaned.chars().any(|c| c.is_ascii_digit()) {
            Some(cleaned)
        } else {
            None
        }
    }

    pub(in crate::controller::actions) fn extract_quoted_fragments(goal: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut in_double = false;
        let mut in_single = false;
        let mut buf = String::new();
        for ch in goal.chars() {
            if ch == '"' && !in_single {
                if in_double {
                    let v = buf.trim().to_string();
                    if !v.is_empty() {
                        out.push(v);
                    }
                    buf.clear();
                    in_double = false;
                } else {
                    in_double = true;
                    buf.clear();
                }
                continue;
            }
            if ch == '\'' && !in_double {
                if in_single {
                    let v = buf.trim().to_string();
                    if !v.is_empty() {
                        out.push(v);
                    }
                    buf.clear();
                    in_single = false;
                } else {
                    in_single = true;
                    buf.clear();
                }
                continue;
            }

            if in_double || in_single {
                buf.push(ch);
            }
        }
        out
    }

    pub(in crate::controller::actions) fn mail_fallback_body_from_goal(goal: &str) -> String {
        let mut lines = Vec::new();
        for frag in Self::extract_quoted_fragments(goal) {
            if frag.len() >= 3 {
                lines.push(frag);
            }
        }
        lines.join("\n")
    }
}
