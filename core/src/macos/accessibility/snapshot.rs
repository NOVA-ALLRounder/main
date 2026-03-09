use super::{config::*, focus::*, *};
use serde_json::json;

pub fn snapshot(_scope: Option<String>) -> Value {
    println!("[MacOS] Capturing Snapshot (Native)...");

    unsafe {
        let strict_mode = snapshot_strict_mode();
        let retry_count = snapshot_focus_retry_count();
        let retry_sleep = Duration::from_millis(snapshot_focus_retry_ms());
        let window_retry_count = snapshot_window_retry_count();
        let window_retry_sleep = Duration::from_millis(snapshot_window_retry_ms());
        let fallback_app = snapshot_fallback_app_name();

        let system_wide = AXUIElementCreateSystemWide();
        let _system_wrapper = AxElement(system_wide);

        let mut focused_app_ref = resolve_focused_application(system_wide);
        if focused_app_ref.is_none() {
            for _ in 0..retry_count {
                let (front_name, front_pid) =
                    frontmost_app_info_via_osascript().unwrap_or_default();
                if let Some(pid) = front_pid {
                    bring_process_frontmost(pid);
                } else if !front_name.is_empty() {
                    activate_application_by_name(&front_name);
                }
                thread::sleep(retry_sleep);
                focused_app_ref = resolve_focused_application(system_wide);
                if focused_app_ref.is_some() {
                    break;
                }
            }
        }
        if focused_app_ref.is_none() {
            activate_application_by_name(&fallback_app);
            thread::sleep(retry_sleep);
            focused_app_ref = resolve_focused_application(system_wide);
        }
        let focused_app_ref = match focused_app_ref {
            Some(v) => v,
            None => {
                let (front_name, _) = frontmost_app_info_via_osascript().unwrap_or_default();
                crate::diagnostic_events::emit(
                    "ax.snapshot.focus_missing",
                    json!({
                        "kind": "application",
                        "frontmost_app_hint": front_name,
                        "fallback_app": fallback_app,
                        "strict_mode": strict_mode
                    }),
                );
                let mut payload = json!({
                    "role": "AXApplication",
                    "title": front_name,
                    "focused_window": {
                        "role": "AXWindow",
                        "title": "",
                        "children": []
                    },
                    "warning": "No focused application",
                    "frontmost_app_hint": front_name,
                    "strict_mode": strict_mode,
                    "ok": false
                });
                if strict_mode {
                    payload["error"] = json!("NO_FOCUSED_APPLICATION");
                }
                return payload;
            }
        };
        let _focused_app = AxElement(focused_app_ref);

        let fallback_front_name = frontmost_app_name_via_osascript().unwrap_or_default();
        let app_title = {
            let via_ax = get_string_attribute(focused_app_ref, "AXTitle").unwrap_or_default();
            if via_ax.is_empty() {
                fallback_front_name
            } else {
                via_ax
            }
        };

        let mut focused_window_ref = resolve_focused_window(focused_app_ref);
        if focused_window_ref.is_none() {
            for _ in 0..window_retry_count {
                let (front_name, front_pid) =
                    frontmost_app_info_via_osascript().unwrap_or_default();
                if let Some(pid) = front_pid {
                    bring_process_frontmost(pid);
                } else if !front_name.trim().is_empty() {
                    activate_application_by_name(&front_name);
                } else if !app_title.trim().is_empty() {
                    activate_application_by_name(&app_title);
                } else {
                    activate_application_by_name(&fallback_app);
                }
                thread::sleep(window_retry_sleep);
                focused_window_ref = resolve_focused_window(focused_app_ref);
                if focused_window_ref.is_some() {
                    break;
                }
            }
        }
        let focused_window_ref = match focused_window_ref {
            Some(r) => r,
            None => {
                crate::diagnostic_events::emit(
                    "ax.snapshot.focus_missing",
                    json!({
                        "kind": "window",
                        "app_title": app_title,
                        "fallback_app": fallback_app,
                        "strict_mode": strict_mode
                    }),
                );
                let mut payload = json!({
                    "role": "AXApplication",
                    "title": app_title,
                    "focused_window": {
                        "role": "AXWindow",
                        "title": "",
                        "children": []
                    },
                    "warning": "No focused window",
                    "strict_mode": strict_mode,
                    "ok": false
                });
                if strict_mode {
                    payload["error"] = json!("NO_FOCUSED_WINDOW");
                }
                return payload;
            }
        };
        let _focused_window = AxElement(focused_window_ref);

        let window_title = get_string_attribute(focused_window_ref, "AXTitle").unwrap_or_default();
        let children_json = traverse_children(focused_window_ref, 0, 2);

        json!({
            "role": "AXApplication",
            "title": app_title,
            "focused_window": {
                "role": "AXWindow",
                "title": window_title,
                "children": children_json
            },
            "strict_mode": strict_mode,
            "ok": true
        })
    }
}

unsafe fn traverse_children(element: AXUIElementRef, depth: usize, max_depth: usize) -> Vec<Value> {
    if depth > max_depth {
        return vec![];
    }

    let mut nodes = Vec::new();

    if let Some(children_ref) = get_attribute(element, "AXChildren") {
        let children_array = CFArray::<CFTypeRef>::wrap_under_get_rule(
            children_ref as core_foundation::array::CFArrayRef,
        );

        for i in 0..children_array.len() {
            let Some(child_ptr) = children_array.get(i) else {
                continue;
            };
            let child_element = *child_ptr as AXUIElementRef;

            let role = get_string_attribute(child_element, "AXRole").unwrap_or_default();
            let title = get_string_attribute(child_element, "AXTitle").unwrap_or_default();
            let value = get_string_attribute(child_element, "AXValue").unwrap_or_default();

            let sub_children = if depth < max_depth {
                traverse_children(child_element, depth + 1, max_depth)
            } else {
                vec![]
            };

            let mut node = json!({
                "role": role,
                "children": sub_children
            });

            if !title.is_empty() {
                node["title"] = json!(title);
            }
            if !value.is_empty() {
                node["value"] = json!(value);
            }

            nodes.push(node);
        }
        core_foundation::base::CFRelease(children_ref);
    }

    nodes
}
