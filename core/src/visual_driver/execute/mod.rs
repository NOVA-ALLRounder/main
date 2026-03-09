use super::*;

mod basic;
mod shortcut;
mod vision;

impl VisualDriver {
    pub async fn execute(&self, llm: Option<&dyn crate::llm_gateway::LLMClient>) -> Result<()> {
        info!("👻 [Smart Visual Driver] Starting Verified Automation...");

        for (i, step) in self.steps.iter().enumerate() {
            info!("   Step {}: {}", i + 1, step.description);

            if let Some(pre_prompt) = &step.pre_verify {
                if let Some(brain) = llm {
                    if !Self::verify_condition(brain, pre_prompt).await? {
                        if step.critical {
                            return Err(AppError::Execution(format!(
                                "❌ Pre-check failed: {}",
                                pre_prompt
                            ))
                            .into());
                        } else {
                            warn!("      ⚠️ Pre-check failed, but proceeding (non-critical).");
                        }
                    }
                }
            }

            match &step.action {
                UiAction::OpenUrl(url) => basic::execute_open_url(url)?,
                UiAction::Wait(secs) => basic::execute_wait(*secs).await,
                UiAction::Click(target) => basic::execute_click(step, target).await?,
                UiAction::Type(text) => basic::execute_type(text).await?,
                UiAction::Scroll(direction) => basic::execute_scroll(direction).await?,
                UiAction::ActivateApp(app) => basic::execute_activate_app(app).await?,
                UiAction::KeyboardShortcut(key, modifiers) => {
                    shortcut::execute_keyboard_shortcut(key, modifiers).await?
                }
                UiAction::ClickVisual(desc) => {
                    vision::execute_click_visual(self, step, desc, llm).await?
                }
            }

            if let Some(post_prompt) = &step.post_verify {
                if let Some(brain) = llm {
                    if !Self::wait_for_ui_settle(4000).await? {
                        if step.critical {
                            return Err(anyhow::anyhow!("❌ Post-action UI failed to settle."));
                        } else {
                            warn!(
                                "      ⚠️ UI unstable after action, but proceeding (non-critical)."
                            );
                        }
                    }

                    if !Self::verify_condition(brain, post_prompt).await? && step.critical {
                        return Err(anyhow::anyhow!("❌ Post-check failed: {}", post_prompt));
                    }
                }
            }
        }

        info!("👻 [Smart Visual Driver] Automation Complete.");
        Ok(())
    }
}
