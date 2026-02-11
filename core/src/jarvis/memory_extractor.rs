// Memory Extractor - ???遺용퓠??餓λ쵐???類ｋ궖???癒?짗 ?곕뗄???뤿연 ?觀由?疫꿸퀣堉??곗쨮 ????
//
// ????N??彛??LLM???????뤿연 疫꿸퀣堉??筌띾슦釉??類ｋ궖???곕뗄??
// - ??????醫륁깈?? ???, 餓λ쵐??????? 野껉퀣???鍮???

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::domains::intelligence::llm_gateway::LLMClient;
use crate::infrastructure::database::ChatMessage;
use crate::jarvis::memory::MemoryStore;

/// ?곕뗄???疫꿸퀣堉?????
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedMemory {
    pub content: String,
    pub category: String,
    pub importance: f64,
}

pub struct MemoryExtractor {
    llm: Arc<LLMClient>,
    memory_store: Arc<MemoryStore>,
}

impl MemoryExtractor {
    pub fn new(llm: Arc<LLMClient>, memory_store: Arc<MemoryStore>) -> Self {
        Self { llm, memory_store }
    }

    /// ???遺용퓠??疫꿸퀣堉??筌띾슦釉??類ｋ궖 ?곕뗄????????
    pub async fn extract_and_store(&self, messages: &[ChatMessage]) -> Result<Vec<ExtractedMemory>> {
        if messages.is_empty() {
            return Ok(Vec::new());
        }

        let extracted = self.extract_from_conversation(messages).await?;

        for mem in &extracted {
            if let Err(e) = self
                .memory_store
                .store(&mem.content, &mem.category, "auto_extract", mem.importance)
                .await
            {
                log::warn!("Failed to store extracted memory: {}", e);
            }
        }

        if !extracted.is_empty() {
            log::info!("Extracted and stored {} memories from conversation", extracted.len());
        }

        Ok(extracted)
    }

    /// ???遺용퓠??疫꿸퀣堉??筌띾슦釉??類ｋ궖 ?곕뗄??(???館釉?쭪? ??놁벉)
    async fn extract_from_conversation(
        &self,
        messages: &[ChatMessage],
    ) -> Result<Vec<ExtractedMemory>> {
        // ???遺? ??용뮞?紐껋쨮 癰궰??
        let conversation = messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let system_prompt = r#"??쇱벉 ???遺용퓠???觀由?怨몄몵嚥?疫꿸퀣堉??곷튊 ??餓λ쵐???類ｋ궖???곕뗄???곸㉭.

?곕뗄??疫꿸퀣?:
- ????癒?벥 揶쏆뮇???類ｋ궖 (??已? 筌욊낯毓? ?온??沅???
- ????癒?벥 ?醫륁깈?袁④돌 ???
- 餓λ쵐????????援?野껉퀣???鍮?
- 獄쏆꼶???롫뮉 ?遺욧퍕 ???쉘
- ?館??筌〓챷?쒎첎? ?袁⑹뒄???類ｋ궖

category ?ル굝履? fact, preference, event, conversation
importance: 0.0~1.0 (?誘れ뱽??롮쨯 餓λ쵐??

JSON 獄쏄퀣肉닸에?뺤춸 ?臾먮뼗?? 疫꿸퀣堉??野???곸몵筌???獄쏄퀣肉?[].

??됰뻻:
[
  {"content": "????癒?뮉 AAPL 雅뚯눘??100雅뚯눖? 癰귣똻???랁???덈뼄", "category": "fact", "importance": 0.8},
  {"content": "筌롫뗄???λ뜆釉?? ??湲???볥럢??以??臾믨쉐 ?醫륁깈", "category": "preference", "importance": 0.7}
]"#;

        let messages_json = json!([
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": format!("??쇱벉 ???遺? ?브쑴苑??곸㉭:\n\n{}", conversation)}
        ]);

        let response = self
            .llm
            .chat_completion_json(messages_json.as_array().unwrap().clone(), Some("gpt-4o-mini"))
            .await?;

        // JSON ???뼓
        let extracted: Vec<ExtractedMemory> = match serde_json::from_str(&response) {
            Ok(v) => v,
            Err(_) => {
                // ?臾먮뼗??獄쏄퀣肉???袁⑤빒 野껋럩??({"memories": [...]} ?? 筌ｌ꼶??
                if let Ok(wrapper) = serde_json::from_str::<serde_json::Value>(&response) {
                    if let Some(arr) = wrapper.get("memories").or(wrapper.get("items")).or(wrapper.get("data")) {
                        serde_json::from_value(arr.clone()).unwrap_or_default()
                    } else {
                        Vec::new()
                    }
                } else {
                    log::warn!("Failed to parse memory extraction response: {}", &response[..response.len().min(200)]);
                    Vec::new()
                }
            }
        };

        Ok(extracted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extracted_memory_deserialize() {
        let json = r#"[
            {"content": "???뮞??疫꿸퀣堉?, "category": "fact", "importance": 0.8},
            {"content": "?醫륁깈?????뮞??, "category": "preference", "importance": 0.5}
        ]"#;

        let memories: Vec<ExtractedMemory> = serde_json::from_str(json).unwrap();
        assert_eq!(memories.len(), 2);
        assert_eq!(memories[0].content, "???뮞??疫꿸퀣堉?);
        assert_eq!(memories[0].category, "fact");
        assert!((memories[0].importance - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_empty_extraction() {
        let json = "[]";
        let memories: Vec<ExtractedMemory> = serde_json::from_str(json).unwrap();
        assert!(memories.is_empty());
    }
}
