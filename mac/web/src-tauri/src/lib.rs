use tauri::{
  menu::{Menu, MenuItem},
  tray::{TrayIconBuilder, TrayIconEvent},
  Manager,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }

      // [QC] Initialize Updater
      app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;

      // System Tray Setup
      let quit_i = MenuItem::with_id(app, "quit", "Quit Antigravity", true, None::<&str>)?;
      let show_i = MenuItem::with_id(app, "show", "Show Launcher", true, None::<&str>)?;
      let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

      let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "quit" => {
                    app.exit(0);
                }
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                _ => {}
            }
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
    .on_window_event(|window, event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            // Prevent close, hide instead
            window.hide().unwrap();
            api.prevent_close();
        }
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
