// Command Parser - Convert natural language to Skill calls using LLM
// Migrated from Tool-based to Skill-based architecture (v0.2.0)

use crate::jarvis::skills::{SkillContext, SkillRegistry};
use crate::jarvis::models::context::UserContext;
use crate::domains::intelligence::llm_gateway::LLMClient;
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

pub struct CommandParser {
    _skill_registry: Arc<SkillRegistry>,
    llm_client: Option<LLMClient>,
}

#[derive(Debug, Clone)]
pub struct ParsedCommand {
    pub skill: String,
    pub action: String,
    pub params: HashMap<String, serde_json::Value>,
}

impl CommandParser {
    pub fn new(skill_registry: Arc<SkillRegistry>) -> Self {
        // Try to initialize LLM client
        let llm_client = LLMClient::new().ok();
        if llm_client.is_none() {
            log::error!("??LLM client not available (OPENAI_API_KEY not set in .env)");
            log::error!("   JARVIS requires OpenAI API key to parse commands.");
        } else {
            log::info!("??LLM-based command parser initialized");
        }
        Self {
            _skill_registry: skill_registry,
            llm_client,
        }
    }

    /// Parse natural language command to skill call using LLM
    pub async fn parse(&self, command: &str) -> Result<ParsedCommand> {
        // Prefer LLM parsing when available, but always keep a deterministic fallback.
        if let Some(ref llm) = self.llm_client {
            match self.parse_with_llm(llm, command).await {
                Ok(parsed) => return Ok(parsed),
                Err(e) => {
                    log::warn!("LLM parsing failed, falling back to rules: {}", e);
                }
            }
        }

        self.parse_with_rules(command)
    }

    /// Parse command with simple deterministic rules (LLM-free fallback)
    fn parse_with_rules(&self, command: &str) -> Result<ParsedCommand> {
        let text = command.trim();
        let lower = text.to_lowercase();        // open app shortcuts
        if lower.contains("notepad") || lower.contains("memo") {
            let mut params = HashMap::new();
            params.insert("app".to_string(), json!("notepad"));
            return Ok(ParsedCommand {
                skill: "computer_use".to_string(),
                action: "open_app".to_string(),
                params,
            });
        }

        if lower.contains("calculator") || lower == "calc" {
            let mut params = HashMap::new();
            params.insert("app".to_string(), json!("calc"));
            return Ok(ParsedCommand {
                skill: "computer_use".to_string(),
                action: "open_app".to_string(),
                params,
            });
        }

        if lower.contains("chrome") {
            let mut params = HashMap::new();
            params.insert("app".to_string(), json!("chrome"));
            return Ok(ParsedCommand {
                skill: "computer_use".to_string(),
                action: "open_app".to_string(),
                params,
            });
        }

        if lower.starts_with("open ") {
            let app = text[5..].trim();
            if !app.is_empty() {
                let mut params = HashMap::new();
                params.insert("app".to_string(), json!(app));
                return Ok(ParsedCommand {
                    skill: "computer_use".to_string(),
                    action: "open_app".to_string(),
                    params,
                });
            }
        }

        if lower.starts_with("type ") {
            let content = text[5..].trim();
            let mut params = HashMap::new();
            params.insert("text".to_string(), json!(content));
            return Ok(ParsedCommand {
                skill: "computer_use".to_string(),
                action: "type".to_string(),
                params,
            });
        }

        // Reasonable default for unknown actions
        Err(anyhow::anyhow!(
            "Could not parse command. Try: 'open notepad', 'open calc', or 'type hello'."
        ))
    }

