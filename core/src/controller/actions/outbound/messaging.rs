use super::super::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) fn goal_mentions_mail(goal: &str) -> bool {
        let lower = goal.to_lowercase();
        lower.contains("mail") || lower.contains("gmail") || lower.contains("메일")
    }

    pub(in crate::controller::actions) fn goal_mentions_telegram(goal: &str) -> bool {
        let lower = goal.to_lowercase();
        lower.contains("telegram") || lower.contains("텔레그램")
    }

    pub(in crate::controller::actions) fn latest_read_result_from_history(
        history: &[String],
    ) -> Option<String> {
        for entry in history.iter().rev() {
            if let Some(rest) = entry.strip_prefix("READ_RESULT: ") {
                let trimmed = rest.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        None
    }

    pub(in crate::controller::actions) fn preferred_telegram_message(
        plan: &serde_json::Value,
        goal: &str,
        history: &[String],
    ) -> String {
        if let Some(raw) = plan
            .get("message")
            .and_then(|v| v.as_str())
            .or_else(|| plan.get("text").and_then(|v| v.as_str()))
        {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

        if let Some(read_result) = Self::latest_read_result_from_history(history) {
            return format!("요청 결과 요약:\n{}", read_result);
        }

        let fragments = Self::extract_quoted_fragments(goal)
            .into_iter()
            .filter(|frag| {
                let trimmed = frag.trim();
                let lower = trimmed.to_lowercase();
                !trimmed.is_empty()
                    && Self::normalize_email_candidate(trimmed).is_none()
                    && !lower.starts_with("cmd+")
                    && !lower.starts_with("status:")
                    && !trimmed.starts_with("RUN_SCOPE_")
            })
            .collect::<Vec<_>>();
        if !fragments.is_empty() {
            return format!("요청 내용 요약:\n{}", fragments.join("\n"));
        }

        if Self::goal_mentions_telegram(goal) {
            return "요청한 작업을 실행했습니다. 세부 로그는 앱/리포트에서 확인해주세요."
                .to_string();
        }

        "요청한 작업을 실행했습니다.".to_string()
    }

    pub(in crate::controller::actions) fn strip_markup_for_mail_body(raw: &str) -> String {
        let with_breaks = raw
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace("<br />", "\n")
            .replace("<br/>", "\n")
            .replace("<br>", "\n")
            .replace("&nbsp;", " ");

        let mut out = String::with_capacity(with_breaks.len());
        let mut in_tag = false;
        for ch in with_breaks.chars() {
            if ch == '<' {
                in_tag = true;
                continue;
            }
            if ch == '>' {
                in_tag = false;
                continue;
            }
            if !in_tag {
                out.push(ch);
            }
        }

        let decoded = out
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&");

        decoded
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(in crate::controller::actions) fn scripted_mail_body_fallback(
        goal: &str,
        history: &[String],
    ) -> Option<(String, String)> {
        let prefer_notes = Self::last_text_app_from_history(history)
            .map(|app| app.eq_ignore_ascii_case("Notes"))
            .unwrap_or(true);
        let source_order = if prefer_notes {
            ["Notes", "TextEdit"]
        } else {
            ["TextEdit", "Notes"]
        };

        for source in source_order {
            let raw = if source == "Notes" {
                Self::notes_read_text(Some(goal))
            } else {
                Self::textedit_read_text(Some(goal))
            };

            if let Ok(text) = raw {
                let normalized = Self::strip_markup_for_mail_body(&text);
                if normalized.chars().count() >= 6 {
                    return Some((source.to_string(), normalized));
                }
            }
        }
        None
    }
}
