/// Windows event monitoring using Raw Input API
///
/// Creates a hidden window to receive WM_INPUT messages for keyboard and mouse events.

use crate::platform::traits::EventMonitor;
use crate::schema::{EventEnvelope, ResourceContext};
use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::sync::mpsc;
use uuid::Uuid;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Windows Event Monitor implementation
pub struct WindowsEventMonitor {
    hwnd: Arc<Mutex<Option<HWND>>>,
}

impl WindowsEventMonitor {
    pub fn new() -> Self {
        Self {
            hwnd: Arc::new(Mutex::new(None)),
        }
    }
}

impl EventMonitor for WindowsEventMonitor {
    fn start(&self, tx: mpsc::Sender<String>) -> Result<()> {
        let hwnd_clone = self.hwnd.clone();

        thread::spawn(move || {
            if let Err(e) = run_message_loop(tx, hwnd_clone) {
                eprintln!("??Windows event monitoring failed: {}", e);
            }
        });

        Ok(())
    }

    fn stop(&self) -> Result<()> {
        if let Some(hwnd) = *self.hwnd.lock().unwrap() {
            unsafe {
                let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
        Ok(())
    }
}

/// Run the Windows message loop for event monitoring
fn run_message_loop(tx: mpsc::Sender<String>, hwnd_arc: Arc<Mutex<Option<HWND>>>) -> Result<()> {
    unsafe {
        // Register window class
        let class_name = w!("SteerEventMonitor");
        let h_instance = GetModuleHandleW(None)?;

        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: h_instance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };

        let atom = RegisterClassW(&wc);
        if atom == 0 {
            return Err(anyhow::anyhow!("Failed to register window class"));
        }

        // Create hidden window
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("Steer Event Monitor"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            h_instance,
            None,
        );

        if hwnd.0 == 0 {
            return Err(anyhow::anyhow!("Failed to create window"));
        }

        // Store the channel sender in window data
        let tx_ptr = Box::into_raw(Box::new(tx)) as isize;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, tx_ptr);

        // Store hwnd
        *hwnd_arc.lock().unwrap() = Some(hwnd);

        // Register for Raw Input (keyboard and mouse)
        let devices = [
            RAWINPUTDEVICE {
                usUsagePage: 0x01, // Generic Desktop
                usUsage: 0x06,     // Keyboard
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            },
            RAWINPUTDEVICE {
                usUsagePage: 0x01, // Generic Desktop
                usUsage: 0x02,     // Mouse
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            },
        ];

        if RegisterRawInputDevices(&devices, std::mem::size_of::<RAWINPUTDEVICE>() as u32).is_err() {
            return Err(anyhow::anyhow!("Failed to register raw input devices"));
        }

        println!("[Windows] Event monitoring started (Raw Input)");

        // Message loop
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Cleanup
        let tx_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
        if tx_ptr != 0 {
            let _ = Box::from_raw(tx_ptr as *mut mpsc::Sender<String>);
        }

        Ok(())
    }
}

/// Window procedure to handle messages
unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_INPUT => {
            // Get the channel sender from window data
            let tx_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if tx_ptr == 0 {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }

            let tx = &*(tx_ptr as *const mpsc::Sender<String>);

            // Process raw input
            let mut size: u32 = 0;
            GetRawInputData(
                HRAWINPUT(lparam.0 as _),
                RID_INPUT,
                None,
                &mut size,
                std::mem::size_of::<RAWINPUTHEADER>() as u32,
            );

            let mut buffer = vec![0u8; size as usize];
            if GetRawInputData(
                HRAWINPUT(lparam.0 as _),
                RID_INPUT,
                Some(buffer.as_mut_ptr() as _),
                &mut size,
                std::mem::size_of::<RAWINPUTHEADER>() as u32,
            ) != u32::MAX
            {
                let raw = &*(buffer.as_ptr() as *const RAWINPUT);

                let event_json = match raw.header.dwType {
                    0 => {
                        // Mouse event
                        let mouse = &raw.data.mouse;
                        if (mouse.Anonymous.Anonymous.usButtonFlags as u32) & RI_MOUSE_LEFT_BUTTON_DOWN != 0 {
                            // Left mouse down
                            let mut point = POINT::default();
                            let _ = GetCursorPos(&mut point);

                            let envelope = base_envelope(
                                "raw_input",
                                "system",
                                "click",
                                "P2",
                                Some(ResourceContext {
                                    resource_type: "input".to_string(),
                                    id: "mouse".to_string(),
                                }),
                                json!({ "location": { "x": point.x, "y": point.y } }),
                            );
                            serde_json::to_value(envelope).ok()
                        } else {
                            None
                        }
                    }
                    1 => {
                        // Keyboard event
                        let keyboard = &raw.data.keyboard;
                        if keyboard.Flags & 1 == 0 {
                            // Key down (RI_KEY_MAKE)
                            let envelope = base_envelope(
                                "raw_input",
                                "system",
                                "key_input",
                                "P2",
                                Some(ResourceContext {
                                    resource_type: "input".to_string(),
                                    id: "keyboard".to_string(),
                                }),
                                json!({ "keycode": keyboard.VKey }),
                            );
                            serde_json::to_value(envelope).ok()
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                if let Some(event) = event_json {
                    if let Ok(log) = serde_json::to_string(&event) {
                        let _ = tx.try_send(log);
                    }
                }
            }

            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn base_envelope(
    source: &str,
    app: &str,
    event_type: &str,
    priority: &str,
    resource: Option<ResourceContext>,
    payload: serde_json::Value,
) -> EventEnvelope {
    EventEnvelope {
        schema_version: "1.0".to_string(),
        event_id: Uuid::new_v4().to_string(),
        ts: Utc::now().to_rfc3339(),
        source: source.to_string(),
        app: app.to_string(),
        event_type: event_type.to_string(),
        priority: priority.to_string(),
        resource,
        payload,
        privacy: None,
        pid: None,
        window_id: None,
        window_title: None,
        browser_url: None,
        raw: None,
    }
}

impl Default for WindowsEventMonitor {
    fn default() -> Self {
        Self::new()
    }
}
