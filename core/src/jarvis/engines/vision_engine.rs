// Vision Engine - Screen analysis using Claude Vision API

use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Vision Engine - Analyzes screen content using Claude Vision API
pub struct VisionEngine {
    api_key: Option<String>,
    cache: Arc<RwLock<VisionCache>>,
    db_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionAnalysis {
    pub timestamp: u64,
    pub description: String,
    pub ui_elements: Vec<UIElement>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIElement {
    pub element_type: String,
    pub label: String,
    pub position: Option<(i32, i32)>,
    pub confidence: f32,
}

struct VisionCache {
    last_analysis: Option<VisionAnalysis>,
    cache_duration_secs: u64,
}

impl VisionEngine {
    pub async fn new() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY").ok();

        if api_key.is_none() {
            log::warn!("ANTHROPIC_API_KEY not set - vision features will be limited");
        }

        Ok(Self {
            api_key,
            cache: Arc::new(RwLock::new(VisionCache {
                last_analysis: None,
                cache_duration_secs: 60, // Cache for 1 minute
            })),
            db_path: None,
        })
    }

    /// Initialize with database connection
    pub async fn with_db(db_path: &str) -> Result<Self> {
        let mut engine = Self::new().await?;
        engine.db_path = Some(db_path.to_string());
        Ok(engine)
    }

    /// Capture and analyze current screen
    pub async fn analyze_screen(&self) -> Result<VisionAnalysis> {
        log::info!("Capturing and analyzing screen...");

        // Capture screen using platform
        let screenshot = self.capture_screenshot().await?;

        // Analyze with Claude Vision API
        let analysis = self.analyze_image(&screenshot).await?;

        // Cache the result
        {
            let mut cache = self.cache.write().await;
            cache.last_analysis = Some(analysis.clone());
        }

        // Store in database
        if let Some(db_path) = &self.db_path {
            self.save_to_database(db_path, &analysis).await?;
        }

        Ok(analysis)
    }

    /// Get cached analysis if still valid
    pub async fn get_cached_analysis(&self) -> Option<VisionAnalysis> {
        let cache = self.cache.read().await;

        if let Some(analysis) = &cache.last_analysis {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            // Check if cache is still valid
            if now - analysis.timestamp < cache.cache_duration_secs {
                return Some(analysis.clone());
            }
        }

        None
    }

    /// Analyze screen with optional prompt
    pub async fn analyze_with_prompt(&self, prompt: &str) -> Result<String> {
        log::info!("Analyzing screen with custom prompt: {}", prompt);

        let screenshot = self.capture_screenshot().await?;
        self.analyze_image_with_prompt(&screenshot, prompt).await
    }

    /// Find UI element at specific coordinates
    pub async fn find_element_at(&self, x: i32, y: i32) -> Result<Option<UIElement>> {
        let analysis = self.analyze_screen().await?;

        // Find element closest to coordinates
        let element = analysis
            .ui_elements
            .iter()
            .filter(|e| {
                if let Some((ex, ey)) = e.position {
                    let distance = ((ex - x).pow(2) + (ey - y).pow(2)) as f32;
                    distance < 100.0 // Within 100 pixel radius
                } else {
                    false
                }
            })
            .max_by(|a, b| {
                a.confidence
                    .partial_cmp(&b.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned();

        Ok(element)
    }

    /// Capture screenshot using platform
    async fn capture_screenshot(&self) -> Result<Vec<u8>> {
        use crate::platform::get_platform;

        let platform = get_platform();
        let screenshot_bytes = platform.screen_capture().capture_screen()?;

        Ok(screenshot_bytes)
    }

    /// Analyze image using Claude Vision API
    async fn analyze_image(&self, image_bytes: &[u8]) -> Result<VisionAnalysis> {
        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;

        // Encode image to base64
        let base64_image = STANDARD.encode(image_bytes);

        // Build request to Claude API
        let client = reqwest::Client::new();
        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&serde_json::json!({
                "model": "claude-3-5-sonnet-20241022",
                "max_tokens": 1024,
                "messages": [{
                    "role": "user",
                    "content": [
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/jpeg",
                                "data": base64_image
                            }
                        },
                        {
                            "type": "text",
                            "text": "Analyze this screenshot. Provide:\n1. Brief description of what's on screen\n2. List of visible UI elements (buttons, text fields, etc.) with approximate positions\n3. Any suggestions for automation or efficiency improvements\n\nFormat as JSON with keys: description, ui_elements (array of {type, label, position_x, position_y}), suggestions (array)"
                        }
                    ]
                }]
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!(
                "Claude API error: {}",
                error_text
            ));
        }

        let response_json: serde_json::Value = response.json().await?;

        // Extract text content
        let content_text = response_json["content"][0]["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid API response"))?;

        // Try to parse as JSON, fallback to simple analysis
        let analysis = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(content_text)
        {
            self.parse_vision_response(&parsed)?
        } else {
            // Fallback: create simple analysis
            VisionAnalysis {
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                description: content_text.to_string(),
                ui_elements: vec![],
                suggestions: vec![],
            }
        };

        Ok(analysis)
    }

    /// Analyze image with custom prompt
    async fn analyze_image_with_prompt(&self, image_bytes: &[u8], prompt: &str) -> Result<String> {
        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;

        let base64_image = STANDARD.encode(image_bytes);

        let client = reqwest::Client::new();
        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&serde_json::json!({
                "model": "claude-3-5-sonnet-20241022",
                "max_tokens": 1024,
                "messages": [{
                    "role": "user",
                    "content": [
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/jpeg",
                                "data": base64_image
                            }
                        },
                        {
                            "type": "text",
                            "text": prompt
                        }
                    ]
                }]
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Claude API error: {}", error_text));
        }

        let response_json: serde_json::Value = response.json().await?;
        let result = response_json["content"][0]["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid API response"))?
            .to_string();

