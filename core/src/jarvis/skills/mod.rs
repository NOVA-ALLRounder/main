// Skills System - Dynamic tool registration with eligibility checks
// Phase 2: Convert tools ??skills

pub mod metadata;

// Concrete skill implementations
pub mod computer_use_skill;
pub mod email_skill;
pub mod telegram_skill;

// Phase 6: New skills ("????삘뀲 ?? - Digital Self)
pub mod calendar_skill;
pub mod llm_skill;
pub mod memory_skill;
pub mod note_skill;
pub mod web_fetch_skill;

// Phase 7: skills-main 筌〓㈇???類ㅼ삢 ??쎄텢
pub mod file_skill;
pub mod weather_skill;
pub mod search_skill;

// Re-exports
pub use computer_use_skill::ComputerUseSkill;
pub use email_skill::EmailSkill;
pub use telegram_skill::TelegramSkill;
pub use calendar_skill::CalendarSkill;
pub use llm_skill::LlmSkill;
pub use memory_skill::MemorySkill;
pub use note_skill::NoteSkill;
pub use web_fetch_skill::WebFetchSkill;
pub use file_skill::FileSkill;
pub use weather_skill::WeatherSkill;
pub use search_skill::SearchSkill;

use anyhow::Result;
use async_trait::async_trait;
use metadata::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Skill execution context
#[derive(Debug, Clone)]
pub struct SkillContext {
    pub session_key: String,
    pub action: String,
    pub params: HashMap<String, serde_json::Value>,
    pub user_context: crate::jarvis::models::context::UserContext,
}

/// Skill execution result
#[derive(Debug, Clone)]
pub struct SkillResult {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
    pub artifacts: Vec<String>, // Files created, URLs opened, etc.
}

impl SkillResult {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: None,
            artifacts: Vec::new(),
        }
    }

    pub fn success_with_data(message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: Some(data),
            artifacts: Vec::new(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
            artifacts: Vec::new(),
        }
    }
}

/// Skill trait - All skills must implement this
#[async_trait]
pub trait Skill: Send + Sync {
    /// Skill metadata (name, description, actions, requirements)
    fn metadata(&self) -> SkillMetadata;

    /// Check if skill can run in current environment
    fn check_eligibility(&self) -> EligibilityResult;

    /// Execute skill action
    async fn execute(&self, ctx: SkillContext) -> SkillResult;

    /// Does this action require approval?
    fn requires_approval(&self, _action: &str) -> bool {
        // Default: no approval required
        // Override for risky actions (send_email, delete_file, etc.)
        false
    }

    /// Cleanup resources (optional)
    async fn cleanup(&self) -> Result<()> {
        Ok(())
    }
}

/// Skill Registry - Manages all registered skills
pub struct SkillRegistry {
    skills: Arc<RwLock<HashMap<String, Arc<dyn Skill>>>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a skill
    pub async fn register(&self, skill: Arc<dyn Skill>) {
        let name = skill.metadata().name.clone();
        self.skills.write().await.insert(name.clone(), skill);
        log::info!("Registered skill: {}", name);
    }

    /// Get a skill by name
    pub async fn get(&self, name: &str) -> Option<Arc<dyn Skill>> {
        self.skills.read().await.get(name).cloned()
    }

    /// Execute a skill
    pub async fn execute(&self, name: &str, ctx: SkillContext) -> SkillResult {
        // Get skill
        let skill = match self.get(name).await {
            Some(s) => s,
            None => {
                return SkillResult::error(format!("Skill not found: {}", name));
            }
        };

        // Check eligibility
        let eligibility = skill.check_eligibility();
        if !eligibility.eligible {
            return SkillResult::error(format!(
                "Skill '{}' not available: {}",
                name,
                eligibility.reason.unwrap_or_else(|| "Unknown reason".to_string())
            ));
        }

        // Check approval requirement
        if skill.requires_approval(&ctx.action) {
            log::warn!(
                "Skill '{}' action '{}' requires approval (not implemented yet)",
                name,
                ctx.action
            );
            // TODO: Phase 4 - Integrate with approval_gate
        }

        // Execute
        skill.execute(ctx).await
    }

    /// List all registered skills
    pub async fn list(&self) -> Vec<SkillMetadata> {
        let skills = self.skills.read().await;
        skills
            .values()
            .map(|skill| skill.metadata())
            .collect()
    }

    /// List eligible skills (can run in current environment)
    pub async fn list_eligible(&self) -> Vec<SkillMetadata> {
        let skills = self.skills.read().await;
        skills
            .values()
            .filter(|skill| skill.check_eligibility().eligible)
            .map(|skill| skill.metadata())
            .collect()
    }

    /// Count registered skills
    pub async fn count(&self) -> usize {
        self.skills.read().await.len()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}
