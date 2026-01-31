use tauri_plugin_shell::ShellExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_shell::init())
    .plugin(tauri_plugin_updater::Builder::new().build())
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      /* Sidecar removed to prevent crash. We use external script orchestration now. */
      /*
      // Auto-launch Core Sidecar
      let sidecar_command = app.handle().shell().sidecar("core").map_err(|e| e.to_string())?;
      let (mut rx, _child) = sidecar_command.spawn().map_err(|e| e.to_string())?;
      
      // Optional: Read sidecar output in background
      tauri::async_runtime::spawn(async move {
          while let Some(event) = rx.recv().await {
              match event {
                  tauri_plugin_shell::process::CommandEvent::Stdout(line) => {
                      println!("Core: {}", String::from_utf8_lossy(&line));
                  }
                  tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                      eprintln!("Core Error: {}", String::from_utf8_lossy(&line));
                  }
                  _ => {}
              }
          }
      });
      */

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
