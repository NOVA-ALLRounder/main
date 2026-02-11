/// Windows input simulation using SendInput API

use crate::platform::traits::InputSimulator;
use anyhow::Result;
use std::mem;
use std::thread;
use std::time::Duration;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Windows Input Simulator implementation
pub struct WindowsInputSimulator;

impl InputSimulator for WindowsInputSimulator {
    fn click_at(&self, x: i32, y: i32) -> Result<()> {
        unsafe {
            // Get screen dimensions for absolute coordinate conversion
            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let screen_height = GetSystemMetrics(SM_CYSCREEN);

            // Convert pixel coordinates to absolute (0-65535 range)
            let abs_x = (x * 65536) / screen_width;
            let abs_y = (y * 65536) / screen_height;

            let inputs = [
                // Mouse move to position
                INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: abs_x,
                            dy: abs_y,
                            mouseData: 0,
                            dwFlags: MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_MOVE,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
                // Mouse left button down
                INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: 0,
                            dy: 0,
                            mouseData: 0,
                            dwFlags: MOUSEEVENTF_LEFTDOWN,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
                // Mouse left button up
                INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: 0,
                            dy: 0,
                            mouseData: 0,
                            dwFlags: MOUSEEVENTF_LEFTUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
            ];

            SendInput(&inputs, mem::size_of::<INPUT>() as i32);
        }

        Ok(())
    }

    fn type_text(&self, text: &str) -> Result<()> {
        unsafe {
            for ch in text.chars() {
                // Use KEYEVENTF_UNICODE for full Unicode support
                let inputs = [
                    // Key down
                    INPUT {
                        r#type: INPUT_KEYBOARD,
                        Anonymous: INPUT_0 {
                            ki: KEYBDINPUT {
                                wVk: VIRTUAL_KEY(0),
                                wScan: ch as u16,
                                dwFlags: KEYEVENTF_UNICODE,
                                time: 0,
                                dwExtraInfo: 0,
                            },
                        },
                    },
                    // Key up
                    INPUT {
                        r#type: INPUT_KEYBOARD,
                        Anonymous: INPUT_0 {
                            ki: KEYBDINPUT {
                                wVk: VIRTUAL_KEY(0),
                                wScan: ch as u16,
                                dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                                time: 0,
                                dwExtraInfo: 0,
                            },
                        },
                    },
                ];

                SendInput(&inputs, mem::size_of::<INPUT>() as i32);
                thread::sleep(Duration::from_millis(10));
            }
        }

        Ok(())
    }

    fn press_key(&self, key_code: u32) -> Result<()> {
        unsafe {
            let inputs = [
                // Key down
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VIRTUAL_KEY(key_code as u16),
                            wScan: 0,
                            dwFlags: KEYBD_EVENT_FLAGS(0),
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
                // Key up
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VIRTUAL_KEY(key_code as u16),
                            wScan: 0,
                            dwFlags: KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
            ];

            SendInput(&inputs, mem::size_of::<INPUT>() as i32);
        }

        Ok(())
    }

    fn scroll(&self, direction: &str, amount: i32) -> Result<()> {
        unsafe {
            let wheel_delta = match direction {
                "up" => amount * 120,     // WHEEL_DELTA = 120
                "down" => -amount * 120,
                "left" => amount * 120,
                "right" => -amount * 120,
                _ => return Err(anyhow::anyhow!("Invalid scroll direction: {}", direction)),
            };

            let dw_flags = match direction {
                "up" | "down" => MOUSEEVENTF_WHEEL,
                "left" | "right" => MOUSEEVENTF_HWHEEL,
                _ => MOUSE_EVENT_FLAGS(0),
            };

            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: wheel_delta as u32,
                        dwFlags: dw_flags,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            SendInput(&[input], mem::size_of::<INPUT>() as i32);
        }

        Ok(())
    }
}
