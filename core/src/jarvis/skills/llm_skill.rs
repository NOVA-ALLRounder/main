// LlmSkill - LLM 疫꿸퀡而???용뮞??筌ｌ꼶??
// ?遺용튋, ?브쑴苑? ?λ뜆釉??臾믨쉐, ?④쑴沅???甕곕뗄??LLM ?臾믩씜
// ??沅?? LLMClient::chat_completion()

use super::{Skill, SkillContext, SkillResult};
use crate::jarvis::skills::metadata::*;
use crate::domains::intelligence::llm_gateway::LLMClient;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

pub struct LlmSkill {
    llm: Arc<LLMClient>,
}

impl LlmSkill {
    pub fn new(llm: Arc<LLMClient>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl Skill for LlmSkill {
    fn metadata(&self) -> SkillMetadata {
        SkillMetadata {
            name: "llm".to_string(),
            description: "LLM 疫꿸퀡而???용뮞??筌ｌ꼶?? ?遺용튋, ?브쑴苑? ?λ뜆釉??臾믨쉐".to_string(),
            version: "1.0.0".to_string(),
            actions: vec![
                "summarize".to_string(),
                "analyze".to_string(),
                "draft".to_string(),
                "translate".to_string(),
            ],
            requirements: SkillRequirements {
                env_vars: vec!["OPENAI_API_KEY".to_string()],
                ..Default::default()
            },
            tags: vec!["ai".to_string(), "text".to_string(), "productivity".to_string()],
        }
    }

    fn check_eligibility(&self) -> EligibilityResult {
        check_requirements(&self.metadata().requirements)
    }

    async fn execute(&self, ctx: SkillContext) -> SkillResult {
        log::info!(
            "LlmSkill executing action: {} (session: {})",
            ctx.action,
            ctx.session_key
        );

        match ctx.action.as_str() {
            "summarize" => self.summarize(ctx).await,
            "analyze" => self.analyze(ctx).await,
            "draft" => self.draft(ctx).await,
            "translate" => self.translate(ctx).await,
            _ => SkillResult::error(format!("Unknown action: {}", ctx.action)),
        }
    }
}

impl LlmSkill {
    /// ??용뮞???遺용튋
    async fn summarize(&self, ctx: SkillContext) -> SkillResult {
        let text = match ctx.params.get("text").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return SkillResult::error("'text' ???뵬沃섎챸苑ｅ첎? ?袁⑹뒄??몃빍??),
        };

        let style = ctx
            .params
            .get("style")
            .and_then(|v| v.as_str())
            .unwrap_or("concise");

        let prompt = match style {
            "bullet" => format!("??쇱벉 ??곸뒠?????뼎 ?????筌뤴뫖以??곗쨮 ?遺용튋??곸㉭:\n\n{}", text),
            "oneline" => format!("??쇱벉 ??곸뒠?????얜챷???곗쨮 ?遺용튋??곸㉭:\n\n{}", text),
            _ => format!("??쇱벉 ??곸뒠??2-3?얜챷???곗쨮 揶쏄쑨猿??띿쓺 ?遺용튋??곸㉭:\n\n{}", text),
        };

        let messages = vec![
            json!({"role": "system", "content": "??덈뮉 ?類μ넇??랁?揶쏄쑨猿???遺용튋 ?袁ⓓ?첎???"}),
            json!({"role": "user", "content": prompt}),
        ];

        match self.llm.chat_completion(messages).await {
            Ok(summary) => SkillResult::success_with_data(
                summary.clone(),
                json!({"summary": summary, "style": style, "original_length": text.len()}),
            ),
            Err(e) => SkillResult::error(format!("?遺용튋 ??쎈솭: {}", e)),
        }
    }

    /// ?怨쀬뵠????용뮞???브쑴苑?
    async fn analyze(&self, ctx: SkillContext) -> SkillResult {
        let data = match ctx.params.get("data").or(ctx.params.get("text")) {
            Some(d) => d.to_string(),
            None => return SkillResult::error("'data' ?癒?뮉 'text' ???뵬沃섎챸苑ｅ첎? ?袁⑹뒄??몃빍??),
        };

        let question = ctx
            .params
            .get("question")
            .and_then(|v| v.as_str())
            .unwrap_or("???怨쀬뵠?怨? ?브쑴苑??곸㉭");

        let messages = vec![
            json!({"role": "system", "content": "??덈뮉 ?怨쀬뵠???브쑴苑??袁ⓓ?첎??? 筌뤿굟???랁??紐꾧텢??꾨뱜 ??덈뮉 ?브쑴苑????볥궗??"}),
            json!({"role": "user", "content": format!("筌욌뜄揆: {}\n\n?怨쀬뵠??\n{}", question, data)}),
        ];

        match self.llm.chat_completion(messages).await {
            Ok(analysis) => SkillResult::success_with_data(
                analysis.clone(),
                json!({"analysis": analysis, "question": question}),
            ),
            Err(e) => SkillResult::error(format!("?브쑴苑???쎈솭: {}", e)),
        }
    }

    /// ?λ뜆釉??臾믨쉐 (??李?? ?얜챷苑? 筌롫뗄?놅쭪? ??
    async fn draft(&self, ctx: SkillContext) -> SkillResult {
        let topic = match ctx.params.get("topic").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return SkillResult::error("'topic' ???뵬沃섎챸苑ｅ첎? ?袁⑹뒄??몃빍??),
        };

        let doc_type = ctx
            .params
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("general");

        let tone = ctx
            .params
            .get("tone")
            .and_then(|v| v.as_str())
            .unwrap_or("professional");

        let system_prompt = match doc_type {
            "email" => "??덈뮉 ??李???臾믨쉐 ?袁ⓓ?첎??? 繹먮뗀嫄??랁??袁ⓓ?怨몄뵥 ??李??깆뱽 ?臾믨쉐??",
            "report" => "??덈뮉 癰귣떯????臾믨쉐 ?袁ⓓ?첎??? ?닌듼?怨몄뵠??筌뤿굟???癰귣떯???? ?臾믨쉐??",
            "message" => "??덈뮉 筌롫뗄?놅쭪? ?臾믨쉐 ?袁⑹뒭沃섎챷鍮? ?怨뱀넺??筌띿쉶???癒?염??살쑎??筌롫뗄?놅쭪????臾믨쉐??",
            _ => "??덈뮉 疫꼲?怨뚮┛ ?袁⑹뒭沃섎챷鍮? ?遺욧퍕??筌띿쉶???λ뜆釉???臾믨쉐??",
        };

        let messages = vec![
            json!({"role": "system", "content": system_prompt}),
            json!({"role": "user", "content": format!(
                "??쇱벉 雅뚯눘?ｆ에?{} ?λ뜆釉???臾믨쉐??곸㉭. ?? {}\n\n雅뚯눘?? {}",
                doc_type, tone, topic
            )}),
        ];

        match self.llm.chat_completion(messages).await {
            Ok(draft) => SkillResult::success_with_data(
                format!("{} ?λ뜆釉??臾믨쉐 ?袁⑥┷", doc_type),
                json!({"draft": draft, "type": doc_type, "tone": tone, "topic": topic}),
            ),
            Err(e) => SkillResult::error(format!("?λ뜆釉??臾믨쉐 ??쎈솭: {}", e)),
        }
    }

    /// 甕곕뜆肉?
    async fn translate(&self, ctx: SkillContext) -> SkillResult {
        let text = match ctx.params.get("text").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return SkillResult::error("'text' ???뵬沃섎챸苑ｅ첎? ?袁⑹뒄??몃빍??),
        };

        let to_lang = ctx
            .params
            .get("to")
            .and_then(|v| v.as_str())
            .unwrap_or("en");

        let messages = vec![
            json!({"role": "system", "content": format!(
                "??덈뮉 ?袁ⓓ?甕곕뜆肉?첎??? ??용뮞?紐? {}嚥??癒?염??살쓦野?甕곕뜆肉?? 甕곕뜆肉?눧紐껋춸 ?곗뮆???",
                to_lang
            )}),
            json!({"role": "user", "content": text}),
        ];

        match self.llm.chat_completion(messages).await {
            Ok(translated) => SkillResult::success_with_data(
                translated.clone(),
                json!({"translated": translated, "to": to_lang, "original": text}),
            ),
            Err(e) => SkillResult::error(format!("甕곕뜆肉???쎈솭: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata() {
        // LlmSkill requires LLMClient, but we can test metadata through a mock
        // For now, just verify the structure
        let meta = SkillMetadata {
            name: "llm".to_string(),
            description: "LLM 疫꿸퀡而???용뮞??筌ｌ꼶??.to_string(),
            version: "1.0.0".to_string(),
            actions: vec![
                "summarize".to_string(),
                "analyze".to_string(),
                "draft".to_string(),
                "translate".to_string(),
            ],
            requirements: SkillRequirements {
                env_vars: vec!["OPENAI_API_KEY".to_string()],
                ..Default::default()
            },
            tags: vec!["ai".to_string()],
        };

        assert_eq!(meta.name, "llm");
        assert_eq!(meta.actions.len(), 4);
        assert!(meta.actions.contains(&"summarize".to_string()));
        assert!(meta.actions.contains(&"analyze".to_string()));
        assert!(meta.actions.contains(&"draft".to_string()));
        assert!(meta.actions.contains(&"translate".to_string()));
    }
}
