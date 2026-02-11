// Memory System - "?얜똻毓??疫꿸퀣堉??롫뮉揶쎛" (openclaw Vector Memory Store pattern)
//
// 甕겸돧苑??袁⑥퓢??疫꿸퀡而???뺛렎??野꺜??+ ??쇱뜖??野꺜????륁뵠?됰슢???獄쎻뫗??
// LLMClient::get_embedding()????沅??븍릭??OpenAI embeddings ??밴쉐

use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::domains::intelligence::llm_gateway::LLMClient;

/// 疫꿸퀣堉?????
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: i64,
    pub content: String,
    pub category: String, // fact, preference, event, conversation
    pub source: String,   // user_edit, auto_extract, explicit_save
    pub importance: f64,
    pub created_at: String,
    pub access_count: i64,
    pub score: f64, // 野꺜?????醫롪텢???癒?땾
}

/// Memory Store - 甕겸돧苑?疫꿸퀡而??觀由?疫꿸퀣堉?
pub struct MemoryStore {
    llm: Option<Arc<LLMClient>>,
}

impl MemoryStore {
    pub fn new(llm: Option<Arc<LLMClient>>) -> Self {
        Self { llm }
    }

    /// 疫꿸퀣堉?????(??용뮞?????袁⑥퓢????DB)
    pub async fn store(
        &self,
        content: &str,
        category: &str,
        source: &str,
        importance: f64,
    ) -> Result<i64> {
        // ?袁⑥퓢????밴쉐 (LLM ????揶쎛?館釉?野껋럩??
        let embedding = if let Some(ref llm) = self.llm {
            match llm.get_embedding(content).await {
                Ok(vec) => Some(vec),
                Err(e) => {
                    log::warn!("Failed to generate embedding: {}. Storing without vector.", e);
                    None
                }
            }
        } else {
            None
        };

        let embedding_blob = embedding.map(|v| serialize_embedding(&v));
        let created_at = chrono::Utc::now().to_rfc3339();

        let conn = crate::infrastructure::database::get_db_connection()?;
        conn.execute(
            "INSERT INTO jarvis_vector_memories (content, category, source, embedding, importance, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![content, category, source, embedding_blob, importance, created_at],
        )?;

        let id = conn.last_insert_rowid();
        log::debug!("Stored memory #{}: [{}] {}", id, category, &content[..content.len().min(50)]);
        Ok(id)
    }

    /// ??뺛렎??野꺜??(甕겸돧苑??醫롪텢??疫꿸퀡而?
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<Memory>> {
        let query_embedding = if let Some(ref llm) = self.llm {
            Some(llm.get_embedding(query).await?)
        } else {
            return self.search_keyword(query, limit);
        };

        let conn = crate::infrastructure::database::get_db_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, content, category, source, embedding, importance, created_at, access_count
             FROM jarvis_vector_memories
             WHERE embedding IS NOT NULL"
        )?;

        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let content: String = row.get(1)?;
            let category: String = row.get(2)?;
            let source: String = row.get(3)?;
            let embedding_blob: Vec<u8> = row.get(4)?;
            let importance: f64 = row.get(5)?;
            let created_at: String = row.get(6)?;
            let access_count: i64 = row.get(7)?;

