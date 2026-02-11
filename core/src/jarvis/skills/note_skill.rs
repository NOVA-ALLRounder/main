// NoteSkill - Notion 筌롫뗀???臾믨쉐/鈺곌퀬??野꺜??
// ??沅?? integrations::notion::NotionClient

use super::{Skill, SkillContext, SkillResult};
use crate::jarvis::skills::metadata::*;
use async_trait::async_trait;
use serde_json::json;

pub struct NoteSkill;

impl NoteSkill {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Skill for NoteSkill {
    fn metadata(&self) -> SkillMetadata {
        SkillMetadata {
            name: "note".to_string(),
            description: "Notion 筌롫뗀???臾믨쉐, 鈺곌퀬?? 野꺜??.to_string(),
            version: "1.0.0".to_string(),
            actions: vec![
                "create".to_string(),
                "read".to_string(),
                "query".to_string(),
            ],
            requirements: SkillRequirements {
                env_vars: vec![
                    "NOTION_API_KEY".to_string(),
                    "NOTION_DATABASE_ID".to_string(),
                ],
                ..Default::default()
            },
            tags: vec!["productivity".to_string(), "note".to_string()],
        }
    }

    fn check_eligibility(&self) -> EligibilityResult {
        check_requirements(&self.metadata().requirements)
    }

    async fn execute(&self, ctx: SkillContext) -> SkillResult {
        let client = match crate::integrations::notion::NotionClient::from_env() {
            Ok(c) => c,
            Err(e) => return SkillResult::error(format!("Notion ?紐꾩쵄 ??쎈솭: {}", e)),
        };

        let db_id = match std::env::var("NOTION_DATABASE_ID") {
            Ok(id) => id,
            Err(_) => return SkillResult::error("NOTION_DATABASE_ID ??띻펾癰궰??? ??쇱젟??? ??녿릭??щ빍??),
        };

        match ctx.action.as_str() {
            "create" => {
                let title = ctx.params.get("title").and_then(|v| v.as_str()).unwrap_or("??筌롫뗀??);
                let content = ctx.params.get("content").and_then(|v| v.as_str()).unwrap_or("");

                match client.create_page(&db_id, title, content).await {
                    Ok(page_id) => SkillResult::success_with_data(
                        format!("筌롫뗀??'{}' ??밴쉐 ?袁⑥┷", title),
                        json!({"page_id": page_id, "title": title}),
                    ),
                    Err(e) => SkillResult::error(format!("筌롫뗀????밴쉐 ??쎈솭: {}", e)),
                }
            }
            "read" => {
                let page_id = match ctx.params.get("page_id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return SkillResult::error("page_id ???뵬沃섎챸苑ｅ첎? ?袁⑹뒄??몃빍??),
                };

                match client.read_page(page_id).await {
                    Ok(page) => SkillResult::success_with_data(
                        format!("筌롫뗀??鈺곌퀬?? {}", page.title),
                        json!({"id": page.id, "title": page.title, "content": page.content}),
                    ),
                    Err(e) => SkillResult::error(format!("筌롫뗀??鈺곌퀬????쎈솭: {}", e)),
                }
            }
            "query" => {
                let limit = ctx
                    .params
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(10) as usize;

                match client.query_database(&db_id, limit as u32).await {
                    Ok(pages) => {
                        let pages_json: Vec<_> = pages
                            .iter()
                            .map(|p| json!({"id": p.id, "title": p.title}))
                            .collect();
                        SkillResult::success_with_data(
                            format!("筌롫뗀??{}椰?野꺜??, pages.len()),
                            json!({"pages": pages_json}),
                        )
                    }
                    Err(e) => SkillResult::error(format!("筌롫뗀??野꺜????쎈솭: {}", e)),
                }
            }
            _ => SkillResult::error(format!("Unknown note action: {}", ctx.action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_skill_metadata() {
        let skill = NoteSkill::new();
        let meta = skill.metadata();
        assert_eq!(meta.name, "note");
        assert_eq!(meta.actions.len(), 3);
    }
}
