use anyhow::{anyhow, Result};
use lazy_static::lazy_static;
use log::{error, info};
use std::collections::HashSet;
use std::process::Command;
use std::sync::Mutex;

// Global cache of installed applications (loaded once at startup).
lazy_static! {
    static ref INSTALLED_APPS: Mutex<Option<HashSet<String>>> = Mutex::new(None);
}

fn builtin_apps() -> HashSet<String> {
    let mut apps = HashSet::new();
    #[cfg(target_os = "windows")]
    {
        for app in [
            "finder",
            "explorer",
            "file explorer",
            "mail",
            "outlook",
            "notes",
            "notepad",
            "textedit",
            "calculator",
            "calc",
            "safari",
            "microsoft edge",
            "google chrome",
            "chrome",
            "terminal",
            "powershell",
        ] {
            apps.insert(app.to_string());
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        for app in [
            "finder",
            "terminal",
            "safari",
            "google chrome",
            "mail",
            "notes",
            "textedit",
            "preview",
            "calendar",
        ] {
            apps.insert(app.to_string());
        }
    }
    apps
}

/// 1. Environment Scanner: Scan installed apps from the host OS.
pub fn scan_app_inventory() -> Result<()> {
    info!("[Reality] Scanning installed applications...");

    let mut apps = builtin_apps();

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg("Get-StartApps | Select-Object -ExpandProperty Name")
            .output()?;

        if output.status.success() {
            let stdout = String::from_utf8(output.stdout)?;
            for line in stdout.lines() {
                let name = line.trim();
                if !name.is_empty() {
                    apps.insert(name.to_lowercase());
                }
            }
        } else {
            println!("[Reality] Get-StartApps failed; continuing with builtin app map.");
        }

        if let Ok(proc_output) = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg("Get-Process | Select-Object -ExpandProperty ProcessName -Unique")
            .output()
        {
            if proc_output.status.success() {
                let stdout = String::from_utf8(proc_output.stdout)?;
                for line in stdout.lines() {
                    let proc_name = line.trim();
                    if !proc_name.is_empty() {
                        apps.insert(proc_name.to_lowercase());
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let output = Command::new("mdfind")
            .arg("kMDItemContentType == 'com.apple.application-bundle'")
            .output()?;

        if output.status.success() {
            let stdout = String::from_utf8(output.stdout)?;
            for line in stdout.lines() {
                if let Some(name) = line.split('/').last() {
                    let clean_name = name.trim_end_matches(".app").to_string();
                    if !clean_name.is_empty() {
                        apps.insert(clean_name.to_lowercase());
                    }
                }
            }
        } else {
            println!("[Reality] mdfind scan failed; continuing with builtin app map.");
        }
    }

    println!("[Reality] Inventory complete. Found {} apps.", apps.len());
    let sample: Vec<_> = apps.iter().take(5).collect();
    println!("[Reality] Sample: {:?}", sample);

    if let Ok(mut cache) = INSTALLED_APPS.lock() {
        *cache = Some(apps);
    }

    Ok(())
}

fn normalize_app_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

fn target_aliases(target_norm: &str) -> Vec<String> {
    let mut aliases = vec![target_norm.to_string()];
    match target_norm {
        "finder" | "explorer" | "fileexplorer" | "windowsexplorer" => {
            aliases.extend(
                ["finder", "explorer", "fileexplorer", "windowsexplorer"]
                    .iter()
                    .map(|s| s.to_string()),
            );
        }
        "mail" | "outlook" | "microsoftoutlook" => {
            aliases.extend(
                ["mail", "outlook", "microsoftoutlook"]
                    .iter()
                    .map(|s| s.to_string()),
            );
        }
        "notes" | "notepad" | "textedit" | "texteditor" => {
            aliases.extend(
                ["notes", "notepad", "textedit", "texteditor"]
                    .iter()
                    .map(|s| s.to_string()),
            );
        }
        "safari" | "microsoftedge" | "msedge" | "edge" => {
            aliases.extend(
                ["safari", "microsoftedge", "msedge", "edge"]
                    .iter()
                    .map(|s| s.to_string()),
            );
        }
        "calculator" | "calc" | "calculatorapp" => {
            aliases.extend(
                ["calculator", "calc", "calculatorapp"]
                    .iter()
                    .map(|s| s.to_string()),
            );
        }
        "googlechrome" | "chrome" => {
            aliases.extend(
                ["googlechrome", "chrome"]
                    .iter()
                    .map(|s| s.to_string()),
            );
        }
        "chatgptatlas" | "atlas" => aliases.push("chatgptatlas".to_string()),
        _ => {}
    }
    aliases.sort();
    aliases.dedup();
    aliases
}

/// 2. Pre-Flight Check: App Existence
pub fn verify_app_exists(app_name: &str) -> Result<String> {
    let app_name = app_name.trim();
    if app_name.is_empty() {
        return Err(anyhow!("REALITY_CHECK_INVALID_INPUT: app name is empty"));
    }

    let has_cache = INSTALLED_APPS.lock().map(|c| c.is_some()).unwrap_or(false);
    if !has_cache {
        println!("[Reality] Inventory is empty. Attempting lazy app scan...");
        if let Err(e) = scan_app_inventory() {
            println!("[Reality] Lazy app scan failed: {}", e);
        }
    }

    if let Ok(cache) = INSTALLED_APPS.lock() {
        if let Some(ref apps) = *cache {
            let target = app_name.to_lowercase();
            let target_norm = normalize_app_name(&target);
            let aliases = target_aliases(&target_norm);

            if apps.contains(&target) {
                return Ok(app_name.to_string());
            }
            for installed in apps {
                let installed_norm = normalize_app_name(installed);
                if aliases.iter().any(|alias| alias == &installed_norm) {
                    println!(
                        "[Reality] Normalized match: '{}' -> '{}'",
                        app_name, installed
                    );
                    return Ok(app_name.to_string());
                }
            }
            for installed in apps {
                if target.len() >= 5
                    && ((installed.starts_with(&target))
                        || (target.starts_with(installed) && installed.len() >= 5))
                {
                    println!("[Reality] Fuzzy match: '{}' -> '{}'", app_name, installed);
                    return Ok(app_name.to_string());
                }
            }

            println!("[Reality] Rejected: app '{}' is not installed.", app_name);
            return Err(anyhow!(
                "HALLUCINATION DETECTED: Application '{}' is not installed on this machine.",
                app_name
            ));
        }
    }

    let target = app_name.to_lowercase();
    let target_norm = normalize_app_name(&target);
    let aliases = target_aliases(&target_norm);
    let fallback = builtin_apps();
    if fallback.contains(&target)
        || fallback.iter().any(|installed| {
            let installed_norm = normalize_app_name(installed);
            aliases.iter().any(|alias| alias == &installed_norm)
        })
    {
        println!(
            "[Reality] Inventory unavailable; allowing '{}' via builtin alias map.",
            app_name
        );
        return Ok(app_name.to_string());
    }

    println!("[Reality] Inventory unavailable. Failing closed by default.");
    Err(anyhow!(
        "REALITY_CHECK_UNAVAILABLE: app inventory is unavailable; refusing to auto-open '{}'",
        app_name
    ))
}

/// 3. Pre-Flight Check: File Existence
pub fn verify_file_exists(path: &str) -> Result<()> {
    let path_obj = std::path::Path::new(path);
    if path_obj.exists() {
        Ok(())
    } else {
        error!("      [Reality] Rejected: file '{}' not found.", path);
        Err(anyhow!(
            "HALLUCINATION DETECTED: File '{}' does not exist.",
            path
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    lazy_static! {
        static ref TEST_MUTEX: Mutex<()> = Mutex::new(());
    }

    fn set_cache(apps: &[&str]) {
        if let Ok(mut cache) = INSTALLED_APPS.lock() {
            let mut set = HashSet::new();
            for app in apps {
                set.insert(app.to_lowercase());
            }
            *cache = Some(set);
        }
    }

    #[test]
    fn verify_app_exists_uses_normalized_exact_match() {
        let _guard = TEST_MUTEX.lock().expect("test mutex");
        set_cache(&["google chrome", "textedit"]);
        let result = verify_app_exists("GoogleChrome");
        assert!(result.is_ok());
    }

    #[test]
    fn verify_app_exists_rejects_short_substring_false_positive() {
        let _guard = TEST_MUTEX.lock().expect("test mutex");
        set_cache(&["calendar", "mail"]);
        let result = verify_app_exists("cal");
        assert!(result.is_err());
    }
}
