use super::Planner;
use crate::platform::AppRole;

impl Planner {
    pub(super) fn ordered_apps_in_goal(goal: &str) -> Vec<&'static str> {
        let goal_lower = Self::normalize_text_for_matching(goal);
        let mut found: Vec<(usize, &'static str)> = Vec::new();
        for role in [
            AppRole::FileManager,
            AppRole::TextEditor,
            AppRole::NotesApp,
            AppRole::MailClient,
            AppRole::Browser,
            AppRole::Calendar,
            AppRole::Calculator,
        ] {
            let app_name = Self::app_name_for_role(role);
            for alias in
                crate::platform::app_role_aliases(crate::platform::current_platform().kind(), role)
            {
                if let Some(idx) = goal_lower.find(&alias.to_lowercase()) {
                    found.push((idx, app_name));
                }
            }
        }
        found.sort_by_key(|(idx, _)| *idx);
        let mut ordered: Vec<&'static str> = Vec::new();
        for (_, app) in found {
            if !ordered.iter().any(|seen| seen.eq_ignore_ascii_case(app)) {
                ordered.push(app);
            }
        }
        ordered
    }

    pub(super) fn extract_quoted_fragments(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut in_double = false;
        let mut in_single = false;
        let mut buf = String::new();
        for ch in text.chars() {
            if ch == '"' && !in_single {
                if in_double {
                    let value = buf.trim().to_string();
                    if !value.is_empty() {
                        out.push(value);
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
                    let value = buf.trim().to_string();
                    if !value.is_empty() {
                        out.push(value);
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

    pub(super) fn normalize_goal_text_fragment(raw: &str) -> Option<String> {
        let mut text = raw.trim().to_string();
        if text.is_empty() {
            return None;
        }

        if let Ok(prefix_re) = regex::Regex::new(
            r"(?i)^.*?(?:메모장|메모|notes|note|textedit|텍스트에디트)\s*(?:을|를)?\s*(?:열어줘|열어서|열고|열어|open|launch)\s*",
        ) {
            text = prefix_re.replace(&text, "").to_string();
        }

        text = text
            .trim()
            .trim_matches(|ch| {
                matches!(
                    ch,
                    '"' | '\'' | '“' | '”' | '‘' | '’' | '「' | '」' | '『' | '』'
                )
            })
            .trim()
            .to_string();

        if let Some(stripped) = text.strip_suffix("이라고") {
            text = stripped.trim().to_string();
        } else if let Some(stripped) = text.strip_suffix("라고") {
            text = stripped.trim().to_string();
        }

        if text.is_empty() {
            return None;
        }

        let lower = text.to_lowercase();
        if lower.starts_with("cmd+")
            || lower.starts_with("status:")
            || lower == "done"
            || lower.starts_with("run_scope_")
        {
            return None;
        }

        Some(text)
    }

    pub(super) fn extract_goal_text_fragments(goal: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for fragment in Self::extract_quoted_fragments(goal) {
            if let Some(normalized) = Self::normalize_goal_text_fragment(&fragment) {
                if !out.contains(&normalized) {
                    out.push(normalized);
                }
            }
        }

        if let Ok(korean_write_re) = regex::Regex::new(
            r#"(?P<payload>[^"'“”‘’\n]{1,160}?)(?:이라고|라고)\s*(?:써줘|써 줘|적어줘|적어 줘|입력해줘|입력해 줘|작성해줘|작성해 줘|써|적어|입력|작성)"#,
        ) {
            for captures in korean_write_re.captures_iter(goal) {
                if let Some(raw_payload) = captures.name("payload") {
                    if let Some(normalized) =
                        Self::normalize_goal_text_fragment(raw_payload.as_str())
                    {
                        if !out.contains(&normalized) {
                            out.push(normalized);
                        }
                    }
                }
            }
        }

        out
    }

    pub(super) fn is_email_like(text: &str) -> bool {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return false;
        }
        regex::Regex::new(r"^[A-Za-z0-9._%+\\-]+@[A-Za-z0-9.\\-]+\\.[A-Za-z]{2,}$")
            .ok()
            .map(|re| re.is_match(trimmed))
            .unwrap_or(false)
    }

    pub(super) fn goal_run_scope_marker(goal: &str) -> Option<String> {
        let run_scope_re = regex::Regex::new(r"(?i)(RUN_SCOPE_[A-Z0-9_]+)").ok();

        for fragment in Self::extract_goal_text_fragments(goal) {
            let f = fragment.trim();
            if let Some(re) = &run_scope_re {
                if let Some(caps) = re.captures(f) {
                    if let Some(m) = caps.get(1) {
                        return Some(m.as_str().to_string());
                    }
                }
            }
            if f.to_uppercase().starts_with("RUN_SCOPE_") {
                return Some(
                    f.chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect(),
                );
            }
        }

        for token in goal.split_whitespace() {
            let cleaned = token
                .trim_matches(|c: char| {
                    c == '"' || c == '\'' || c == '“' || c == '”' || c == '‘' || c == '’'
                })
                .trim();
            if let Some(re) = &run_scope_re {
                if let Some(caps) = re.captures(cleaned) {
                    if let Some(m) = caps.get(1) {
                        return Some(m.as_str().to_string());
                    }
                }
            }
            if cleaned.to_uppercase().starts_with("RUN_SCOPE_") {
                return Some(
                    cleaned
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect(),
                );
            }
        }

        None
    }

    pub(super) fn extract_mail_subject_from_goal(goal: &str) -> Option<String> {
        let lower = goal.to_lowercase();
        let mut scopes: Vec<&str> = Vec::new();
        let mail_idx = lower
            .rfind("mail")
            .or_else(|| lower.rfind("메일"))
            .or_else(|| lower.rfind("이메일"));
        if let Some(idx) = mail_idx {
            scopes.push(&goal[idx..]);
        }
        scopes.push(goal);

        let email_re =
            regex::Regex::new(r"[A-Za-z0-9._%+\\-]+@[A-Za-z0-9.\\-]+\\.[A-Za-z]{2,}$").ok();

        for scope in scopes {
            let scope_lower = scope.to_lowercase();
            for marker in ["제목", "subject", "title"] {
                if let Some(idx) = scope_lower.find(marker) {
                    let rest = &scope[idx + marker.len()..];
                    for frag in Self::extract_quoted_fragments(rest) {
                        let f = frag.trim();
                        let lf = f.to_lowercase();
                        if f.len() < 2 {
                            continue;
                        }
                        if lf.starts_with("cmd+") || lf.starts_with("status:") {
                            continue;
                        }
                        if f.starts_with("RUN_SCOPE_") {
                            continue;
                        }
                        if email_re.as_ref().map(|re| re.is_match(f)).unwrap_or(false) {
                            continue;
                        }
                        return Some(f.to_string());
                    }
                }
            }
        }

        if lower.contains("mail") || lower.contains("메일") {
            for frag in Self::extract_quoted_fragments(goal) {
                if frag.contains("S1_")
                    || frag.contains("S2_")
                    || frag.contains("S3_")
                    || frag.contains("S4_")
                    || frag.contains("S5_")
                {
                    return Some(frag.trim().to_string());
                }
            }
            for frag in Self::extract_quoted_fragments(goal) {
                let f = frag.trim();
                let lf = f.to_lowercase();
                if f.len() < 2 {
                    continue;
                }
                if lf.contains("cmd+") || lf.starts_with("status:") {
                    continue;
                }
                if f.starts_with("RUN_SCOPE_") {
                    continue;
                }
                if email_re.as_ref().map(|re| re.is_match(f)).unwrap_or(false) {
                    continue;
                }
                return Some(f.to_string());
            }
        }
        None
    }

    pub(super) fn extract_known_app_from_text(text: &str) -> Option<&'static str> {
        let lower = Self::normalize_text_for_matching(text);
        for role in [
            AppRole::FileManager,
            AppRole::TextEditor,
            AppRole::NotesApp,
            AppRole::MailClient,
            AppRole::Browser,
            AppRole::Calendar,
            AppRole::Calculator,
        ] {
            if Self::goal_mentions_app_role(&lower, role) {
                return Some(Self::app_name_for_role(role));
            }
        }
        None
    }

    pub(super) fn next_unopened_app_in_goal(
        goal: &str,
        history: &[String],
    ) -> Option<&'static str> {
        for app in Self::ordered_apps_in_goal(goal) {
            if !Self::history_contains_opened_app(history, app) {
                return Some(app);
            }
        }
        None
    }
}
