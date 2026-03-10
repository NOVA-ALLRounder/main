use super::*;
use crate::platform::current_platform;

pub(super) fn execute_open_url(url: &str) -> Result<()> {
    current_platform().browser_navigate(url, None)?;
    Ok(())
}

pub(super) async fn execute_wait(secs: u64) {
    tokio::time::sleep(tokio::time::Duration::from_secs(secs)).await;
}

pub(super) async fn execute_click(step: &SmartStep, target: &str) -> Result<()> {
    let target_name = target.to_string();

    if crate::env_flag("STEER_ADAPTIVE_POLLING") {
        info!("      ⏳ Adaptive Polling: Waiting for UI settle...");
        let _ = VisualDriver::wait_for_ui_settle(2000).await;
    }

    let task = tokio::task::spawn_blocking(move || {
        let mut automation = crate::browser_automation::get_browser_automation();
        automation.take_snapshot()?;
        let ref_id = automation.find_by_name(&target_name).ok_or_else(|| {
            anyhow::anyhow!("Element '{}' not found in current UI snapshot", target_name)
        })?;
        automation.click_by_ref(&ref_id, false)
    });

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
    let task =
        tokio::task::spawn_blocking(move || current_platform().browser_type_text(&text_clone, 0));

    match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
        Ok(Ok(Ok(_))) => Ok(()),
        Ok(Ok(Err(e))) => Err(anyhow::anyhow!("Type Failed: {}", e)),
        Ok(Err(_)) => Err(anyhow::anyhow!("Task Panic")),
        Err(_) => Err(anyhow::anyhow!("Type Timed Out")),
    }
}

pub(super) async fn execute_scroll(direction: &str) -> Result<()> {
    let dir = direction.to_lowercase();
    let pixels = if dir == "up" { -480 } else { 480 };
    let task = tokio::task::spawn_blocking(move || current_platform().browser_scroll(pixels));
    match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
        Ok(Ok(Ok(_))) => Ok(()),
        Ok(Ok(Err(e))) => Err(anyhow::anyhow!("Scroll Failed: {}", e)),
        Ok(Err(_)) => Err(anyhow::anyhow!("Task Panic")),
        Err(_) => Err(anyhow::anyhow!("Scroll Timed Out")),
    }
}

pub(super) async fn execute_activate_app(app: &str) -> Result<()> {
    let app_name = app.to_string();
    let task =
        tokio::task::spawn_blocking(move || current_platform().activate_app_by_name(&app_name));
    match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
        Ok(Ok(Ok(_))) => Ok(()),
        Ok(Ok(Err(e))) => Err(anyhow::anyhow!("Activate Failed: {}", e)),
        Ok(Err(_)) => Err(anyhow::anyhow!("Task Panic")),
        Err(_) => Err(anyhow::anyhow!("Activate Timed Out")),
    }
}
