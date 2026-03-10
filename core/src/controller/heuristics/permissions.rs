use crate::applescript;
use anyhow::Result;

pub fn permission_help() -> &'static str {
    "Enable Screen Recording + UI automation for Terminal/Codex (System Settings > Privacy & Security). If prompts disappear, try `tccutil reset Accessibility` and `tccutil reset ScreenCapture` then relaunch the app."
}

pub fn preflight_permissions() -> Result<()> {
    if crate::peekaboo_cli::is_available() {
        if let Ok(perms) = crate::peekaboo_cli::check_permissions() {
            if perms.screen_recording == Some(false) {
                return Err(anyhow::anyhow!(
                    "Screen Recording permission missing (Peekaboo). {}",
                    permission_help()
                ));
            }
            if perms.accessibility == Some(false) {
                return Err(anyhow::anyhow!(
                    "UI automation permission missing (Peekaboo). {}",
                    permission_help()
                ));
            }
        }
    }

    if let Err(e) = applescript::check_accessibility() {
        return Err(anyhow::anyhow!(
            "UI automation permission check failed: {}. {}",
            e,
            permission_help()
        ));
    }

    Ok(())
}

pub fn verify_screen_capture() -> Result<()> {
    if !env_truthy_default("STEER_PREFLIGHT_SCREEN_CAPTURE", true) {
        let skip_allowed = env_truthy_default("STEER_TEST_MODE", false)
            || env_truthy_default("STEER_ALLOW_SCREEN_CAPTURE_SKIP", false);
        if skip_allowed {
            return Ok(());
        }
        return Err(anyhow::anyhow!(
            "Screen capture preflight disabled by env (STEER_PREFLIGHT_SCREEN_CAPTURE=0) in non-test mode. {}",
            permission_help()
        ));
    }

    let native_granted = crate::permission_manager::PermissionManager::check_screen_recording();
    let shot_path = format!(
        "/tmp/steer_preflight_capture_{}_{}.png",
        std::process::id(),
        chrono::Utc::now().timestamp_millis()
    );

    let probe_status = std::process::Command::new("screencapture")
        .args(["-x", shot_path.as_str()])
        .status();

    let exists = std::fs::metadata(&shot_path)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false);
    if exists {
        let _ = std::fs::remove_file(&shot_path);
    }
    let _ = std::fs::remove_file(&shot_path);

    if !native_granted && exists {
        return Ok(());
    }

    if !native_granted {
        return Err(anyhow::anyhow!(
            "Screen capture unavailable (Native permission missing). {}",
            permission_help()
        ));
    }

    if !exists {
        let probe_hint = match probe_status {
            Ok(status) => format!("status={}", status),
            Err(err) => format!("spawn_error={}", err),
        };
        return Err(anyhow::anyhow!(
            "Screen capture probe produced no file ({}). {}",
            probe_hint,
            permission_help()
        ));
    }

    Ok(())
}

fn env_truthy_default(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| {
            let n = v.trim().to_ascii_lowercase();
            matches!(n.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(default)
}
