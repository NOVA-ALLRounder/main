use std::collections::HashMap;

use super::*;

#[test]
fn test_mcp_server_config() {
    let server = McpServer {
        name: "test".to_string(),
        command: "echo".to_string(),
        args: vec!["hello".to_string()],
        env: HashMap::new(),
        enabled: true,
    };

    let json = serde_json::to_string(&server).unwrap();
    assert!(json.contains("test"));
}

#[test]
fn test_registry_default_config() {
    let registry = McpRegistry::new();
    assert!(registry.list_all_tools().is_empty());
}