            Ok((id, content, category, source, embedding_blob, importance, created_at, access_count))
        })?;

        let query_vec = query_embedding.unwrap();
        let mut scored: Vec<Memory> = Vec::new();

        for row in rows {
            let (id, content, category, source, embedding_blob, importance, created_at, access_count) = row?;
            let stored_vec = deserialize_embedding(&embedding_blob);

            let similarity = cosine_similarity(&query_vec, &stored_vec);

            if similarity > 0.3 {
                scored.push(Memory {
                    id,
                    content,
                    category,
                    source,
                    importance,
                    created_at,
                    access_count,
                    score: similarity as f64,
                });
            }
        }

        // ?醫롪텢?????앾㎕?λ떄 ?類ｌ졊
        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);

        // ?臾롫젏 ??쏅땾 ??낅쑓??꾨뱜
        for mem in &scored {
            let _ = conn.execute(
                "UPDATE jarvis_vector_memories SET access_count = access_count + 1, last_accessed = ?1 WHERE id = ?2",
                params![chrono::Utc::now().to_rfc3339(), mem.id],
            );
        }

        Ok(scored)
    }

    /// ??쇱뜖??野꺜??(LIKE 疫꿸퀡而?fallback)
    pub fn search_keyword(&self, keyword: &str, limit: usize) -> Result<Vec<Memory>> {
        let conn = crate::infrastructure::database::get_db_connection()?;
        let pattern = format!("%{}%", keyword);
        let mut stmt = conn.prepare(
            "SELECT id, content, category, source, importance, created_at, access_count
             FROM jarvis_vector_memories
             WHERE content LIKE ?1
             ORDER BY importance DESC, created_at DESC
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![pattern, limit as i64], |row| {
            Ok(Memory {
                id: row.get(0)?,
                content: row.get(1)?,
                category: row.get(2)?,
                source: row.get(3)?,
                importance: row.get(4)?,
                created_at: row.get(5)?,
                access_count: row.get(6)?,
                score: 1.0, // ??쇱뜖??筌띲끉臾?? score 1.0
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// ??륁뵠?됰슢???野꺜??(甕겸돧苑?70% + ??쇱뜖??30%)
    pub async fn search_hybrid(&self, query: &str, limit: usize) -> Result<Vec<Memory>> {
        let vector_results = self.search(query, limit * 2).await.unwrap_or_default();
        let keyword_results = self.search_keyword(query, limit * 2).unwrap_or_default();

        let mut combined: std::collections::HashMap<i64, Memory> = std::collections::HashMap::new();

        // 甕겸돧苑?野껉퀗??(70% 揶쎛餓λ쵐??
        for mut mem in vector_results {
            mem.score *= 0.7;
            combined.insert(mem.id, mem);
        }

        // ??쇱뜖??野껉퀗??(30% 揶쎛餓λ쵐??
        for mem in keyword_results {
            combined
                .entry(mem.id)
                .and_modify(|existing| existing.score += 0.3)
                .or_insert_with(|| Memory { score: 0.3, ..mem });
        }

        let mut results: Vec<Memory> = combined.into_values().collect();
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }

    /// 疫꿸퀣堉?????
    pub fn forget(&self, memory_id: i64) -> Result<()> {
        let conn = crate::infrastructure::database::get_db_connection()?;
        conn.execute("DELETE FROM jarvis_vector_memories WHERE id = ?1", params![memory_id])?;
        Ok(())
    }

    /// 疫꿸퀣堉?揶쏆뮇??
    pub fn count(&self) -> Result<i64> {
        let conn = crate::infrastructure::database::get_db_connection()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM jarvis_vector_memories",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// 餓λ쵐???疫꿸퀡而??類ｂ봺 (??살삋????餓λ쵐???疫꿸퀣堉???볤탢)
    pub fn prune(&self, max_count: usize) -> Result<usize> {
        let count = self.count()? as usize;
        if count <= max_count {
            return Ok(0);
        }

        let to_remove = count - max_count;
        let conn = crate::infrastructure::database::get_db_connection()?;
        conn.execute(
            "DELETE FROM jarvis_vector_memories WHERE id IN (
                SELECT id FROM jarvis_vector_memories
                ORDER BY importance ASC, access_count ASC, created_at ASC
                LIMIT ?1
            )",
            params![to_remove as i64],
        )?;

        log::info!("Pruned {} low-importance memories", to_remove);
        Ok(to_remove)
    }
}

/// f32 甕겸돧苑ｇ몴?獄쏅뗄???筌욊낮???
fn serialize_embedding(vec: &[f32]) -> Vec<u8> {
    vec.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// 獄쏅뗄??紐꾨퓠??f32 甕겸돧苑?癰귣벊??
fn deserialize_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// ?꾨뗄沅???醫롪텢???④쑴沅?(??뽯땾 Rust)
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_embedding_serialization_roundtrip() {
        let original = vec![1.5_f32, -2.3, 0.0, 100.0, -0.001];
        let bytes = serialize_embedding(&original);
        let restored = deserialize_embedding(&bytes);
        assert_eq!(original.len(), restored.len());
        for (a, b) in original.iter().zip(restored.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let sim = cosine_similarity(&[], &[]);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_mismatched_lengths() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }
}
