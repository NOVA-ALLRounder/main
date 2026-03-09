use super::super::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) fn normalize_email_candidate(raw: &str) -> Option<String> {
        let email_re =
            regex::Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").ok()?;
        let matched = email_re.find(raw)?.as_str();
        let trimmed = matched.trim_matches(|c: char| {
            matches!(
                c,
                '"' | '\'' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | ':' | '.'
            )
        });
        if trimmed.is_empty() || trimmed.contains(' ') {
            return None;
        }
        let mut parts = trimmed.split('@');
        let local = parts.next().unwrap_or_default();
        let domain = parts.next().unwrap_or_default();
        if local.is_empty() || domain.is_empty() || parts.next().is_some() {
            return None;
        }
        if !domain.contains('.') {
            return None;
        }
        Some(trimmed.to_string())
    }

    pub(in crate::controller::actions) fn extract_mail_recipient_from_goal(
        goal: &str,
    ) -> Option<String> {
        Self::normalize_email_candidate(goal)
    }

    pub(in crate::controller::actions) fn preferred_mail_recipient(
        goal: Option<&str>,
    ) -> Option<String> {
        if let Some(g) = goal {
            if let Some(recipient) = Self::extract_mail_recipient_from_goal(g) {
                return Some(recipient);
            }
        }
        Self::default_mail_recipient()
    }

    pub(in crate::controller::actions) fn preferred_mail_subject(
        goal: Option<&str>,
    ) -> Option<String> {
        let goal_text = goal?.trim();
        if goal_text.is_empty() {
            return None;
        }

        let lower_goal = goal_text.to_lowercase();
        let mut scopes: Vec<&str> = Vec::new();
        let mail_idx = lower_goal
            .rfind("mail")
            .or_else(|| lower_goal.rfind("메일"))
            .or_else(|| lower_goal.rfind("이메일"));
        if let Some(idx) = mail_idx {
            scopes.push(&goal_text[idx..]);
        }
        scopes.push(goal_text);

        for scope in scopes {
            let scope_lower = scope.to_lowercase();
            for keyword in ["제목", "subject", "title"] {
                if let Some(idx) = scope_lower.find(keyword) {
                    let tail = &scope[idx..];
                    for frag in Self::extract_quoted_fragments(tail) {
                        if Self::normalize_email_candidate(&frag).is_some() {
                            continue;
                        }
                        if frag.starts_with("RUN_SCOPE_") {
                            continue;
                        }
                        let lower_frag = frag.to_lowercase();
                        if lower_frag.starts_with("cmd+") || lower_frag.starts_with("status:") {
                            continue;
                        }
                        return Some(frag);
                    }
                }
            }
        }

        let quoted = Self::extract_quoted_fragments(goal_text);
        if quoted.is_empty() {
            return None;
        }

        for frag in &quoted {
            if frag.contains("S1_")
                || frag.contains("S2_")
                || frag.contains("S3_")
                || frag.contains("S4_")
                || frag.contains("S5_")
                || frag.contains("DONE_")
            {
                return Some(frag.clone());
            }
        }

        for frag in &quoted {
            if Self::normalize_email_candidate(frag).is_some() {
                continue;
            }
            if frag.starts_with("RUN_SCOPE_") {
                continue;
            }
            return Some(frag.clone());
        }

        quoted.first().cloned()
    }

    pub(in crate::controller::actions) fn preferred_run_scope_marker(
        goal: Option<&str>,
    ) -> Option<String> {
        let goal_text = goal?.trim();
        if goal_text.is_empty() {
            return None;
        }

        let is_scope_marker = |candidate: &str| {
            let upper = candidate.to_ascii_uppercase();
            if !upper.starts_with("RUN_SCOPE_") || candidate.len() <= "RUN_SCOPE_".len() {
                return false;
            }
            candidate
                .chars()
                .skip("RUN_SCOPE_".len())
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        };

        let quoted = Self::extract_quoted_fragments(goal_text);
        for frag in quoted.iter().rev() {
            let cleaned = frag.trim_matches(|c: char| {
                matches!(
                    c,
                    '"' | '\'' | ',' | '.' | ';' | ':' | ')' | '(' | ']' | '[' | '}' | '{'
                )
            });
            if is_scope_marker(cleaned) {
                return Some(cleaned.to_string());
            }
        }

        for raw in goal_text.split_whitespace() {
            let cleaned = raw.trim_matches(|c: char| {
                matches!(
                    c,
                    '"' | '\'' | ',' | '.' | ';' | ':' | ')' | '(' | ']' | '[' | '}' | '{'
                )
            });
            if is_scope_marker(cleaned) {
                return Some(cleaned.to_string());
            }
        }
        None
    }

    pub(in crate::controller::actions) fn default_mail_recipient() -> Option<String> {
        let candidates = [
            "STEER_DEFAULT_MAIL_TO",
            "STEER_USER_EMAIL",
            "APPLE_ID_EMAIL",
        ];
        for key in candidates {
            if let Ok(value) = std::env::var(key) {
                let trimmed = value.trim();
                if trimmed.contains('@') && !trimmed.contains(' ') {
                    return Some(trimmed.to_string());
                }
            }
        }
        Self::mail_account_primary_email()
    }

    pub(in crate::controller::actions) fn mail_account_primary_email() -> Option<String> {
        let lines = [
            "tell application \"Mail\"",
            "repeat with ac in accounts",
            "try",
            "set addrList to email addresses of ac",
            "if addrList is not missing value and (count of addrList) > 0 then",
            "set candidate to item 1 of addrList as text",
            "if candidate is not \"\" then return candidate",
            "end if",
            "end if",
            "end try",
            "try",
            "set candidate to user name of ac as text",
            "if candidate contains \"@\" then return candidate",
            "end try",
            "end repeat",
            "end tell",
            "return \"\"",
        ];
        let out = crate::applescript::run_with_args(&lines, &Vec::<String>::new()).ok()?;
        Self::normalize_email_candidate(out.trim())
    }
}
