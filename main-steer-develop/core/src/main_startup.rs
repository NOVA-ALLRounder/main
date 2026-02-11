use crate::env_flag;

pub fn print_env_diagnostics() {
    let cwd = std::env::current_dir().unwrap_or_default();
    println!("🔍 Current Working Directory: {}", cwd.display());
    match dotenv::dotenv() {
        Ok(path) => println!("✅ Loaded .env from: {}", path.display()),
        Err(e) => println!("⚠️ Failed to load .env: {}", e),
    }
}

pub fn install_panic_hook() {
    if env_flag("STEER_PANIC_STD") {
        eprintln!("⚠️  Panic hook disabled (STEER_PANIC_STD=1).");
        return;
    }

    std::panic::set_hook(Box::new(|info| {
        let backtrace = std::backtrace::Backtrace::capture();
        let timestamp = chrono::Utc::now().to_rfc3339();

        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            *s
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            &**s
        } else {
            "Unknown panic"
        };

        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown".to_string());

        let log_entry = format!(
            "[{}] CRASH REPORT\nMessage: {}\nLocation: {}\nBacktrace:\n{:#?}\n--------------------------------------------------\n",
            timestamp, msg, location, backtrace
        );

        let log_dir = crate::paths::app_dir().join("logs");
        if std::fs::create_dir_all(&log_dir).is_ok() {
            let log_file = log_dir.join("crash.log");
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_file)
            {
                use std::io::Write;
                let _ = writeln!(file, "{}", log_entry);
            }
        }

        eprintln!("🚨 FATAL ERROR: {}", msg);
        eprintln!("🧾 Crash report saved to {}/crash.log", log_dir.display());
    }));
}

pub fn print_ui_permission_hint() {
    println!("Checking UI permissions...");
    #[cfg(target_os = "macos")]
    {
        let ax_check = std::process::Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to return name of first application process")
            .output();

        match ax_check {
            Ok(output) if output.status.success() => {
                println!("Accessibility Permissions: GRANTED.");
            }
            _ => {
                println!("Accessibility permission missing/revoked. UI click/type may fail.");
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        println!("Windows mode: UI automation depends on active desktop session and permissions.");
    }
}

