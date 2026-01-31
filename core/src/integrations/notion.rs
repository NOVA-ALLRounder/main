use reqwest::Client;
use serde_json::json;
use anyhow::Result;

pub struct NotionClient {
    token: String,
    client: Client,
}

impl NotionClient {
    pub fn new(token: &str) -> Self {
        Self {
            token: token.to_string(),
            client: Client::new(),
        }
    }

    pub fn from_env() -> Result<Self> {
        dotenv::dotenv().ok();
        let token = std::env::var("NOTION_API_KEY")
            .map_err(|_| anyhow::anyhow!("NOTION_API_KEY not set"))?;
        Ok(Self::new(&token))
    }

    /// Create a new page in a database
    pub async fn create_page(&self, database_id: &str, title: &str, content: &str) -> Result<String> {
        let url = "https://api.notion.com/v1/pages";

        let body = json!({
            "parent": { "database_id": database_id },
            "properties": {
                "이름": {
                    "title": [{ "text": { "content": title } }]
                }
            },
            "children": [
                {
                    "object": "block",
                    "type": "paragraph",
                    "paragraph": {
                        "rich_text": [{ "type": "text", "text": { "content": content } }]
                    }
                }
            ]
        });

        let resp = self.client.post(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Notion-Version", "2022-06-28")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let err = resp.text().await?;
            return Err(anyhow::anyhow!("Notion API Error: {}", err));
        }

        let resp_json: serde_json::Value = resp.json().await?;
        let page_id = resp_json["id"].as_str().unwrap_or("unknown").to_string();
        Ok(page_id)
    }
}
