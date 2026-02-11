// System Prompt Builder - "??堉멨칰??臾먮뼗??롫뮉揶쎛"
//
// openclaw筌ｌ꼶??筌????遺얠춳????덉읅??곗쨮 ??뽯뮞???袁⑨세?袁る뱜???닌딄쉐:
// 1. Identity (SOUL.toml) ???源껉봄, 筌띾??? 揶쎛燁살꼵?
// 2. Memory Recall ???????筌롫뗄?놅쭪? ?온??疫꿸퀣堉?top-k
// 3. Recent Conversation ??筌ㅼ뮄???????遺용튋
// 4. Available Skills ??????揶쎛?館釉???쎄텢 筌뤴뫖以?

use anyhow::Result;
use std::sync::Arc;

use crate::infrastructure::database::ChatMessage;
use crate::jarvis::identity::IdentityManager;
use crate::jarvis::memory::MemoryStore;

pub struct SystemPromptBuilder {
    identity: Arc<IdentityManager>,
    memory_store: Arc<MemoryStore>,
}

impl SystemPromptBuilder {
    pub fn new(identity: Arc<IdentityManager>, memory_store: Arc<MemoryStore>) -> Self {
        Self {
            identity,
            memory_store,
        }
    }

    /// ??덉읅 ??뽯뮞???袁⑨세?袁る뱜 ??밴쉐
    pub async fn build(
        &self,
        user_message: &str,
        recent_history: &[ChatMessage],
        available_skills: &[String],
        execution_result: Option<&str>,
    ) -> Result<String> {
        let mut prompt = String::with_capacity(2048);

        // 1. Identity ?諭??
        prompt.push_str(&self.identity.to_prompt_section());

        // 2. ?袁⑹삺 ??볦퍢
        let now = chrono::Local::now();
        prompt.push_str(&format!(
            "\n## Current Time\n{}\n",
            now.format("%Y-%m-%d %H:%M %Z")
        ));

        // 3. Memory Recall (?????筌롫뗄?놅쭪??? ?온??ㅻ쭆 疫꿸퀣堉?
        let memories = self
            .memory_store
            .search_hybrid(user_message, 5)
            .await
            .unwrap_or_default();

        if !memories.is_empty() {
            prompt.push_str("\n## Memory Recall\n");
            prompt.push_str("(????癒? ?온??ㅻ쭆 疫꿸퀣堉?- ?癒?염??살쓦野???뽰뒠??\n");
            for mem in &memories {
                let date_part = mem.created_at.get(..10).unwrap_or(&mem.created_at);
                prompt.push_str(&format!(
                    "- [{}] {} ({})\n",
                    mem.category,
                    mem.content,
                    date_part
                ));
            }
        }

        // 4. Recent Conversation (筌ㅼ뮄??????
        if !recent_history.is_empty() {
            prompt.push_str("\n## Recent Conversation\n");
            let display_count = recent_history.len().min(5);
            for msg in recent_history.iter().rev().take(display_count).rev() {
                let role_label = if msg.role == "user" { "????? } else { "?? };
                let truncated = if msg.content.chars().count() > 150 {
                    let s: String = msg.content.chars().take(150).collect();
                    format!("{}...", s)
                } else {
                    msg.content.clone()
                };
                prompt.push_str(&format!("- {}: {}\n", role_label, truncated));
            }
        }

        // 5. Available Skills
        if !available_skills.is_empty() {
            prompt.push_str("\n## Available Skills\n");
            prompt.push_str(&format!("{}\n", available_skills.join(", ")));
        }

        // 6. Execution Result (??쎄텢 ??쎈뻬 野껉퀗?드첎? ??덈뮉 野껋럩??
        if let Some(result) = execution_result {
            prompt.push_str(&format!(
                "\n## Task Result\n??????遺욧퍕??筌ｌ꼶???野껉퀗??\n{}\n\n??野껉퀗?든몴?獄쏅?源??곗쨮 ?癒?염??살쓦野??臾먮뼗??\n",
                result
            ));
        }

        // 7. ?臾먮뼗 揶쎛??諭??깆뵥
        prompt.push_str(&format!(
            "\n## Response Guidelines\n\
             - {}嚥??臾먮뼗??n\
             - ????癒? {}(????⑦??븍뜄??n\
             - 疫꿸퀣堉????덈뮉 ?類ｋ궖???癒?염??살쓦野???뽰뒠??n\
             - ??곸읈 ????筌띘살뵭???⑥쥓???n",
            self.identity.soul.tone,
            self.identity.user_profile.name,
        ));

        Ok(prompt)
    }

    /// 揶쏄쑬????袁⑨세?袁る뱜 (LLM ??곸뵠, 疫꿸퀡??identity筌?
    pub fn build_simple(&self, execution_result: Option<&str>) -> String {
        let mut prompt = self.identity.to_prompt_section();

        if let Some(result) = execution_result {
            prompt.push_str(&format!(
                "\n## Task Result\n{}\n\n??野껉퀗?든몴?獄쏅?源??곗쨮 ?癒?염??살쓦野??臾먮뼗??\n",
                result
            ));
        }

        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jarvis::identity::{SoulConfig, UserProfile};
    use std::path::PathBuf;

    fn test_identity() -> IdentityManager {
        IdentityManager {
            soul: SoulConfig {
                name: "TestBot".to_string(),
                persona: "???뮞?紐꾩뒠 AI".to_string(),
                language: "ko".to_string(),
                tone: "燁살뮄???띿쓺".to_string(),
                values: vec!["?類μ넇??.to_string()],
                boundaries: vec![],
                expertise: vec!["???뮞??.to_string()],
                quirks: vec![],
            },
            user_profile: UserProfile {
                name: "TestUser".to_string(),
                timezone: "UTC".to_string(),
                work_context: "???뮞??.to_string(),
                preferences: std::collections::HashMap::new(),
            },
            config_path: PathBuf::from("/tmp/test.toml"),
        }
    }

    #[test]
    fn test_build_simple() {
        let identity = Arc::new(test_identity());
        let memory_store = Arc::new(MemoryStore::new(None));
        let builder = SystemPromptBuilder::new(identity, memory_store);

        let prompt = builder.build_simple(None);
        assert!(prompt.contains("TestBot"));
        assert!(prompt.contains("TestUser"));
    }

    #[test]
    fn test_build_simple_with_result() {
        let identity = Arc::new(test_identity());
        let memory_store = Arc::new(MemoryStore::new(None));
        let builder = SystemPromptBuilder::new(identity, memory_store);

        let prompt = builder.build_simple(Some("筌롫뗄???袁⑸꽊 ?袁⑥┷"));
        assert!(prompt.contains("筌롫뗄???袁⑸꽊 ?袁⑥┷"));
        assert!(prompt.contains("Task Result"));
    }
}
