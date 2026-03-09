use super::*;
use serde_json::json;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn create_workflow_uses_mock_path_when_enabled() {
    unsafe {
        std::env::set_var("STEER_N8N_MOCK", "1");
    }
    let api = N8nApi::new("http://127.0.0.1:5678/api/v1", "");
    let wf = json!({
        "nodes": [],
        "connections": {}
    });
    let result = api.create_workflow("mock-test", &wf, true).await;
    unsafe {
        std::env::remove_var("STEER_N8N_MOCK");
    }

    assert!(result.is_ok());
    let id = result.unwrap_or_default();
    assert!(id.starts_with("mock-wf-"));
}
