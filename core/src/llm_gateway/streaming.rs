use super::*;
use futures::StreamExt;

impl OpenAILLMClient {
    pub async fn chat_completion_stream<F>(
        &self,
        messages: Vec<Value>,
        mut on_chunk: F,
    ) -> Result<String>
    where
        F: FnMut(&str) + Send,
    {
        let body = json!({
            "model": self.model,
            "messages": messages,
            "stream": true
        });

        let response: reqwest::Response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Stream API Error: {}", error_text));
        }

        let mut full_content = String::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            let chunk_str = String::from_utf8_lossy(&chunk);

            for line in chunk_str.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        break;
                    }
                    if let Ok(parsed) = serde_json::from_str::<Value>(data) {
                        if let Some(delta) = parsed["choices"][0]["delta"]["content"].as_str() {
                            full_content.push_str(delta);
                            on_chunk(delta);
                        }
                    }
                }
            }
        }

        Ok(full_content)
    }
}
