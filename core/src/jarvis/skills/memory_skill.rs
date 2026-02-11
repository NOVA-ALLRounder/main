// MemorySkill - 疫꿸퀣堉??온????쎄텢
// "??욧탢 疫꿸퀣堉??곸㉭" ??memory.save
// "??? ?袁⑸퓠 ?몃Ŧ?ゆ?????" ??memory.search
// "域밸㈇援???녿선餓? ??memory.forget
//
// openclaw??memory_search / memory_get ???쉘

use super::{Skill, SkillContext, SkillResult};
use crate::jarvis::skills::metadata::*;
use crate::jarvis::memory::MemoryStore;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

pub struct MemorySkill {
    memory_store: Arc<MemoryStore>,
}

impl MemorySkill {
    pub fn new(memory_store: Arc<MemoryStore>) -> Self {
        Self { memory_store }
    }
}

#[async_trait]
impl Skill for MemorySkill {
    fn metadata(&self) -> SkillMetadata {
        SkillMetadata {
            name: "memory".to_string(),
            description: "疫꿸퀣堉????? 野꺜?? ????- ?觀由?疫꿸퀣堉??온??.to_string(),
            version: "1.0.0".to_string(),
            actions: vec![
                "save".to_string(),
                "search".to_string(),
                "forget".to_string(),
                "list".to_string(),
            ],
            requirements: SkillRequirements::default(), // DB筌??袁⑹뒄 (??湲?揶쎛??
            tags: vec!["memory".to_string(), "core".to_string()],
        }
    }

    fn check_eligibility(&self) -> EligibilityResult {
        // MemoryStore??DB 疫꿸퀡而???嚥???湲?????揶쎛??
        EligibilityResult::eligible()
    }

    async fn execute(&self, ctx: SkillContext) -> SkillResult {
        log::info!(
            "MemorySkill executing action: {} (session: {})",
            ctx.action,
            ctx.session_key
        );

        match ctx.action.as_str() {
            "save" => self.save(ctx).await,
            "search" => self.search(ctx).await,
            "forget" => self.forget(ctx).await,
            "list" => self.list(ctx).await,
            _ => SkillResult::error(format!("Unknown action: {}", ctx.action)),
        }
    }
}