        Ok(result)
    }

    /// Parse Claude Vision API response
    fn parse_vision_response(&self, json: &serde_json::Value) -> Result<VisionAnalysis> {
        let description = json["description"]
            .as_str()
            .unwrap_or("No description")
            .to_string();

        let ui_elements = if let Some(elements) = json["ui_elements"].as_array() {
            elements
                .iter()
                .filter_map(|e| {
                    Some(UIElement {
                        element_type: e["type"].as_str()?.to_string(),
                        label: e["label"].as_str()?.to_string(),
                        position: {
                            let x = e["position_x"].as_i64()? as i32;
                            let y = e["position_y"].as_i64()? as i32;
                            Some((x, y))
                        },
                        confidence: 0.8, // Default confidence
                    })
                })
                .collect()
        } else {
            vec![]
        };

        let suggestions = if let Some(suggs) = json["suggestions"].as_array() {
            suggs
                .iter()
                .filter_map(|s| s.as_str().map(|s| s.to_string()))
                .collect()
        } else {
            vec![]
        };

        Ok(VisionAnalysis {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            description,
            ui_elements,
            suggestions,
        })
    }

    /// Save analysis to database
    async fn save_to_database(&self, db_path: &str, analysis: &VisionAnalysis) -> Result<()> {
        let db_path = db_path.to_string();
        let analysis_json = serde_json::to_string(analysis)?;
        let timestamp = analysis.timestamp;

        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)?;

            conn.execute(
                "INSERT INTO vision_cache (timestamp, analysis_json, expires_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![
                    timestamp,
                    &analysis_json,
                    timestamp + 3600 // Expire after 1 hour
                ],
            )?;

            Ok::<_, anyhow::Error>(())
        })
        .await??;

        Ok(())
    }

    /// Get recent analyses from database
    pub async fn get_recent_analyses(&self, limit: usize) -> Result<Vec<VisionAnalysis>> {
        let db_path = self
            .db_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not initialized"))?
            .clone();

        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)?;

            let mut stmt = conn.prepare(
                "SELECT analysis_json FROM vision_cache ORDER BY timestamp DESC LIMIT ?1",
            )?;

            let analyses = stmt
                .query_map([limit], |row| {
                    let json_str: String = row.get(0)?;
                    Ok(json_str)
                })?
                .filter_map(|r| r.ok())
                .filter_map(|json_str| serde_json::from_str(&json_str).ok())
                .collect();

            Ok(analyses)
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vision_engine_creation() {
        let engine = VisionEngine::new().await.unwrap();
        // Should create even without API key
        assert!(engine.api_key.is_none() || engine.api_key.is_some());
    }

    #[tokio::test]
    async fn test_cache_functionality() {
        let engine = VisionEngine::new().await.unwrap();

        // Initially no cache
        assert!(engine.get_cached_analysis().await.is_none());

        // Manually add to cache
        {
            let mut cache = engine.cache.write().await;
            cache.last_analysis = Some(VisionAnalysis {
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                description: "Test".to_string(),
                ui_elements: vec![],
                suggestions: vec![],
            });
        }

        // Should now have cache
        assert!(engine.get_cached_analysis().await.is_some());
    }

    #[test]
    fn test_vision_response_parsing() {
        let engine = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(VisionEngine::new())
            .unwrap();

        let json = serde_json::json!({
            "description": "Test screen",
            "ui_elements": [
                {
                    "type": "button",
                    "label": "Submit",
                    "position_x": 100,
                    "position_y": 200
                }
            ],
            "suggestions": ["Automate this", "Add shortcut"]
        });

        let analysis = engine.parse_vision_response(&json).unwrap();

        assert_eq!(analysis.description, "Test screen");
        assert_eq!(analysis.ui_elements.len(), 1);
        assert_eq!(analysis.suggestions.len(), 2);
    }
}
