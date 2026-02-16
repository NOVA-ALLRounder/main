use std::{
    fs::OpenOptions,
    net::SocketAddr,
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    str::FromStr,
    sync::{Mutex, OnceLock},
    time::Duration,
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    Manager,
};

#[derive(serde::Serialize)]
struct ChatProxyResponse {
    response: String,
    command: Option<String>,
}

#[tauri::command]
async fn proxy_chat(message: String) -> Result<ChatProxyResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| format!("http client init failed: {e}"))?;

    let targets = [
        "http://127.0.0.1:5680/api/chat",
        "http://localhost:5680/api/chat",
        "http://127.0.0.1:5680/chat",
        "http://localhost:5680/chat",
    ];

    let mut last_err = String::from("request failed");
    for url in targets {
        match client
            .post(url)
            .json(&serde_json::json!({ "message": message }))
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                if !status.is_success() {
                    last_err = format!("status {} body {}", status.as_u16(), body);
                    continue;
                }

                match serde_json::from_str::<serde_json::Value>(&body) {
                    Ok(v) => {
                        let response = v
                            .get("response")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        let command = v
                            .get("command")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string());
                        return Ok(ChatProxyResponse { response, command });
                    }
                    Err(e) => {
                        last_err = format!("invalid json: {e}");
                    }
                }
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }
    }

    Err(last_err)
}

fn resolve_core_binary_path() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let candidates = [
        exe_dir.join("core.exe"),
        exe_dir.join("core-x86_64-pc-windows-msvc.exe"),
        exe_dir.join("binaries").join("core.exe"),
        exe_dir.join("binaries").join("core-x86_64-pc-windows-msvc.exe"),
    ];

    candidates.into_iter().find(|p| p.exists())
}

fn is_dir_writable(path: &Path) -> bool {
    if std::fs::create_dir_all(path).is_err() {
        return false;
    }
    let test_file = path.join(".steer_write_test");
    let created = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&test_file)
        .is_ok();
    if created {
        let _ = std::fs::remove_file(test_file);
        return true;
    }
    false
}

fn core_sidecar_pid_slot() -> &'static Mutex<Option<u32>> {
    static CORE_SIDECAR_PID: OnceLock<Mutex<Option<u32>>> = OnceLock::new();
    CORE_SIDECAR_PID.get_or_init(|| Mutex::new(None))
}

fn record_core_sidecar_pid(pid: u32) {
    if let Ok(mut guard) = core_sidecar_pid_slot().lock() {
        *guard = Some(pid);
    }
}

fn terminate_core_sidecar_if_known() {
    let pid = core_sidecar_pid_slot()
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());

    let Some(pid) = pid else {
        return;
    };

    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

fn ensure_core_sidecar_running() {
    if api_reachable_quick() {
        println!("Core API already running on 127.0.0.1:5680");
        return;
    }

    let Some(core_bin) = resolve_core_binary_path() else {
        eprintln!("Core sidecar binary not found near app executable.");
        return;
    };

    let mut cmd = Command::new(&core_bin);
    cmd.arg("--api").arg("--port").arg("5680");

    if let Some(parent) = core_bin.parent() {
        cmd.current_dir(parent);

        // Keep sidecar startup deterministic regardless of parent shell env.
        cmd.env("STEER_ALLOW_MULTI", "1");
        cmd.env("STEER_API_PORT", "5680");

        let steer_home = std::env::var("STEER_HOME")
            .ok()
            .map(PathBuf::from)
            .filter(|p| is_dir_writable(p))
            .or_else(|| {
                std::env::var("LOCALAPPDATA")
                    .ok()
                    .map(|v| PathBuf::from(v).join("steer"))
                    .filter(|p| is_dir_writable(p))
            })
            .unwrap_or_else(|| parent.join(".steer_runtime"));
        let _ = std::fs::create_dir_all(&steer_home);
        cmd.env("STEER_HOME", steer_home);

        let out_log_path = parent.join("core_sidecar.out.log");
        let err_log_path = parent.join("core_sidecar.err.log");
        if let Ok(out_file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(out_log_path)
        {
            cmd.stdout(Stdio::from(out_file));
        }
        if let Ok(err_file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(err_log_path)
        {
            cmd.stderr(Stdio::from(err_file));
        }
    }

    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    match cmd.spawn() {
        Ok(child) => {
            record_core_sidecar_pid(child.id());
            for _ in 0..20 {
                std::thread::sleep(Duration::from_millis(250));
                if api_reachable_quick() {
                    println!("Core sidecar started on 127.0.0.1:5680");
                    return;
                }
            }
            eprintln!("Core sidecar spawned but API port 5680 is not responding yet.");
        }
        Err(e) => eprintln!("Failed to spawn core sidecar: {}", e),
    }
}

fn api_reachable_quick() -> bool {
    // Prevent setup-thread stalls when localhost socket is half-open or unresponsive.
    let Ok(addr) = SocketAddr::from_str("127.0.0.1:5680") else {
        return false;
    };
    TcpStream::connect_timeout(&addr, Duration::from_millis(180)).is_ok()
}



fn append_diag_log(line: &str) {
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    let dir = base.join("com.steer.os").join("logs");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("tauri_page_load.log");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = std::io::Write::write_all(&mut f, line.as_bytes());
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            append_diag_log("SETUP_START\n");
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            std::panic::set_hook(Box::new(|info| {
                eprintln!("Panic: {:?}", info);
                let _ = std::fs::write("panic.log", format!("{:?}", info));
            }));

            println!("Tauri app started");
            ensure_core_sidecar_running();
            append_diag_log("SETUP_AFTER_SIDECAR\n");

            if let Some(window) = app.get_webview_window("main") {
                println!(
                    "Main window found. Visible: {}",
                    window.is_visible().unwrap_or(false)
                );
                append_diag_log("SETUP_MAIN_WINDOW_FOUND\n");
                let _ = window.show();
                let _ = window.set_focus();
            } else {
                println!("Main window not found in setup");
                append_diag_log("SETUP_MAIN_WINDOW_NOT_FOUND\n");
            }

            let quit_i = MenuItem::with_id(app, "quit", "Quit Antigravity", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Show Launcher", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        terminate_core_sidecar_if_known();
                        app.exit(0)
                    }
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                terminate_core_sidecar_if_known();
            }
        })
        .on_page_load(|window, payload| {
            let log_line = format!(
                "PAGE_LOAD label={} url={}\n",
                window.label(),
                payload.url()
            );
            append_diag_log(&log_line);

            let probe = r#"
try {
  const id = 'steer-rust-probe';
  if (!document.getElementById(id)) {
    const el = document.createElement('div');
    el.id = id;
    el.textContent = 'RUST_WEBVIEW_EVAL_OK';
    el.style.position = 'fixed';
    el.style.top = '8px';
    el.style.right = '8px';
    el.style.zIndex = '2147483647';
    el.style.background = '#7f1d1d';
    el.style.color = 'white';
    el.style.padding = '6px 10px';
    el.style.fontFamily = 'Consolas, monospace';
    el.style.fontSize = '12px';
    document.body.appendChild(el);
  }
} catch (_) {}
"#;
            let _ = window.eval(probe);
        })
        .invoke_handler(tauri::generate_handler![proxy_chat])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}



