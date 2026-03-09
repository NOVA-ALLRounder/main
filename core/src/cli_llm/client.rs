use super::{CLILLMClient, LLMProvider};
use anyhow::{anyhow, Result};
use log::{debug, warn};
use serde_json::Value;
use std::process::{Command, Stdio};

pub(super) fn check_version(client: &CLILLMClient) -> Result<String> {
    let cmd = match client.provider {
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

impl CLILLMClient {
    pub fn execute(&self, prompt: &str) -> Result<String> {
        debug!("Preparing to execute CLI LLM...");
        let mut cmd = match self.provider {
            LLMProvider::Codex => Command::new("codex"),
            LLMProvider::Gemini => Command::new("gemini"),
            LLMProvider::Claude => Command::new("claude"),
        };

        let use_stdin = match self.provider {
            LLMProvider::Codex => {
                let model = Self::codex_model();
                cmd.args([
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
                cmd.args(["--output-format", "json"]);
                cmd.arg(prompt);
                false
            }
            LLMProvider::Claude => {
                cmd.args(["--dangerously-skip-permissions", "-p", "-"]);
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
            cmd.stdin(Stdio::piped());
        } else {
            cmd.stdin(Stdio::null());
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

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
            return Err(anyhow!("CLI Error: {}", stderr));
        }

        let raw_output = String::from_utf8_lossy(&output.stdout).to_string();
        debug!("Raw Output: {:.200}...", raw_output);

        let provider_output = match self.provider {
            LLMProvider::Gemini => {
                extract_gemini_response(&raw_output).unwrap_or_else(|| raw_output.clone())
            }
            _ => raw_output.clone(),
        };

        match extract_json(&provider_output) {
            Some(json) => Ok(json),
            None => {
                warn!(
                    "Failed to extract JSON from output: {:.100}...",
                    provider_output
                );
                Err(anyhow!(
                    "No valid JSON found in CLI output: {}",
                    provider_output
                ))
            }
        }
    }

    pub fn execute_raw(&self, prompt: &str) -> Result<String> {
        debug!("Preparing to execute CLI LLM (raw mode)...");
        let mut cmd = match self.provider {
            LLMProvider::Codex => Command::new("codex"),
            LLMProvider::Gemini => Command::new("gemini"),
            LLMProvider::Claude => Command::new("claude"),
        };

        let use_stdin = match self.provider {
            LLMProvider::Codex => {
                let model = Self::codex_model();
                cmd.args([
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
                cmd.args(["--dangerously-skip-permissions", "-p", "-"]);
                true
            }
        };

        if let Some(cwd) = &self.cwd {
            if !matches!(self.provider, LLMProvider::Gemini) {
                cmd.current_dir(cwd);
            }
        }

        if use_stdin {
            cmd.stdin(Stdio::piped());
        } else {
            cmd.stdin(Stdio::null());
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

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
            return Err(anyhow!("CLI Error (raw): {}", stderr));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn execute_with_vision(&self, base64_image: &str, prompt: &str) -> Result<String> {
        let full_prompt = format!(
            "I'm showing you a screenshot (base64 encoded below). {}\n\n[Screenshot data: {} bytes]\n\nBase64 Image:\n{}",
            prompt,
            base64_image.len(),
            base64_image
        );

        self.execute(&full_prompt)
    }
}

fn extract_gemini_response(text: &str) -> Option<String> {
    let wrapper = extract_json(text)?;
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