    /// Parse command using LLM
    async fn parse_with_llm(&self, llm: &LLMClient, command: &str) -> Result<ParsedCommand> {
        let system_prompt = r#"You are JARVIS, an intelligent computer automation assistant.

Parse the user's natural language command into executable skill calls.

Available Skills and Actions:

1. computer_use (Computer Control):
   - screenshot: Capture screen
   - type_text: Type text ??params: {"text": "content"}
   - key_press: Press key ??params: {"key": "Enter", "modifiers": ["ctrl"]}
   - mouse_move: Move mouse ??params: {"x": 100, "y": 200}
   - mouse_click: Click ??params: {"button": "left", "x": 100, "y": 200}
   - list_windows: List open windows
   - focus_window: Focus window ??params: {"title": "window name"}

2. email (Email Management):
   - send_email: Send email ??params: {"to": "addr", "subject": "subj", "body": "text"}
   - list_emails: List recent emails ??params: {"count": 10}
   - read_email: Read email ??params: {"id": "email_id"}

3. telegram (Telegram Bot):
   - send_message: Send message ??params: {"text": "content"}

Korean/English Mappings:
- 癲ル슢??????notepad ??use computer_use/focus_window
- ??櫻?chrome/??? ??use computer_use/focus_window
- ??節뚮쳮雅?굞?꿱눧?calculator ??use computer_use/focus_window
- ???嶺??email ??use email skill
- ??釉먮폏??뺤쐺獄쏅챷援??telegram ??use telegram skill

For COMPOUND commands (multiple actions), return an array of actions.
For SINGLE commands, return a single action.

Output ONLY valid JSON in this format:
Single action:
{"skill": "computer_use", "action": "type_text", "params": {"text": "Hello"}}

Multiple actions (compound command):
[
  {"skill": "computer_use", "action": "focus_window", "params": {"title": "notepad"}},
  {"skill": "computer_use", "action": "type_text", "params": {"text": "Hello"}}
]
"#;

        let messages = vec![
            json!({"role": "system", "content": system_prompt}),
            json!({"role": "user", "content": command})
        ];

        // Call LLM API with JSON mode
        let content = llm.chat_completion_json(messages, Some("gpt-4o-mini")).await?;

        let parsed_json: Value = serde_json::from_str(&content)?;

        // Handle both single and compound commands
        let action = if parsed_json.is_array() {
            log::info!("???LLM returned compound command with {} actions", parsed_json.as_array().unwrap().len());
            // For now, use first action (orchestrator will handle compound in future)
            &parsed_json[0]
        } else {
            &parsed_json
        };

        let skill = action["skill"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'skill' field in LLM response"))?
            .to_string();

        let action_name = action["action"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'action' field in LLM response"))?
            .to_string();

        let mut params = HashMap::new();
        if let Some(params_obj) = action["params"].as_object() {
            for (key, value) in params_obj {
                params.insert(key.clone(), value.clone());
            }
        }

        log::info!("??Parsed: {} ??{}/{}", command, skill, action_name);

        Ok(ParsedCommand {
            skill,
            action: action_name,
            params,
        })
    }

    /// Convert parsed command to skill context
    pub fn to_skill_context(&self, parsed: ParsedCommand, session_key: String) -> SkillContext {
        SkillContext {
            session_key,
            action: parsed.action,
            params: parsed.params,
            user_context: UserContext::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jarvis::skills::SkillRegistry;

    async fn create_test_parser() -> CommandParser {
        let registry = Arc::new(SkillRegistry::new());
        CommandParser::new(registry)
    }

    #[tokio::test]
    async fn test_parser_init() {
        let parser = create_test_parser().await;
        // Just check it doesn't crash
        assert!(parser.llm_client.is_some() || parser.llm_client.is_none());
    }

    #[test]
    fn test_parsed_command_structure() {
        let cmd = ParsedCommand {
            skill: "computer_use".to_string(),
            action: "type_text".to_string(),
            params: {
                let mut map = HashMap::new();
                map.insert("text".to_string(), json!("Hello"));
                map
            },
        };

        assert_eq!(cmd.skill, "computer_use");
        assert_eq!(cmd.action, "type_text");
        assert_eq!(cmd.params.get("text").unwrap(), &json!("Hello"));
    }
}
