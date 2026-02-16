use crate::db;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowQueryHints {
    pub derived_query: String,
    pub reasons: Vec<String>,
    pub keywords: Vec<String>,
}

fn normalize_keyword_token(raw: &str) -> Option<String> {
    let mut token = raw
        .trim()
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .to_lowercase();
    if token.is_empty() {
        return None;
    }
    if token.len() < 3 || token.len() > 28 {
        return None;
    }
    if token.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if ["http", "https", "www", "com", "gmail", "notion", "telegram", "google"]
        .contains(&token.as_str())
    {
        return None;
    }
    token.retain(|c| c.is_alphanumeric() || c == '-' || c == '_');
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

pub fn derive_email_workflow_hints(default_query: &str) -> WorkflowQueryHints {
    let mut keyword_scores: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut app_scores: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    if let Ok(events) = db::get_recent_events(72) {
        for event_json in events.iter().rev().take(600) {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(event_json) else {
                continue;
            };

            if let Some(app) = v.get("app").and_then(|x| x.as_str()) {
                let a = app.trim().to_lowercase();
                if !a.is_empty() {
                    *app_scores.entry(a).or_insert(0) += 1;
                }
            }

            let mut raw_texts: Vec<&str> = Vec::new();
            if let Some(s) = v.get("window_title").and_then(|x| x.as_str()) {
                raw_texts.push(s);
            }
            if let Some(s) = v.get("event_type").and_then(|x| x.as_str()) {
                raw_texts.push(s);
            }
            if let Some(payload) = v.get("payload") {
                if let Some(s) = payload.get("text").and_then(|x| x.as_str()) {
                    raw_texts.push(s);
                }
                if let Some(s) = payload.get("title").and_then(|x| x.as_str()) {
                    raw_texts.push(s);
                }
                if let Some(s) = payload.get("subject").and_then(|x| x.as_str()) {
                    raw_texts.push(s);
                }
            }

            for text in raw_texts {
                for part in text.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_') {
                    if let Some(token) = normalize_keyword_token(part) {
                        let w = if [
                            "invoice",
                            "payment",
                            "billing",
                            "receipt",
                            "urgent",
                            "escalation",
                            "meeting",
                            "agenda",
                            "schedule",
                            "action",
                            "required",
                            "deadline",
                        ]
                        .contains(&token.as_str())
                        {
                            3
                        } else {
                            1
                        };
                        *keyword_scores.entry(token).or_insert(0) += w;
                    }
                }
            }
        }
    }

    let mut ranked_keywords: Vec<(String, usize)> = keyword_scores.into_iter().collect();
    ranked_keywords.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let keywords: Vec<String> = ranked_keywords
        .into_iter()
        .filter(|(_, score)| *score >= 2)
        .map(|(k, _)| k)
        .take(6)
        .collect();

    let mut ranked_apps: Vec<(String, usize)> = app_scores.into_iter().collect();
    ranked_apps.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let top_apps: Vec<String> = ranked_apps
        .into_iter()
        .map(|(a, _)| a)
        .filter(|a| !a.is_empty())
        .take(3)
        .collect();

    let mut reasons = Vec::new();
    if !top_apps.is_empty() {
        reasons.push(format!("Top active apps(72h): {}", top_apps.join(", ")));
    }
    if !keywords.is_empty() {
        reasons.push(format!("Frequent intent keywords: {}", keywords.join(", ")));
    }

    let derived_query = if keywords.is_empty() {
        default_query.to_string()
    } else {
        format!("({}) ({})", default_query, keywords.join(" OR "))
    };

    WorkflowQueryHints {
        derived_query,
        reasons,
        keywords,
    }
}

pub fn score_email_priority(subject: &str, snippet: &str, hints: &WorkflowQueryHints) -> i32 {
    let text = format!("{} {}", subject.to_lowercase(), snippet.to_lowercase());
    let mut score = 0i32;
    for kw in &hints.keywords {
        if text.contains(kw) {
            score += 3;
        }
    }
    for urgent in ["urgent", "action required", "asap", "today", "deadline", "escalation"] {
        if text.contains(urgent) {
            score += 4;
        }
    }
    if text.contains("invoice") || text.contains("payment") || text.contains("billing") {
        score += 3;
    }
    if text.contains("meeting") || text.contains("agenda") || text.contains("schedule") {
        score += 2;
    }
    score
}
