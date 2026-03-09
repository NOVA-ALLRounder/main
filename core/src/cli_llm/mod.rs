//! CLI-based LLM client for Steer Agent
//!
//! Supports Codex, Gemini, and Claude CLI tools for LLM execution.
//! Uses login-based authentication (no API keys required).

use anyhow::Result;
use std::time::{Duration, Instant};

mod client;
mod fallback;
mod prompt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LLMProvider {
    Codex,
    Gemini,
    Claude,
}

impl LLMProvider {
    #[allow(clippy::should_implement_trait)]
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

pub struct CLILLMClient {
    pub(crate) provider: LLMProvider,
    #[allow(dead_code)]
    pub(crate) timeout_sec: u64,
    pub(crate) cwd: Option<String>,
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

    pub fn from_env() -> Option<Self> {
        println!("[CLI DEBUG] Checking STEER_CLI_LLM env var...");
        match std::env::var("STEER_CLI_LLM") {
            Ok(val) => {
                println!("[CLI DEBUG] Found STEER_CLI_LLM={}", val);
                let provider = LLMProvider::from_str(&val)?;
                let mut client = Self::new(provider);
                client.cwd = Some("/tmp".to_string());
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

    pub fn check_version(&self) -> Result<String> {
        client::check_version(self)
    }

    pub(crate) fn wait_with_output_timeout(
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
                return Err(anyhow::anyhow!(
                    "CLI timeout (provider={}, timeout={}s)",
                    self.provider.name(),
                    self.timeout_sec
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        Ok(child.wait_with_output()?)
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

pub use fallback::fallback_chat_completion;
pub use prompt::{execute_cli_llm, is_cli_llm_available, messages_to_prompt};

#[cfg(test)]
mod tests;
