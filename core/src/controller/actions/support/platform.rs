use crate::controller::heuristics;
use crate::platform::{app_matches_role, app_role_primary_name, current_platform, AppRole};
use std::thread;
use std::time::Duration as StdDuration;
use tokio::time::{sleep, Duration};

use crate::controller::actions::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) fn app_has_role(app: &str, role: AppRole) -> bool {
        app_matches_role(current_platform().kind(), role, app)
    }

    pub(in crate::controller::actions) fn role_app_name(role: AppRole) -> &'static str {
        app_role_primary_name(current_platform().kind(), role)
    }

    pub(in crate::controller::actions) async fn ensure_role_focus(role: AppRole, retries: usize) {
        let _ = heuristics::ensure_app_focus(Self::role_app_name(role), retries).await;
    }

    pub(in crate::controller::actions) fn platform_keyboard_shortcut(
        key: &str,
        modifiers: &[&str],
    ) -> anyhow::Result<()> {
        let modifiers = modifiers
            .iter()
            .map(|modifier| (*modifier).to_string())
            .collect::<Vec<_>>();
        current_platform().keyboard_shortcut(key, &modifiers)
    }

    pub(in crate::controller::actions) async fn capture_front_text_via_platform(
        select_all_first: bool,
        after_select_ms: u64,
        after_copy_ms: u64,
    ) -> String {
        if select_all_first {
            let _ = Self::platform_keyboard_shortcut("a", &["command"]);
            sleep(Duration::from_millis(after_select_ms)).await;
        }

        if let Ok(Some(selected_text)) = current_platform().selected_text() {
            if !selected_text.trim().is_empty() {
                return selected_text;
            }
        }

        let _ = Self::platform_keyboard_shortcut("c", &["command"]);
        sleep(Duration::from_millis(after_copy_ms)).await;
        crate::tool_chaining::CrossAppBridge::get_clipboard().unwrap_or_default()
    }

    pub(in crate::controller::actions) fn capture_front_text_via_platform_sync(
        select_all_first: bool,
        after_select_ms: u64,
        after_copy_ms: u64,
    ) -> anyhow::Result<String> {
        if select_all_first {
            Self::platform_keyboard_shortcut("a", &["command"])?;
            thread::sleep(StdDuration::from_millis(after_select_ms));
        }

        if let Ok(Some(selected_text)) = current_platform().selected_text() {
            if !selected_text.trim().is_empty() {
                return Ok(selected_text);
            }
        }

        let previous_clipboard = current_platform().get_clipboard_text().ok();
        Self::platform_keyboard_shortcut("c", &["command"])?;
        thread::sleep(StdDuration::from_millis(after_copy_ms));
        let captured = current_platform().get_clipboard_text();
        if let Some(clipboard) = previous_clipboard {
            let _ = current_platform().set_clipboard_text(&clipboard);
        }
        captured
    }

    pub(in crate::controller::actions) fn replace_front_text_via_platform_sync(
        text: &str,
        select_all_first: bool,
        after_select_ms: u64,
        after_paste_ms: u64,
    ) -> anyhow::Result<()> {
        let previous_clipboard = current_platform().get_clipboard_text().ok();

        let result = (|| -> anyhow::Result<()> {
            if select_all_first {
                Self::platform_keyboard_shortcut("a", &["command"])?;
                thread::sleep(StdDuration::from_millis(after_select_ms));
            }

            current_platform().set_clipboard_text(text)?;
            current_platform().paste_clipboard()?;
            thread::sleep(StdDuration::from_millis(after_paste_ms));
            Ok(())
        })();

        if let Some(clipboard) = previous_clipboard {
            let _ = current_platform().set_clipboard_text(&clipboard);
        }

        result
    }
}
