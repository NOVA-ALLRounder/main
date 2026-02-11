// Proactive Assistant - AI-driven suggestions and automation

use crate::jarvis::engines::*;
use crate::jarvis::tools::TelegramTool;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// Proactive Assistant - Provides intelligent suggestions based on context and patterns
pub struct ProactiveAssistant {
    context_engine: Arc<ContextEngine>,
    pattern_detector: Arc<PatternDetector>,
    workflow_builder: Arc<WorkflowBuilder>,
    telegram: Option<TelegramTool>,
    suggestions: Arc<RwLock<Vec<Suggestion>>>,
    config: AssistantConfig,
}

#[derive(Debug, Clone)]
struct AssistantConfig {
    suggestion_interval_secs: u64,
    min_confidence: f32,
    _max_suggestions_per_day: usize,
    enable_telegram_notifications: bool,
}

impl Default for AssistantConfig {
    fn default() -> Self {
        Self {
            suggestion_interval_secs: 300, // 5 minutes
            min_confidence: 0.7,
            _max_suggestions_per_day: 10,
            enable_telegram_notifications: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub id: String,
    pub suggestion_type: SuggestionType,
    pub title: String,
    pub description: String,
    pub confidence: f32,
    pub timestamp: u64,
    pub status: SuggestionStatus,
    pub action: SuggestedAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SuggestionType {
    Automation,      // Automate repetitive task
    Optimization,    // Improve workflow
    Reminder,        // Contextual reminder
    Learning,        // Learn new pattern
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SuggestionStatus {
    Pending,
    Accepted,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedAction {
    pub action_type: String,
    pub parameters: HashMap<String, String>,
}

impl ProactiveAssistant {
    pub async fn new(
        context_engine: Arc<ContextEngine>,
        pattern_detector: Arc<PatternDetector>,
        workflow_builder: Arc<WorkflowBuilder>,
    ) -> Result<Self> {
        let telegram = if std::env::var("TELEGRAM_BOT_TOKEN").is_ok() {
            Some(TelegramTool::new())
        } else {
            None
        };

        Ok(Self {
            context_engine,
            pattern_detector,
            workflow_builder,
            telegram,
            suggestions: Arc::new(RwLock::new(Vec::new())),
            config: AssistantConfig::default(),
        })
    }

    /// Start proactive monitoring and suggestion generation
    pub async fn start(&self) -> Result<()> {
        log::info!("Starting proactive assistant...");

        let context_engine = self.context_engine.clone();
        let pattern_detector = self.pattern_detector.clone();
        let workflow_builder = self.workflow_builder.clone();
        let suggestions = self.suggestions.clone();
        let config = self.config.clone();
        let has_telegram = self.telegram.is_some();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.suggestion_interval_secs));

            loop {
                interval.tick().await;

                // Check if user is active
                if !context_engine.is_user_active().await {
                    log::debug!("User idle, skipping suggestion generation");
                    continue;
                }

                // Generate suggestions
                if let Ok(new_suggestions) = Self::generate_suggestions_static(
                    &context_engine,
                    &pattern_detector,
                    &workflow_builder,
                    &config,
                )
                .await
                {
                    // Add to suggestions list
                    {
                        let mut suggs = suggestions.write().await;
                        for suggestion in new_suggestions {
                            // Check if suggestion already exists
                            if !suggs.iter().any(|s| s.id == suggestion.id) {
                                log::info!(
                                    "New suggestion: {} (confidence: {:.2})",
                                    suggestion.title,
                                    suggestion.confidence
                                );

                                // Send Telegram notification if enabled
                                if config.enable_telegram_notifications && has_telegram {
                                    // Notification will be sent separately
                                    log::debug!("Telegram notification enabled for suggestion");
                                }

                                suggs.push(suggestion);
                            }
                        }

                        // Clean up old suggestions (> 24 hours)
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        suggs.retain(|s| now - s.timestamp < 86400);
                    }
                }
            }
        });

        Ok(())
    }

    /// Generate suggestions based on current context and patterns
    async fn generate_suggestions_static(
        context_engine: &Arc<ContextEngine>,
        pattern_detector: &Arc<PatternDetector>,
        _workflow_builder: &Arc<WorkflowBuilder>,
        config: &AssistantConfig,
    ) -> Result<Vec<Suggestion>> {
        let mut suggestions = Vec::new();

        // Get current context
        let context = context_engine.get_context().await;

        // Get detected patterns
        let patterns = pattern_detector.get_patterns().await;

        // Generate automation suggestions from patterns
        for pattern in patterns {
            if pattern.confidence < config.min_confidence {
                continue;
            }

            // Create workflow suggestion
            let workflow_suggestion = Suggestion {
                id: format!("auto_{}", pattern.id),
                suggestion_type: SuggestionType::Automation,
                title: format!("Automate: {}", pattern.pattern_type_string()),
                description: pattern.automation_suggestion.clone(),
                confidence: pattern.confidence,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                status: SuggestionStatus::Pending,
                action: SuggestedAction {
                    action_type: "create_workflow".to_string(),
                    parameters: {
                        let mut params = HashMap::new();
                        params.insert("pattern_id".to_string(), pattern.id.clone());
                        params
                    },
                },
            };

            suggestions.push(workflow_suggestion);
        }

        // Generate context-based suggestions
        if let Some(app) = &context.active_app {
            // Suggest break if user has been active for too long
            if context.activity_count > 100 {
                suggestions.push(Suggestion {
                    id: format!("break_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()),
                    suggestion_type: SuggestionType::Reminder,
                    title: "Take a break".to_string(),
                    description: format!("You've been active in {} for a while. Consider taking a short break.", app),
                    confidence: 0.8,
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    status: SuggestionStatus::Pending,
                    action: SuggestedAction {
                        action_type: "reminder".to_string(),
                        parameters: HashMap::new(),
                    },
                });
            }
        }

        Ok(suggestions)
    }

    /// Send suggestion notification via Telegram
    #[allow(dead_code)]
    #[allow(deprecated)]
    async fn send_suggestion_notification(telegram: &TelegramTool, suggestion: &Suggestion) -> Result<()> {
        let message = format!(
            "?裕?*JARVIS Suggestion*\n\n*{}*\n\n{}\n\n_Confidence: {:.0}%_",
            suggestion.title,
            suggestion.description,
            suggestion.confidence * 100.0
        );

        use crate::jarvis::tools::tool_trait::{Tool, ToolParams};

        let mut params = ToolParams::new("send_message");
        params.params.insert("text".to_string(), serde_json::Value::String(message));

        telegram.execute(params).await?;

        Ok(())
    }

    /// Get all pending suggestions
    pub async fn get_pending_suggestions(&self) -> Vec<Suggestion> {
        self.suggestions
            .read()
            .await
            .iter()
            .filter(|s| s.status == SuggestionStatus::Pending)
            .cloned()
            .collect()
    }

    /// Get all suggestions
    pub async fn get_all_suggestions(&self) -> Vec<Suggestion> {
        self.suggestions.read().await.clone()
    }

    /// Accept a suggestion
    #[allow(deprecated)]
    pub async fn accept_suggestion(&self, suggestion_id: &str) -> Result<()> {
        let mut suggestions = self.suggestions.write().await;

        if let Some(suggestion) = suggestions.iter_mut().find(|s| s.id == suggestion_id) {
            suggestion.status = SuggestionStatus::Accepted;
            log::info!("Suggestion accepted: {}", suggestion.title);

            // Execute the suggested action
            match suggestion.action.action_type.as_str() {
                "create_workflow" => {
                    if let Some(pattern_id) = suggestion.action.parameters.get("pattern_id") {
                        // Get pattern and create workflow
                        let patterns = self.pattern_detector.get_patterns().await;
                        if let Some(pattern) = patterns.iter().find(|p| &p.id == pattern_id) {
                            let workflow = self
                                .workflow_builder
                                .create_from_pattern(pattern_id, &pattern.actions)
                                .await?;

                            log::info!("Workflow created: {}", workflow.name);

                            // Notify user
                            if let Some(telegram) = &self.telegram {
                                let message = format!(
                                    "??Workflow '{}' has been created!\n\nYou can now deploy it to n8n.",
                                    workflow.name
                                );

                                use crate::jarvis::tools::tool_trait::{Tool, ToolParams};
                                let mut params = ToolParams::new("send_message");
                                params.params.insert("text".to_string(), serde_json::Value::String(message));
                                let _ = telegram.execute(params).await;
                            }
                        }
                    }
                }
                _ => {
                    log::debug!("Action type not implemented: {}", suggestion.action.action_type);
                }
            }

            Ok(())
        } else {
            Err(anyhow::anyhow!("Suggestion not found: {}", suggestion_id))
        }
    }

    /// Reject a suggestion
    pub async fn reject_suggestion(&self, suggestion_id: &str) -> Result<()> {
        let mut suggestions = self.suggestions.write().await;

        if let Some(suggestion) = suggestions.iter_mut().find(|s| s.id == suggestion_id) {
            suggestion.status = SuggestionStatus::Rejected;
            log::info!("Suggestion rejected: {}", suggestion.title);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Suggestion not found: {}", suggestion_id))
        }
    }

    /// Get suggestion statistics
    pub async fn get_statistics(&self) -> SuggestionStatistics {
        let suggestions = self.suggestions.read().await;

        let total = suggestions.len();
        let pending = suggestions.iter().filter(|s| s.status == SuggestionStatus::Pending).count();
        let accepted = suggestions.iter().filter(|s| s.status == SuggestionStatus::Accepted).count();
        let rejected = suggestions.iter().filter(|s| s.status == SuggestionStatus::Rejected).count();

        let avg_confidence = if !suggestions.is_empty() {
            suggestions.iter().map(|s| s.confidence).sum::<f32>() / suggestions.len() as f32
        } else {
            0.0
        };

        SuggestionStatistics {
            total,
            pending,
            accepted,
            rejected,
            avg_confidence,
            acceptance_rate: if total > 0 {
                accepted as f32 / total as f32
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestionStatistics {
    pub total: usize,
    pub pending: usize,
    pub accepted: usize,
    pub rejected: usize,
    pub avg_confidence: f32,
    pub acceptance_rate: f32,
}

// Helper trait extension for pattern
trait PatternExt {
    fn pattern_type_string(&self) -> String;
}

impl PatternExt for pattern_detector::DetectedPattern {
    fn pattern_type_string(&self) -> String {
        format!("{:?}", self.pattern_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_proactive_assistant_creation() {
        let context = Arc::new(ContextEngine::new().await.unwrap());
        let pattern = Arc::new(PatternDetector::new().await.unwrap());
        let workflow = Arc::new(WorkflowBuilder::new().await.unwrap());

        let assistant = ProactiveAssistant::new(context, pattern, workflow)
            .await
            .unwrap();

        let suggestions = assistant.get_all_suggestions().await;
        assert_eq!(suggestions.len(), 0);
    }

    #[tokio::test]
    async fn test_suggestion_acceptance() {
        let context = Arc::new(ContextEngine::new().await.unwrap());
        let pattern = Arc::new(PatternDetector::new().await.unwrap());
        let workflow = Arc::new(WorkflowBuilder::new().await.unwrap());

        let assistant = ProactiveAssistant::new(context, pattern, workflow)
            .await
            .unwrap();

        // Manually add a suggestion
        {
            let mut suggestions = assistant.suggestions.write().await;
            suggestions.push(Suggestion {
                id: "test_1".to_string(),
                suggestion_type: SuggestionType::Automation,
                title: "Test".to_string(),
                description: "Test suggestion".to_string(),
                confidence: 0.9,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                status: SuggestionStatus::Pending,
                action: SuggestedAction {
                    action_type: "test".to_string(),
                    parameters: HashMap::new(),
                },
            });
        }

        // Accept suggestion
        assistant.accept_suggestion("test_1").await.unwrap();

        let suggestions = assistant.get_all_suggestions().await;
        assert_eq!(suggestions[0].status, SuggestionStatus::Accepted);
    }

    #[tokio::test]
    async fn test_suggestion_statistics() {
        let context = Arc::new(ContextEngine::new().await.unwrap());
        let pattern = Arc::new(PatternDetector::new().await.unwrap());
        let workflow = Arc::new(WorkflowBuilder::new().await.unwrap());

        let assistant = ProactiveAssistant::new(context, pattern, workflow)
            .await
            .unwrap();

        // Add test suggestions
        {
            let mut suggestions = assistant.suggestions.write().await;
            suggestions.push(Suggestion {
                id: "1".to_string(),
                suggestion_type: SuggestionType::Automation,
                title: "Test 1".to_string(),
                description: "Test".to_string(),
                confidence: 0.8,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                status: SuggestionStatus::Accepted,
                action: SuggestedAction {
                    action_type: "test".to_string(),
                    parameters: HashMap::new(),
                },
            });

            suggestions.push(Suggestion {
                id: "2".to_string(),
                suggestion_type: SuggestionType::Automation,
                title: "Test 2".to_string(),
                description: "Test".to_string(),
                confidence: 0.9,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                status: SuggestionStatus::Pending,
                action: SuggestedAction {
                    action_type: "test".to_string(),
                    parameters: HashMap::new(),
                },
            });
        }

        let stats = assistant.get_statistics().await;
        assert_eq!(stats.total, 2);
        assert_eq!(stats.accepted, 1);
        assert_eq!(stats.pending, 1);
        assert_eq!(stats.acceptance_rate, 0.5);
    }
}
