use super::{messages_to_prompt, CLILLMClient, LLMProvider};
use anyhow::{anyhow, Result};
use serde_json::Value;

pub async fn fallback_chat_completion(messages: &[Value]) -> Result<String> {
    let prompt = messages_to_prompt(messages);

    let chain = std::env::var("STEER_LLM_FALLBACK_CHAIN")
        .unwrap_or_else(|_| "gemini,codex,local".to_string());

    let providers: Vec<&str> = chain.split(',').map(|s| s.trim()).collect();

    for provider_name in &providers {
        match *provider_name {
            "gemini" | "codex" | "claude" => {
                if let Some(llm_provider) = LLMProvider::from_str(provider_name) {
                    let mut client = CLILLMClient::new(llm_provider);
                    client.cwd = Some("/tmp".to_string());

                    if client.check_version().is_err() {
                        eprintln!(
                            "⚠️ [fallback] {} CLI not available, skipping",
                            provider_name
                        );
                        continue;
                    }

                    eprintln!("🔄 [fallback] Trying {} CLI...", provider_name);
                    match client.execute_raw(&prompt) {
                        Ok(response) if !response.is_empty() => {
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
                if !crate::llama_local::ensure_running().await {
                    eprintln!("⚠️ [fallback] llama-server not available, skipping");
                    continue;
                }
                match crate::llama_local::chat_completion(messages).await {
                    Ok(response) => {
                        eprintln!(
                            "✅ [fallback] local llama-server succeeded ({} chars)",
                            response.len()
                        );
                        return Ok(response);
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
