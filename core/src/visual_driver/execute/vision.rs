use super::*;

pub(super) async fn execute_click_visual(
    driver: &VisualDriver,
    step: &SmartStep,
    desc: &str,
    llm: Option<&dyn crate::llm_gateway::LLMClient>,
) -> Result<()> {
    info!("      👁️ Vision Click: Finding '{}'...", desc);
    if let Some(brain) = llm {
        let max_retries = 2;
        let find_timeout_ms = normalize_timeout_ms(
            std::env::var("STEER_CLICK_VISUAL_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok()),
            8000,
            30000,
        );
        let click_timeout_ms = normalize_timeout_ms(
            std::env::var("STEER_CLICK_EXEC_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok()),
            5000,
            15000,
        );
        for attempt in 0..=max_retries {
            if attempt > 0 {
                info!(
                    "      ⏳ Retry {}/{}: Re-observing screen...",
                    attempt, max_retries
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            } else if crate::env_flag("STEER_ADAPTIVE_POLLING") {
                info!("      ⏳ Adaptive Polling: Waiting for UI settle before Vision...");
                let _ = VisualDriver::wait_for_ui_settle(2000).await;
            }

            debug!("      📸 Capturing screen for Vision Click...");
            match VisualDriver::capture_screen() {
                Ok((b64, scale)) => {
                    debug!(
                        "      🔍 Calling find_element_coordinates (image size: {} bytes)...",
                        b64.len()
                    );
                    let coord_result = tokio::time::timeout(
                        tokio::time::Duration::from_millis(find_timeout_ms),
                        brain.find_element_coordinates(desc, &b64),
                    )
                    .await;

                    match coord_result {
                        Err(_) => {
                            warn!(
                                "      ⚠️ Vision coordinate lookup timed out after {}ms.",
                                find_timeout_ms
                            );
                            if attempt == max_retries && step.critical {
                                return Err(anyhow::anyhow!("Visual Click LLM timeout"));
                            }
                        }
                        Ok(Err(e)) => {
                            error!("      ⚠️ LLM Vision Error: {}", e);
                            if attempt == max_retries && step.critical {
                                return Err(anyhow::anyhow!("Visual Click LLM error"));
                            }
                        }
                        Ok(Ok(Some((x_raw, y_raw)))) => {
                            let x = (x_raw as f32 * scale) as i32;
                            let y = (y_raw as f32 * scale) as i32;
                            info!("      🎯 LLM Target: ({}, {}) [Scaled x{:.2}]", x, y, scale);

                            match crate::platform::current_platform().ui_element_center_at(x, y) {
                                Ok(Some((_sx, _sy))) => {
                                    info!(
                                        "      🧲 Grounded: Valid UI Element confirmed at ({}, {})",
                                        x, y
                                    );
                                }
                                Ok(None) => {
                                    warn!(
                                        "      ⚠️  Warning: No UI Element found at coordinates via platform adapter."
                                    );
                                }
                                Err(error) => {
                                    warn!(
                                        "      ⚠️  Warning: UI grounding probe failed: {}",
                                        error
                                    );
                                }
                            }

                            let click_task = tokio::task::spawn_blocking(move || {
                                crate::platform::current_platform().browser_click_at(x, y, false)
                            });
                            let click_result = tokio::time::timeout(
                                tokio::time::Duration::from_millis(click_timeout_ms),
                                click_task,
                            )
                            .await;

                            match click_result {
                                Ok(Ok(Ok(()))) => {
                                    info!("      ✅ Click executed successfully!");
                                    break;
                                }
                                Ok(Ok(Err(e))) => {
                                    error!("      ❌ Click visual script failed: {}", e);
                                    if attempt == max_retries && step.critical {
                                        return Err(anyhow::anyhow!(
                                            "Visual Click execution failed"
                                        ));
                                    }
                                }
                                Ok(Err(_)) => {
                                    error!("      ❌ Click visual task panicked.");
                                    if attempt == max_retries && step.critical {
                                        return Err(anyhow::anyhow!("Visual Click task panic"));
                                    }
                                }
                                Err(_) => {
                                    warn!(
                                        "      ⚠️ Click visual execution timed out after {}ms.",
                                        click_timeout_ms
                                    );
                                    if attempt == max_retries && step.critical {
                                        return Err(anyhow::anyhow!(
                                            "Visual Click execution timeout"
                                        ));
                                    }
                                }
                            }
                        }
                        Ok(Ok(None)) => {
                            warn!("      ⚠️ Element '{}' not found on screen.", desc);
                            if attempt == max_retries && step.critical {
                                return Err(anyhow::anyhow!(
                                    "Visual Element '{}' not found after retries",
                                    desc
                                ));
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("      ❌ Screen capture failed: {}", e);
                    if attempt == max_retries && step.critical {
                        return Err(anyhow::anyhow!("Screen capture failed"));
                    }
                }
            }
        }
    } else {
        warn!("      ⚠️ No LLM client provided for Visual Click.");
    }

    let _ = driver;
    Ok(())
}
