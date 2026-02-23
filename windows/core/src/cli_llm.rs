//! CLI-based LLM client for Steer Agent
//!
//! Supports Codex, Gemini, and Claude CLI tools for LLM execution.
//! Uses login-based authentication (no API keys required).

use anyhow::{anyhow, Result};
use log::{debug, warn};
use serde_json::{json, Value};
use std::process::Command;
use std::time::{Duration, Instant};

/// Supported CLI LLM providers
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LLMProvider {
    Codex,
    Gemini,
    Claude,
}

impl LLMProvider {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "codex" => Some(Self::Codex),
            "gemini" => Some(Self::Gemini),
            "claude" => Some(Self::Claude),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::Claude => "claude",
        }
    }
}

/// CLI LLM Client - executes prompts via CLI tools
pub struct CLILLMClient {
    provider: LLMProvider,
    #[allow(dead_code)]
    timeout_sec: u64,
    cwd: Option<String>,
}

impl CLILLMClient {
    fn codex_model() -> String {
        std::env::var("STEER_CLI_CODEX_MODEL")
            .or_else(|_| std::env::var("STEER_CODEX_MODEL"))
            .ok()
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| "gpt-5.3-codex-spark".to_string())
    }

    pub fn new(provider: LLMProvider) -> Self {
        let default_timeout = std::env::var("STEER_CLI_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(120);
        let provider_timeout_key = match provider {
            LLMProvider::Codex => "STEER_CLI_TIMEOUT_CODEX",
            LLMProvider::Gemini => "STEER_CLI_TIMEOUT_GEMINI",
            LLMProvider::Claude => "STEER_CLI_TIMEOUT_CLAUDE",
        };
        let timeout = std::env::var(provider_timeout_key)
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(default_timeout);

        Self {
            provider,
            timeout_sec: timeout,
            cwd: None,
        }
    }

    pub fn with_cwd(mut self, cwd: &str) -> Self {
        self.cwd = Some(cwd.to_string());
        self
    }

    /// Get provider from environment variable
    pub fn from_env() -> Option<Self> {
        println!("[CLI DEBUG] Checking STEER_CLI_LLM env var...");
        match std::env::var("STEER_CLI_LLM") {
            Ok(val) => {
                println!("[CLI DEBUG] Found STEER_CLI_LLM={}", val);
                let provider = LLMProvider::from_str(&val)?;
                let mut client = Self::new(provider);
                client.cwd = Some(std::env::temp_dir().to_string_lossy().to_string());
                Some(client)
            }
            Err(_) => {
                println!("[CLI DEBUG] STEER_CLI_LLM NOT set.");
                None
            }
        }
    }

    pub fn uses_stdin(&self) -> bool {
        matches!(self.provider, LLMProvider::Codex | LLMProvider::Claude)
    }

    /// Check if CLI is available
    pub fn check_version(&self) -> Result<String> {
        let cmd = match self.provider {
            LLMProvider::Codex => "codex",
            LLMProvider::Gemini => "gemini",
            LLMProvider::Claude => "claude",
        };

        let output = Command::new(cmd)
            .arg("--version")
            .output()
            .map_err(|e| anyhow!("{} CLI not found: {}", cmd, e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(anyhow!("{} CLI not configured or not logged in", cmd))
        }
    }

    /// Execute prompt via CLI and return JSON-like output
    pub fn execute(&self, prompt: &str) -> Result<String> {
        debug!("Preparing to execute CLI LLM...");
        let mut cmd = match self.provider {
            LLMProvider::Codex => std::process::Command::new("codex"),
            LLMProvider::Gemini => std::process::Command::new("gemini"),
            LLMProvider::Claude => std::process::Command::new("claude"),
        };

        let use_stdin = match self.provider {
            LLMProvider::Codex => {
                let model = Self::codex_model();
                cmd.args(&[
                    "exec",
                    "-m",
                    &model,
                    "--sandbox",
                    "danger-full-access",
                    "--skip-git-repo-check",
                    "--color",
                    "never",
                    "-",
                ]);
                true
            }
            LLMProvider::Gemini => {
                // Gemini CLI hangs on stdin pipe (Agent mode).
                cmd.arg("--sandbox");
                cmd.args(&["--output-format", "json"]);
                cmd.arg(prompt);
                false
            }
            LLMProvider::Claude => {
                cmd.args(&["--dangerously-skip-permissions", "-p", "-"]);
                true
            }
        };

        if let Some(cwd) = &self.cwd {
            if !matches!(self.provider, LLMProvider::Gemini) {
                debug!("Setting CWD to: {}", cwd);
                cmd.current_dir(cwd);
            }
        }

        if use_stdin {
            cmd.stdin(std::process::Stdio::piped());
        } else {
            cmd.stdin(std::process::Stdio::null());
        }

        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        debug!("Spawning command (Args: {} bytes)...", prompt.len());
        let mut child = cmd.spawn()?;

        if use_stdin {
            if let Some(mut stdin) = child.stdin.take() {
                debug!("Writing {} bytes to stdin...", prompt.len());
                use std::io::Write;
                stdin.write_all(prompt.as_bytes())?;
            }
        }

        debug!("Waiting for output (timeout={}s)...", self.timeout_sec);
        let output = self.wait_with_output_timeout(child)?;
        debug!("Exit Status: {}", output.status);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("CLI STDERR: {}", stderr);
            return Err(anyhow::anyhow!("CLI Error: {}", stderr));
        }

        let raw_output = String::from_utf8_lossy(&output.stdout).to_string();
        debug!("Raw Output: {:.200}...", raw_output);

        let provider_output = match self.provider {
            LLMProvider::Gemini => {
                Self::extract_gemini_response(&raw_output).unwrap_or_else(|| raw_output.clone())
            }
            _ => raw_output.clone(),
        };

        match Self::extract_json(&provider_output) {
            Some(json) => Ok(json),
            None => {
                warn!(
                    "Failed to extract JSON from output: {:.100}...",
                    provider_output
                );
                Err(anyhow::anyhow!(
                    "No valid JSON found in CLI output: {}",
                    provider_output
                ))
            }
        }
    }

    /// Execute prompt and return raw text output (no JSON extraction).
    pub fn execute_raw(&self, prompt: &str) -> Result<String> {
        debug!("Preparing to execute CLI LLM (raw mode)...");
        let mut cmd = match self.provider {
            LLMProvider::Codex => std::process::Command::new("codex"),
            LLMProvider::Gemini => std::process::Command::new("gemini"),
            LLMProvider::Claude => std::process::Command::new("claude"),
        };

        let use_stdin = match self.provider {
            LLMProvider::Codex => {
                let model = Self::codex_model();
                cmd.args(&[
                    "exec",
                    "-m",
                    &model,
                    "--sandbox",
                    "danger-full-access",
                    "--skip-git-repo-check",
                    "--color",
                    "never",
                    "-",
                ]);
                true
            }
            LLMProvider::Gemini => {
                cmd.arg("--sandbox");
                cmd.arg(prompt);
                false
            }
            LLMProvider::Claude => {
                cmd.args(&["--dangerously-skip-permissions", "-p", "-"]);
                true
            }
        };

        if let Some(cwd) = &self.cwd {
            if !matches!(self.provider, LLMProvider::Gemini) {
                cmd.current_dir(cwd);
            }
        }

        if use_stdin {
            cmd.stdin(std::process::Stdio::piped());
        } else {
            cmd.stdin(std::process::Stdio::null());
        }

        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn()?;

        if use_stdin {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(prompt.as_bytes())?;
            }
        }

        let output = self.wait_with_output_timeout(child)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("CLI Error (raw): {}", stderr));
        }

        let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let provider_output = match self.provider {
            LLMProvider::Gemini => {
                Self::extract_gemini_response(&raw).unwrap_or_else(|| raw.clone())
            }
            _ => raw,
        };
        Ok(provider_output)
    }

    fn extract_gemini_response(text: &str) -> Option<String> {
        let wrapper = Self::extract_json(text)?;
        let parsed = serde_json::from_str::<Value>(&wrapper).ok()?;
        let response = parsed.get("response")?;
        if let Some(s) = response.as_str() {
            return Some(s.to_string());
        }
        if response.is_object() || response.is_array() {
            return Some(response.to_string());
        }
        None
    }

    /// Extract the first valid JSON object or array from a string
    fn extract_json(text: &str) -> Option<String> {
        if let Some(start) = text.find("```json") {
            let content_start = start + 7;
            if let Some(end_offset) = text[content_start..].find("```") {
                let end = content_start + end_offset;
                let json_str = &text[content_start..end].trim();
                return Some(json_str.to_string());
            }
        }

        if let Some(start) = text.find('{') {
            if let Some(end) = text.rfind('}') {
                if end > start {
                    return Some(text[start..=end].to_string());
                }
            }
        }

        Some(text.to_string())
    }

    fn wait_with_output_timeout(
        &self,
        mut child: std::process::Child,
    ) -> Result<std::process::Output> {
        let start = Instant::now();
        let timeout = Duration::from_secs(self.timeout_sec.max(1));

        loop {
            if child.try_wait()?.is_some() {
                break;
            }
            if start.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(anyhow!(
                    "CLI timeout (provider={}, timeout={}s)",
                    self.provider.name(),
                    self.timeout_sec
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        Ok(child.wait_with_output()?)
    }

    /// Execute with base64 image for vision tasks
    pub fn execute_with_vision(&self, base64_image: &str, prompt: &str) -> Result<String> {
        let full_prompt = format!(
            "I'm showing you a screenshot (base64 encoded below). {}\n\n[Screenshot data: {} bytes]\n\nBase64 Image:\n{}",
            prompt,
            base64_image.len(),
            base64_image
        );

        self.execute(&full_prompt)
    }

    #[allow(dead_code)]
    fn build_command(&self) -> (String, Vec<String>) {
        match self.provider {
            LLMProvider::Codex => {
                let model = Self::codex_model();
                (
                    "codex".to_string(),
                    vec![
                        "exec".to_string(),
                        "-m".to_string(),
                        model,
                        "--sandbox".to_string(),
                        "danger-full-access".to_string(),
                        "--skip-git-repo-check".to_string(),
                        "--color".to_string(),
                        "never".to_string(),
                        "-".to_string(),
                    ],
                )
            }
            LLMProvider::Gemini => ("gemini".to_string(), vec!["-s".to_string()]),
            LLMProvider::Claude => (
                "claude".to_string(),
                vec![
                    "--dangerously-skip-permissions".to_string(),
                    "-p".to_string(),
                    "-".to_string(),
                ],
            ),
        }
    }
}

/// Convert OpenAI-format messages to a plain-text prompt for CLI tools
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

async fn local_ollama_chat_completion(messages: &[Value]) -> Result<String> {
    let model = std::env::var("STEER_LOCAL_LLM_MODEL")
        .or_else(|_| std::env::var("OLLAMA_MODEL"))
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "llama3".to_string());

    let prompt = messages_to_prompt(messages);
    let url = std::env::var("STEER_LOCAL_LLM_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "http://localhost:11434/api/generate".to_string());

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

    let response = client
        .post(&url)
        .json(&json!({
            "model": model,
            "prompt": prompt,
            "stream": false
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("local llama-server http error: {}", body));
    }

    let body: Value = response.json().await?;
    let content = body["response"]
        .as_str()
        .ok_or_else(|| anyhow!("local llama-server returned no response field"))?;
    Ok(content.trim().to_string())
}

/// Fallback chain: OpenAI failure -> Gemini CLI -> Codex CLI -> local llama-server
pub async fn fallback_chat_completion(messages: &[Value]) -> Result<String> {
    let prompt = messages_to_prompt(messages);

    let chain = std::env::var("STEER_LLM_FALLBACK_CHAIN")
        .unwrap_or_else(|_| "gemini,codex,local".to_string());
    let providers: Vec<&str> = chain.split(',').map(|s| s.trim()).collect();

    for provider_name in &providers {
        match *provider_name {
            "gemini" | "codex" | "claude" => {
                if let Some(llm_provider) = LLMProvider::from_str(provider_name) {
                    let temp_dir = std::env::temp_dir().to_string_lossy().to_string();
                    let client = CLILLMClient::new(llm_provider).with_cwd(&temp_dir);

                    if client.check_version().is_err() {
                        eprintln!(
                            "⚠️ [fallback] {} CLI not available, skipping",
                            provider_name
                        );
                        continue;
                    }

                    eprintln!("🔄 [fallback] Trying {} CLI...", provider_name);
                    match client.execute_raw(&prompt) {
                        Ok(response) if !response.trim().is_empty() => {
                            eprintln!(
                                "✅ [fallback] {} succeeded ({} chars)",
                                provider_name,
                                response.len()
                            );
                            return Ok(response);
                        }
                        Ok(_) => {
                            eprintln!(
                                "⚠️ [fallback] {} returned empty, trying next",
                                provider_name
                            );
                        }
                        Err(e) => {
                            eprintln!("⚠️ [fallback] {} failed: {}, trying next", provider_name, e);
                        }
                    }
                }
            }
            "local" => {
                eprintln!("🔄 [fallback] Trying local llama-server...");
                match local_ollama_chat_completion(messages).await {
                    Ok(response) if !response.trim().is_empty() => {
                        eprintln!(
                            "✅ [fallback] local llama-server succeeded ({} chars)",
                            response.len()
                        );
                        return Ok(response);
                    }
                    Ok(_) => {
                        eprintln!("⚠️ [fallback] local llama-server returned empty output");
                    }
                    Err(e) => {
                        eprintln!("⚠️ [fallback] local llama-server failed: {}", e);
                    }
                }
            }
            _ => {
                eprintln!("⚠️ [fallback] Unknown provider: {}", provider_name);
            }
        }
    }

    Err(anyhow!(
        "All LLM fallback providers failed. Chain: {}",
        chain
    ))
}

/// Convenience function for quick execution
pub fn execute_cli_llm(prompt: &str) -> Result<String> {
    let client = CLILLMClient::from_env().ok_or_else(|| {
        anyhow!("STEER_CLI_LLM not set. Use: export STEER_CLI_LLM=gemini|codex|claude")
    })?;
    client.execute(prompt)
}

/// Check if CLI LLM is configured and available
pub fn is_cli_llm_available() -> bool {
    CLILLMClient::from_env()
        .map(|c| c.check_version().is_ok())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_from_str() {
        assert_eq!(LLMProvider::from_str("gemini"), Some(LLMProvider::Gemini));
        assert_eq!(LLMProvider::from_str("CODEX"), Some(LLMProvider::Codex));
        assert_eq!(LLMProvider::from_str("Claude"), Some(LLMProvider::Claude));
        assert_eq!(LLMProvider::from_str("unknown"), None);
    }

    #[test]
    fn test_build_command() {
        let client = CLILLMClient::new(LLMProvider::Gemini);
        let (cmd, args) = client.build_command();
        assert_eq!(cmd, "gemini");
        assert!(args.contains(&"-s".to_string()));
    }

    #[test]
    fn test_messages_to_prompt() {
        let messages = vec![
            serde_json::json!({"role": "system", "content": "You are helpful."}),
            serde_json::json!({"role": "user", "content": "Hello!"}),
        ];
        let prompt = messages_to_prompt(&messages);
        assert!(prompt.contains("[System Instructions]"));
        assert!(prompt.contains("You are helpful."));
        assert!(prompt.contains("[User]"));
        assert!(prompt.contains("Hello!"));
    }
}
