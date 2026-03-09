use super::*;

pub(super) fn execute_open_url(url: &str) -> Result<()> {
    crate::applescript::open_url(url).map(|_| ())?;
    Ok(())
}

pub(super) async fn execute_wait(secs: u64) {
    tokio::time::sleep(tokio::time::Duration::from_secs(secs)).await;
}

pub(super) async fn execute_click(step: &SmartStep, target: &str) -> Result<()> {
    let target_clone = target.to_string();
    let script = format!(
        "tell application \"System Events\" to click button {:?} of window 1 of (first application process whose frontmost is true)",
        target_clone
    );

    if crate::env_flag("STEER_ADAPTIVE_POLLING") {
        info!("      ⏳ Adaptive Polling: Waiting for UI settle...");
        let _ = VisualDriver::wait_for_ui_settle(2000).await;
    }

    let task = tokio::task::spawn_blocking(move || applescript::run(&script));

    match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
        Ok(Ok(Ok(_))) => Ok(()),
        Ok(Ok(Err(e))) => {
            warn!("      (Click failed: {})", e);
            if step.critical {
                Err(AppError::Execution(format!("Critical Click Failed: {}", e)).into())
            } else {
                Ok(())
            }
        }
        Ok(Err(_)) => Err(anyhow::anyhow!("Task Panic")),
        Err(_) => {
            warn!("      (Click timed out)");
            if step.critical {
                Err(anyhow::anyhow!("Critical Click Timed Out"))
            } else {
                Ok(())
            }
        }
    }
}

pub(super) async fn execute_type(text: &str) -> Result<()> {
    let text_clone = text.to_string();
    let text_for_native_fallback = text.to_string();
    let compact: String = text_clone.chars().filter(|c| !c.is_whitespace()).collect();
    let calc_like = !compact.is_empty()
        && compact.chars().all(|c| {
            c.is_ascii_digit() || matches!(c, '+' | '-' | '*' | '/' | '=' | '.' | ',' | '(' | ')')
        });

    let task = tokio::task::spawn_blocking(move || {
        if calc_like {
            let script = format!(
                "tell application \"System Events\" to keystroke {:?}",
                text_clone
            );
            applescript::run(&script)
        } else {
            let lines = [
                "on run argv",
                "set targetText to item 1 of argv",
                "set oldClipboard to the clipboard",
                "set the clipboard to targetText",
                "tell application \"System Events\" to keystroke \"v\" using {command down}",
                "delay 0.35",
                "try",
                "set the clipboard to oldClipboard",
                "end try",
                "return \"ok\"",
                "end run",
            ];
            applescript::run_with_args(&lines, &[text_clone])
        }
    });

    match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
        Ok(Ok(Ok(_))) => Ok(()),
        Ok(Ok(Err(e))) => {
            #[cfg(target_os = "macos")]
            {
                let err_text = e.to_string();
                if should_fallback_to_native_type(&err_text) {
                    warn!("      (Type permission issue detected, trying native fallback)");
                    let fallback_text = text_for_native_fallback.clone();
                    let fallback = tokio::task::spawn_blocking(move || {
                        crate::macos::actions::type_text(&fallback_text)
                    });
                    match tokio::time::timeout(std::time::Duration::from_secs(5), fallback).await {
                        Ok(Ok(Ok(_))) => Ok(()),
                        Ok(Ok(Err(e2))) => Err(anyhow::anyhow!(
                            "Type Failed: {} | Native fallback failed: {}",
                            err_text,
                            e2
                        )),
                        Ok(Err(_)) => Err(anyhow::anyhow!(
                            "Type Failed: {} | Native fallback task panic",
                            err_text
                        )),
                        Err(_) => Err(anyhow::anyhow!(
                            "Type Failed: {} | Native fallback timed out",
                            err_text
                        )),
                    }
                } else {
                    Err(anyhow::anyhow!("Type Failed: {}", err_text))
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err(anyhow::anyhow!("Type Failed: {}", e))
            }
        }
        Ok(Err(_)) => Err(anyhow::anyhow!("Task Panic")),
        Err(_) => Err(anyhow::anyhow!("Type Timed Out")),
    }
}

pub(super) async fn execute_scroll(direction: &str) -> Result<()> {
    let dir = direction.to_lowercase();
    let key_code = if dir == "up" { 116 } else { 121 };
    let script = format!(
        "tell application \"System Events\" to key code {}",
        key_code
    );
    let task = tokio::task::spawn_blocking(move || applescript::run(&script));
    match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
        Ok(Ok(Ok(_))) => Ok(()),
        Ok(Ok(Err(e))) => Err(anyhow::anyhow!("Scroll Failed: {}", e)),
        Ok(Err(_)) => Err(anyhow::anyhow!("Task Panic")),
        Err(_) => Err(anyhow::anyhow!("Scroll Timed Out")),
    }
}

pub(super) async fn execute_activate_app(app: &str) -> Result<()> {
    let app_name = app.to_string();
    let task = tokio::task::spawn_blocking(move || {
        if app_name.to_lowercase() == "frontmost" {
            applescript::activate_frontmost_app()
        } else {
            applescript::activate_app(&app_name)
        }
    });
    match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
        Ok(Ok(Ok(_))) => Ok(()),
        Ok(Ok(Err(e))) => Err(anyhow::anyhow!("Activate Failed: {}", e)),
        Ok(Err(_)) => Err(anyhow::anyhow!("Task Panic")),
        Err(_) => Err(anyhow::anyhow!("Activate Timed Out")),
    }
}
