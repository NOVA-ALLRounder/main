use super::{CLILLMClient, LLMProvider};
use anyhow::{anyhow, Result};
use serde_json::Value;

pub fn messages_to_prompt(messages: &[Value]) -> String {
    let mut parts = Vec::new();
    for msg in messages {
        let role = msg["role"].as_str().unwrap_or("user");
        let content = msg["content"].as_str().unwrap_or("");
        match role {
            "system" => parts.push(format!("[System Instructions]\n{}\n", content)),
            "user" => parts.push(format!("[User]\n{}\n", content)),
            "assistant" => parts.push(format!("[Assistant]\n{}\n", content)),
            _ => parts.push(format!("[{}]\n{}\n", role, content)),
        }
    }
    parts.join("\n")
}

pub fn execute_cli_llm(prompt: &str) -> Result<String> {
    let client = CLILLMClient::from_env().ok_or_else(|| {
        anyhow!("STEER_CLI_LLM not set. Use: export STEER_CLI_LLM=gemini|codex|claude")
    })?;
    client.execute(prompt)
}

pub fn is_cli_llm_available() -> bool {
    CLILLMClient::from_env()
        .map(|c| c.check_version().is_ok())
        .unwrap_or(false)
}

#[allow(dead_code)]
pub fn provider_name(provider: LLMProvider) -> &'static str {
    provider.name()
}
