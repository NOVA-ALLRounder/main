use anyhow::{Context, Result};

use crate::schema::EventEnvelope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectorMode {
    Internal,
    Dcp,
}

pub fn collector_mode() -> CollectorMode {
    match std::env::var("STEER_COLLECTOR_MODE")
        .unwrap_or_else(|_| "internal".to_string())
        .to_lowercase()
        .as_str()
    {
        "dcp" | "data-collection-projection" => CollectorMode::Dcp,
        _ => CollectorMode::Internal,
    }
}

pub fn is_dcp_mode() -> bool {
    matches!(collector_mode(), CollectorMode::Dcp)
}

pub fn dcp_endpoint() -> String {
    std::env::var("STEER_DCP_ENDPOINT")
        .unwrap_or_else(|_| "http://127.0.0.1:8080/events".to_string())
}

pub async fn send_events(events: &[EventEnvelope]) -> Result<usize> {
    if events.is_empty() {
        return Ok(0);
    }

    let url = dcp_endpoint();
    let body = if events.len() == 1 {
        serde_json::to_value(&events[0])?
    } else {
        serde_json::to_value(events)?
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("Failed to reach DCP endpoint: {}", url))?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "DCP ingest failed ({}): {}",
            resp.status(),
            text
        ));
    }

    Ok(events.len())
}