impl MemorySkill {
    /// 疫꿸퀣堉?????("??욧탢 疫꿸퀣堉??곸㉭")
    async fn save(&self, ctx: SkillContext) -> SkillResult {
        let content = match ctx.params.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return SkillResult::error("'content' ???뵬沃섎챸苑ｅ첎? ?袁⑹뒄??몃빍??),
        };

        let category = ctx
            .params
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("fact");

        let importance = ctx
            .params
            .get("importance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.7); // 筌뤿굞??????關? 疫꿸퀡??餓λ쵐????誘れ벉

        match self
            .memory_store
            .store(content, category, "explicit_save", importance)
            .await
        {
            Ok(id) => SkillResult::success_with_data(
                format!("疫꿸퀣堉??됰뮸??덈뼄: {}", &content[..content.len().min(50)]),
                json!({
                    "memory_id": id,
                    "content": content,
                    "category": category,
                    "importance": importance,
                }),
            ),
            Err(e) => SkillResult::error(format!("疫꿸퀣堉???????쎈솭: {}", e)),
        }
    }

    /// 疫꿸퀣堉?野꺜??("??? ?袁⑸퓠 ?몃Ŧ?ゆ?????")
    async fn search(&self, ctx: SkillContext) -> SkillResult {
        let query = match ctx.params.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return SkillResult::error("'query' ???뵬沃섎챸苑ｅ첎? ?袁⑹뒄??몃빍??),
        };

        let limit = ctx
            .params
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;

        match self.memory_store.search_hybrid(query, limit).await {
            Ok(memories) => {
                if memories.is_empty() {
                    return SkillResult::success_with_data(
                        format!("'{}'??????疫꿸퀣堉????곷뮸??덈뼄", query),
                        json!({"query": query, "results": [], "count": 0}),
                    );
                }

                let results: Vec<serde_json::Value> = memories
                    .iter()
                    .map(|m| {
                        json!({
                            "id": m.id,
                            "content": m.content,
                            "category": m.category,
                            "importance": m.importance,
                            "score": m.score,
                            "created_at": m.created_at,
                        })
                    })
                    .collect();

                SkillResult::success_with_data(
                    format!("{}椰꾨똻???온??疫꿸퀣堉??筌≪뼚釉??щ빍??, memories.len()),
                    json!({
                        "query": query,
                        "results": results,
                        "count": memories.len(),
                    }),
                )
            }
            Err(e) => SkillResult::error(format!("疫꿸퀣堉?野꺜????쎈솭: {}", e)),
        }
    }

    /// 疫꿸퀣堉?????("域밸㈇援???녿선餓?)
    async fn forget(&self, ctx: SkillContext) -> SkillResult {
        let memory_id = match ctx.params.get("memory_id").and_then(|v| v.as_i64()) {
            Some(id) => id,
            None => return SkillResult::error("'memory_id' ???뵬沃섎챸苑ｅ첎? ?袁⑹뒄??몃빍??(??ъ쁽)"),
        };

        match self.memory_store.forget(memory_id) {
            Ok(()) => SkillResult::success_with_data(
                format!("疫꿸퀣堉?#{} ?????袁⑥┷", memory_id),
                json!({"deleted_id": memory_id}),
            ),
            Err(e) => SkillResult::error(format!("疫꿸퀣堉???????쎈솭: {}", e)),
        }
    }

    /// 疫꿸퀣堉?筌뤴뫖以?鈺곌퀬??(筌ㅼ뮄??
    async fn list(&self, ctx: SkillContext) -> SkillResult {
        let limit = ctx
            .params
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        let category = ctx.params.get("category").and_then(|v| v.as_str());

        // ??쇱뜖??野꺜??깆몵嚥??袁⑷퍥 鈺곌퀬??(??野꺜??깅선 = ?袁⑷퍥)
        let query = if let Some(cat) = category {
            cat
        } else {
            ""
        };

        // ??쇱뜖??野꺜??????(???얜챷???곸뵠筌?LIKE '%%' = ?袁⑷퍥)
        match self.memory_store.search_keyword(query, limit) {
            Ok(memories) => {
                let results: Vec<serde_json::Value> = memories
                    .iter()
                    .map(|m| {
                        json!({
                            "id": m.id,
                            "content": m.content,
                            "category": m.category,
                            "importance": m.importance,
                            "created_at": m.created_at,
                        })
                    })
                    .collect();

                let count = self.memory_store.count().unwrap_or(0);

                SkillResult::success_with_data(
                    format!("疫꿸퀣堉?{}椰?鈺곌퀬??(?袁⑷퍥 {}椰?", results.len(), count),
                    json!({
                        "results": results,
                        "count": results.len(),
                        "total": count,
                    }),
                )
            }
            Err(e) => SkillResult::error(format!("疫꿸퀣堉?筌뤴뫖以?鈺곌퀬????쎈솭: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata() {
        let store = Arc::new(MemoryStore::new(None));
        let skill = MemorySkill::new(store);
        let meta = skill.metadata();

        assert_eq!(meta.name, "memory");
        assert_eq!(meta.actions.len(), 4);
        assert!(meta.actions.contains(&"save".to_string()));
        assert!(meta.actions.contains(&"search".to_string()));
        assert!(meta.actions.contains(&"forget".to_string()));
        assert!(meta.actions.contains(&"list".to_string()));
    }

    #[test]
    fn test_always_eligible() {
        let store = Arc::new(MemoryStore::new(None));
        let skill = MemorySkill::new(store);
        assert!(skill.check_eligibility().eligible);
    }
}
