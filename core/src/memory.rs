use anyhow::Result;
use crate::llm_gateway::LLMClient;
use lancedb::{connect, Connection};
use lancedb::query::{QueryBase, ExecutableQuery};
use futures::StreamExt;
use arrow::array::{FixedSizeListArray, StringArray, Float32Array, Array};
use arrow::record_batch::{RecordBatch, RecordBatchIterator};
use arrow::datatypes::{DataType, Field, Schema};
use std::sync::Arc;

pub struct MemoryStore {
    conn: Connection,
    table_name: String,
    llm: Arc<LLMClient>,
}

impl MemoryStore {
    pub async fn new(uri: &str, llm: Arc<LLMClient>) -> Result<Self> {
        let conn = connect(uri).execute().await?;
        
        Ok(Self {
            conn,
            table_name: "context_logs".to_string(),
            llm,
        })
    }

    #[allow(dead_code)]
    pub async fn init_table(&self) -> Result<()> {
        // Define Schema: id (utf8), text (utf8), vector (fixed_size_list<float32>[384]), metadata (utf8)
        // Note: For now, we rely on dynamic schema or explicit creation if not exists.
        // LanceDB often infers from data, but creating explicitly is safer.
        // However, lancedb-rs 0.4 might behave differently. 
        // We will try to create if not exists using a dummy empty batch or check existence.
        
        // Simplified: We'll assume the table is created on first insert if not present
        // or we check self.conn.open_table(name).
        Ok(())
    }

    pub async fn add(&self, text: &str, metadata: serde_json::Value) -> Result<()> {
        let vector = self.llm.get_embedding(text).await?;

        // Create Arrow Arrays
        // 1. Context Text
        let text_array = StringArray::from(vec![text]);
        
        // 2. Vector (FixedSizeList)
        // OpenAI embedding size is 1536
        let dim = vector.len() as i32;
        
        let values = Float32Array::from(vector.clone());
        let field = Field::new("item", DataType::Float32, true);
        let vector_array = FixedSizeListArray::new(
            Arc::new(field),
            dim, // Embedding size
            Arc::new(values),
            None,
        );

        // Create Arrow Arrays
        // ... (existing code checks out)

        // 3. Metadata
        let meta_str = metadata.to_string();
        let meta_array = StringArray::from(vec![meta_str]);

        // Schema
        let schema = Arc::new(Schema::new(vec![
            Field::new("text", DataType::Utf8, false),
            Field::new("vector", DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                dim
            ), false),
            Field::new("metadata", DataType::Utf8, true),
        ]));

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(text_array),
                Arc::new(vector_array),
                Arc::new(meta_array),
            ],
        )?;

        // Prepare data for potential create
        let batch_for_create = batch.clone();
        
        let _table = match self.conn.open_table(&self.table_name).execute().await {
            Ok(t) => {
                // Iterator for append
                let iterator = RecordBatchIterator::new(vec![Ok(batch)], schema.clone());
                t.add(iterator).execute().await?;
                t
            },
            Err(_) => {
                // Iterator for create
                let iterator = RecordBatchIterator::new(vec![Ok(batch_for_create)], schema);
                self.conn.create_table(&self.table_name, iterator).execute().await?
            }
        };

        Ok(())
    }
    
    #[allow(dead_code)]
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<(String, f64)>> {
        let vector = self.llm.get_embedding(query).await?;
        
        // Open Table
        let table = self.conn.open_table(&self.table_name).execute().await?;

        // Search
        let mut stream = table.query()
            .nearest_to(vector)?
            .limit(limit)
            .execute()
            .await?;
        
        let mut results = Vec::new();
        while let Some(batch_result) = stream.next().await {
            let batch = batch_result?;
            if let Some(text_col) = batch.column_by_name("text") {
                // Explicit cast to help inference
                let col: &dyn std::any::Any = text_col.as_any();
                if let Some(strings) = col.downcast_ref::<StringArray>() {
                   for i in 0..strings.len() {
                       results.push((strings.value(i).to_string(), 0.0));
                   }
                }
            }
        }
        
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use crate::llm_gateway::LLMClient;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_memory_functionality() {
        if std::env::var("OPENAI_API_KEY").is_err() {
            dotenv::dotenv().ok();
        }
        
        if std::env::var("OPENAI_API_KEY").is_err() {
             // Skip if no key (CI friendly)
            return;
        }

        let temp_dir = std::env::temp_dir().join("steer_test_mem");
        if temp_dir.exists() {
            std::fs::remove_dir_all(&temp_dir).unwrap_or(());
        }
        let uri = temp_dir.to_str().unwrap();

        let llm = Arc::new(LLMClient::new().unwrap());
        let store = MemoryStore::new(uri, llm).await.unwrap();
        
        // 1. Add
        store.add("Steer is an AI Agent", json!({"source": "manual"})).await.unwrap();
        
        // 2. Search
        let results = store.search("AI Agent", 1).await.unwrap();
        assert!(!results.is_empty());
        println!("Search Result: {:?}", results);
        assert!(results[0].0.contains("Steer"));
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
