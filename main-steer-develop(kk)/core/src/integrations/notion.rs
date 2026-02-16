use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Deserialize)]
pub struct NotionPage {
    pub id: String,
    pub title: String,
    pub content: String,
}

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

    fn auth_headers(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req.header("Authorization", format!("Bearer {}", self.token))
            .header("Notion-Version", "2022-06-28")
    }

    async fn get_database_title_property_name(&self, database_id: &str) -> Result<String> {
        let url = format!("https://api.notion.com/v1/databases/{}", database_id);
        let resp = self.auth_headers(self.client.get(url)).send().await?;
        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_else(|_| "<no-body>".to_string());
            return Err(anyhow::anyhow!("Notion database retrieve error: {}", err));
        }
        let db_json: Value = resp.json().await?;
        let properties = db_json["properties"]
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("Notion database has no properties"))?;
        Self::extract_title_property_name(properties)
            .ok_or_else(|| anyhow::anyhow!("No title property found in Notion database"))
    }

    fn extract_title_property_name(properties: &Map<String, Value>) -> Option<String> {
        for (name, prop) in properties {
            if prop["type"].as_str() == Some("title") {
                return Some(name.clone());
            }
        }
        None
    }

    fn extract_page_title(page: &Value) -> String {
        let properties = match page["properties"].as_object() {
            Some(p) => p,
            None => return "Untitled".to_string(),
        };
        for (_, prop) in properties {
            if prop["type"].as_str() == Some("title") {
                if let Some(arr) = prop["title"].as_array() {
                    if let Some(first) = arr.first() {
                        if let Some(txt) = first["plain_text"].as_str() {
                            return txt.to_string();
                        }
                        if let Some(txt) = first["text"]["content"].as_str() {
                            return txt.to_string();
                        }
                    }
                }
            }
        }
        "Untitled".to_string()
    }

    /// Create a new page in a database.
    pub async fn create_page(&self, database_id: &str, title: &str, content: &str) -> Result<String> {
        let title_property = self.get_database_title_property_name(database_id).await?;
        let clipped: String = content.chars().take(1800).collect();
        let body = json!({
            "parent": { "database_id": database_id },
            "properties": {
                title_property: {
                    "title": [{ "text": { "content": title } }]
                }
            },
            "children": [
                {
                    "object": "block",
                    "type": "paragraph",
                    "paragraph": {
                        "rich_text": [{ "type": "text", "text": { "content": clipped } }]
                    }
                }
            ]
        });

        let resp = self
            .auth_headers(self.client.post("https://api.notion.com/v1/pages"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let err = resp.text().await?;
            return Err(anyhow::anyhow!("Notion API Error: {}", err));
        }

        let resp_json: Value = resp.json().await?;
        let page_id = resp_json["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Notion create response missing page id"))?
            .to_string();
        Ok(page_id)
    }

    /// Append a paragraph block to an existing page.
    pub async fn append_to_page(&self, page_id: &str, content: &str) -> Result<()> {
        let clipped: String = content.chars().take(1800).collect();
        let url = format!("https://api.notion.com/v1/blocks/{}/children", page_id);
        let body = json!({
            "children": [
                {
                    "object": "block",
                    "type": "paragraph",
                    "paragraph": {
                        "rich_text": [{ "type": "text", "text": { "content": clipped } }]
                    }
                }
            ]
        });
        let resp = self
            .auth_headers(self.client.patch(url))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_else(|_| "<no-body>".to_string());
            return Err(anyhow::anyhow!("Notion append error: {}", err));
        }
        Ok(())
    }

    /// Read a page's content by ID.
    pub async fn read_page(&self, page_id: &str) -> Result<NotionPage> {
        let page_url = format!("https://api.notion.com/v1/pages/{}", page_id);
        let page_resp = self.auth_headers(self.client.get(&page_url)).send().await?;

        if !page_resp.status().is_success() {
            let err = page_resp.text().await?;
            return Err(anyhow::anyhow!("Notion Page Error: {}", err));
        }

        let page_json: Value = page_resp.json().await?;
        let title = Self::extract_page_title(&page_json);

        let blocks_url = format!("https://api.notion.com/v1/blocks/{}/children", page_id);
        let blocks_resp = self.auth_headers(self.client.get(&blocks_url)).send().await?;

        let mut content = String::new();
        if blocks_resp.status().is_success() {
            let blocks_json: Value = blocks_resp.json().await?;
            if let Some(results) = blocks_json["results"].as_array() {
                for block in results {
                    if let Some(para) = block["paragraph"]["rich_text"].as_array() {
                        for text in para {
                            if let Some(t) = text["plain_text"].as_str() {
                                content.push_str(t);
                                content.push('\n');
                            } else if let Some(t) = text["text"]["content"].as_str() {
                                content.push_str(t);
                                content.push('\n');
                            }
                        }
                    }
                }
            }
        }

        Ok(NotionPage {
            id: page_id.to_string(),
            title,
            content,
        })
    }

    /// Query a database for pages.
    pub async fn query_database(&self, database_id: &str, limit: u32) -> Result<Vec<NotionPage>> {
        let url = format!("https://api.notion.com/v1/databases/{}/query", database_id);
        let body = json!({ "page_size": limit });

        let resp = self
            .auth_headers(self.client.post(&url))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let err = resp.text().await?;
            return Err(anyhow::anyhow!("Notion Query Error: {}", err));
        }

        let resp_json: Value = resp.json().await?;
        let mut pages = Vec::new();

        if let Some(results) = resp_json["results"].as_array() {
            for page in results {
                let id = page["id"].as_str().unwrap_or("").to_string();
                let title = Self::extract_page_title(page);

                pages.push(NotionPage {
                    id,
                    title,
                    content: String::new(),
                });
            }
        }

        Ok(pages)
    }
}
