// CalendarSkill - Google Calendar ??깆젟 鈺곌퀬????밴쉐
// ??沅?? integrations::calendar::CalendarClient

use super::{Skill, SkillContext, SkillResult};
use crate::jarvis::skills::metadata::*;
use async_trait::async_trait;
use serde_json::json;

pub struct CalendarSkill;

impl CalendarSkill {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Skill for CalendarSkill {
    fn metadata(&self) -> SkillMetadata {
        SkillMetadata {
            name: "calendar".to_string(),
            description: "Google Calendar ??깆젟 鈺곌퀬??獄???밴쉐".to_string(),
            version: "1.0.0".to_string(),
            actions: vec![
                "list_today".to_string(),
                "list_week".to_string(),
                "create_event".to_string(),
            ],
            requirements: SkillRequirements {
                env_vars: vec!["GOOGLE_AUTH_TOKEN_PATH".to_string()],
                ..Default::default()
            },
            tags: vec!["productivity".to_string(), "calendar".to_string()],
        }
    }

    fn check_eligibility(&self) -> EligibilityResult {
        check_requirements(&self.metadata().requirements)
    }

    async fn execute(&self, ctx: SkillContext) -> SkillResult {
        match ctx.action.as_str() {
            "list_today" => {
                match crate::integrations::calendar::CalendarClient::new().await {
                    Ok(client) => match client.list_today().await {
                        Ok(events) => {
                            let events_json: Vec<_> = events
                                .iter()
                                .map(|(id, summary, time)| {
                                    json!({"id": id, "summary": summary, "time": time})
                                })
                                .collect();
                            SkillResult::success_with_data(
                                format!("??삳뮎 ??깆젟 {}椰?, events.len()),
                                json!({"events": events_json}),
                            )
                        }
                        Err(e) => SkillResult::error(format!("??깆젟 鈺곌퀬????쎈솭: {}", e)),
                    },
                    Err(e) => SkillResult::error(format!("Calendar ?紐꾩쵄 ??쎈솭: {}", e)),
                }
            }
            "list_week" => {
                match crate::integrations::calendar::CalendarClient::new().await {
                    Ok(client) => match client.list_week().await {
                        Ok(events) => {
                            let events_json: Vec<_> = events
                                .iter()
                                .map(|(id, summary, time)| {
                                    json!({"id": id, "summary": summary, "time": time})
                                })
                                .collect();
                            SkillResult::success_with_data(
                                format!("??苡?雅???깆젟 {}椰?, events.len()),
                                json!({"events": events_json}),
                            )
                        }
                        Err(e) => SkillResult::error(format!("??깆젟 鈺곌퀬????쎈솭: {}", e)),
                    },
                    Err(e) => SkillResult::error(format!("Calendar ?紐꾩쵄 ??쎈솭: {}", e)),
                }
            }
            "create_event" => {
                let title = ctx.params.get("title").and_then(|v| v.as_str()).unwrap_or("????깆젟");
                let start = ctx.params.get("start").and_then(|v| v.as_str()).unwrap_or("");
                let end = ctx.params.get("end").and_then(|v| v.as_str()).unwrap_or("");

                if start.is_empty() {
                    return SkillResult::error("start ???뵬沃섎챸苑ｅ첎? ?袁⑹뒄??몃빍??(ISO 8601)");
                }

                match crate::integrations::calendar::CalendarClient::new().await {
                    Ok(client) => match client.create_event(title, start, end).await {
                        Ok(event_id) => SkillResult::success_with_data(
                            format!("??깆젟 '{}' ??밴쉐 ?袁⑥┷", title),
                            json!({"event_id": event_id, "title": title}),
                        ),
                        Err(e) => SkillResult::error(format!("??깆젟 ??밴쉐 ??쎈솭: {}", e)),
                    },
                    Err(e) => SkillResult::error(format!("Calendar ?紐꾩쵄 ??쎈솭: {}", e)),
                }
            }
            _ => SkillResult::error(format!("Unknown calendar action: {}", ctx.action)),
        }
    }

    fn requires_approval(&self, action: &str) -> bool {
        matches!(action, "create_event")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calendar_skill_metadata() {
        let skill = CalendarSkill::new();
        let meta = skill.metadata();
        assert_eq!(meta.name, "calendar");
        assert_eq!(meta.actions.len(), 3);
        assert!(meta.actions.contains(&"list_today".to_string()));
    }

    #[test]
    fn test_create_event_requires_approval() {
        let skill = CalendarSkill::new();
        assert!(skill.requires_approval("create_event"));
        assert!(!skill.requires_approval("list_today"));
    }
}
