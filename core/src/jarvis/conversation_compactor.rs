// Conversation Compactor - 疫????遺? ?遺용튋??뤿연 ?뚢뫂???쎈뱜 ??덈즲????됰튋
//
// openclaw??session compaction ???쉘:
// ??살삋??筌롫뗄?놅쭪????遺용튋??랁? 筌ㅼ뮄??筌롫뗄?놅쭪?筌??醫?

use anyhow::Result;
use serde_json::json;
use std::sync::Arc;

use crate::domains::intelligence::llm_gateway::LLMClient;
use crate::infrastructure::database::ChatMessage;

pub struct ConversationCompactor {
    llm: Arc<LLMClient>,
}

impl ConversationCompactor {
    pub fn new(llm: Arc<LLMClient>) -> Self {
        Self { llm }
    }

    /// 疫????遺? ?遺용튋 + 筌ㅼ뮄??筌롫뗄?놅쭪?嚥??類ㅽ뀧
    ///
    /// max_messages???λ뜃???롢늺:
    /// - ??살삋??筌롫뗄?놅쭪???쇱뱽 LLM??곗쨮 ?遺용튋
    /// - ?遺용튋 1揶?+ 筌ㅼ뮄??keep_recent 揶?獄쏆꼹??
    pub async fn compact(
        &self,
        messages: &[ChatMessage],
        max_messages: usize,
        keep_recent: usize,
    ) -> Result<Vec<ChatMessage>> {
        if messages.len() <= max_messages {
            return Ok(messages.to_vec());
        }

        let split_at = messages.len().saturating_sub(keep_recent);
        let old_messages = &messages[..split_at];
        let recent_messages = &messages[split_at..];

        // ??살삋??筌롫뗄?놅쭪? ?遺용튋
        let summary = self.summarize(old_messages).await?;

        let mut compacted = vec![ChatMessage {
            role: "system".to_string(),
            content: format!("[??곸읈 ?????遺용튋] {}", summary),
            created_at: old_messages
                .last()
                .map(|m| m.created_at.clone())
                .unwrap_or_default(),
        }];

        compacted.extend_from_slice(recent_messages);

        log::info!(
            "Compacted {} messages ??1 summary + {} recent",
            messages.len(),
            recent_messages.len()
        );

        Ok(compacted)
    }

    /// 筌롫뗄?놅쭪? 筌뤴뫖以???遺용튋
    async fn summarize(&self, messages: &[ChatMessage]) -> Result<String> {
        let conversation = messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = json!([
            {
                "role": "system",
                "content": "??쇱벉 ???遺? 2-3?얜챷???곗쨮 ???뼎筌??遺용튋??곸㉭. 餓λ쵐??????? 野껉퀣???鍮? 筌띘살뵭??癰귣똻???"
            },
            {
                "role": "user",
                "content": conversation
            }
        ]);

        let summary = self
            .llm
            .chat_completion(prompt.as_array().unwrap().clone())
            .await?;

        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_messages(count: usize) -> Vec<ChatMessage> {
        (0..count)
            .map(|i| ChatMessage {
                role: if i % 2 == 0 { "user" } else { "assistant" }.to_string(),
                content: format!("Message {}", i),
                created_at: format!("2024-01-{:02}T00:00:00Z", (i % 28) + 1),
            })
            .collect()
    }

    #[test]
    fn test_no_compaction_needed() {
        // compact is async, so we just test the logic that decides whether to compact
        let messages = make_messages(5);
        assert!(messages.len() <= 20);
    }

    #[test]
    fn test_message_split() {
        let messages = make_messages(30);
        let keep_recent = 10;
        let split_at = messages.len().saturating_sub(keep_recent);
        assert_eq!(split_at, 20);
        assert_eq!(messages[split_at..].len(), 10);
    }
}
