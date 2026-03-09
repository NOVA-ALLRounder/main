use super::*;
use serde_json::json;

#[test]
fn normalize_open_app() {
    let plan = json!({"action": "open_app", "app": "Notes"});
    let result = normalize_action(&plan);
    assert!(result.error.is_none());
    assert_eq!(result.normalized["name"].as_str().unwrap(), "Notes");
}

#[test]
fn normalize_shortcut_keys() {
    let plan = json!({"action": "shortcut", "keys": ["command", "n"]});
    let result = normalize_action(&plan);
    assert!(result.error.is_none());
    assert_eq!(result.normalized["key"].as_str().unwrap(), "n");
    assert_eq!(
        result.normalized["modifiers"][0].as_str().unwrap(),
        "command"
    );
}

#[test]
fn normalize_shortcut_cmd_plus_v_to_paste() {
    let plan = json!({"action": "shortcut", "key": "cmd+v"});
    let result = normalize_action(&plan);
    assert!(result.error.is_none());
    assert_eq!(result.normalized["action"].as_str().unwrap(), "paste");
}

#[test]
fn normalize_key_combo_to_shortcut() {
    let plan = json!({"action": "key", "key": "command+n"});
    let result = normalize_action(&plan);
    assert!(result.error.is_none());
    assert_eq!(result.normalized["action"].as_str().unwrap(), "shortcut");
    assert_eq!(result.normalized["key"].as_str().unwrap(), "n");
    assert_eq!(
        result.normalized["modifiers"][0].as_str().unwrap(),
        "command"
    );
}

#[test]
fn unknown_action_errors() {
    let plan = json!({"action": "unknown"});
    let result = normalize_action(&plan);
    assert!(result.error.is_some());
}

#[test]
fn normalize_telegram_send_alias() {
    let plan = json!({"action": "send_telegram", "text": "hello"});
    let result = normalize_action(&plan);
    assert!(result.error.is_none());
    assert_eq!(
        result.normalized["action"].as_str().unwrap(),
        "telegram_send"
    );
    assert_eq!(result.normalized["message"].as_str().unwrap(), "hello");
}

#[test]
fn normalize_notion_create_alias() {
    let plan = json!({"action": "notion_create", "title": "t", "text": "body"});
    let result = normalize_action(&plan);
    assert!(result.error.is_none());
    assert_eq!(
        result.normalized["action"].as_str().unwrap(),
        "notion_write"
    );
    assert_eq!(result.normalized["content"].as_str().unwrap(), "body");
}

#[test]
fn normalize_n8n_execute_alias() {
    let plan = json!({"action": "run_n8n_workflow", "id": "wf_123"});
    let result = normalize_action(&plan);
    assert!(result.error.is_none());
    assert_eq!(
        result.normalized["action"].as_str().unwrap(),
        "n8n_execute_workflow"
    );
    assert_eq!(result.normalized["workflow_id"].as_str().unwrap(), "wf_123");
}
