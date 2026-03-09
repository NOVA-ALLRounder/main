use crate::controller::actions::{ActionRunner, NotionWriteResult};
use anyhow::{anyhow, Result};

impl ActionRunner {
    pub(in crate::controller::actions) async fn notion_write_text(
        title: &str,
        content: &str,
    ) -> Result<NotionWriteResult> {
        crate::load_env_with_fallback();
        let client = crate::integrations::notion::NotionClient::from_env()?;
        let title = if title.trim().is_empty() {
            "Steer Note".to_string()
        } else {
            title.trim().to_string()
        };
        let body = content.trim();
        if body.is_empty() {
            return Err(anyhow!("notion content is empty"));
        }

        let database_id = std::env::var("NOTION_DATABASE_ID")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_default();
        let page_id = std::env::var("NOTION_PAGE_ID")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_default();

        if !database_id.is_empty() {
            let blocks = Self::notion_paragraph_blocks(body);
            let created_page_id = if blocks.is_empty() {
                client.create_page(&database_id, &title, body).await?
            } else {
                client
                    .create_database_page_with_children(&database_id, &title, &blocks)
                    .await?
            };
            let page_url = format!("https://www.notion.so/{}", created_page_id.replace('-', ""));
            return Ok(NotionWriteResult {
                page_id: created_page_id,
                page_url,
                title,
            });
        }

        if page_id.is_empty() {
            return Err(anyhow!("NOTION_DATABASE_ID or NOTION_PAGE_ID not set"));
        }

        let blocks = Self::notion_paragraph_blocks(body);
        let effective_page_id = match client
            .create_child_page_with_children(&page_id, &title, &blocks)
            .await
        {
            Ok(created_child_id) => created_child_id,
            Err(child_err) => {
                let paragraphs = body
                    .lines()
                    .map(|line| line.trim().to_string())
                    .filter(|line| !line.is_empty())
                    .collect::<Vec<_>>();
                client
                    .append_paragraphs(&page_id, &paragraphs)
                    .await
                    .map_err(|append_err| {
                        anyhow!(
                            "failed to create child page ({}) and append page ({})",
                            child_err,
                            append_err
                        )
                    })?;
                page_id.clone()
            }
        };
        let page_url = format!(
            "https://www.notion.so/{}",
            effective_page_id.replace('-', "")
        );
        Ok(NotionWriteResult {
            page_id: effective_page_id,
            page_url,
            title,
        })
    }
}
