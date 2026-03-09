use super::*;

#[test]
fn test_provider_from_str() {
    assert_eq!(LLMProvider::from_str("gemini"), Some(LLMProvider::Gemini));
    assert_eq!(LLMProvider::from_str("CODEX"), Some(LLMProvider::Codex));
    assert_eq!(LLMProvider::from_str("Claude"), Some(LLMProvider::Claude));
    assert_eq!(LLMProvider::from_str("unknown"), None);
}

#[test]
fn test_build_command() {
    let client = CLILLMClient::new(LLMProvider::Gemini);
    let (cmd, args) = client.build_command();
    assert_eq!(cmd, "gemini");
    assert!(args.contains(&"-s".to_string()));
}

#[test]
fn test_messages_to_prompt() {
    let messages = vec![
        serde_json::json!({"role": "system", "content": "You are helpful."}),
        serde_json::json!({"role": "user", "content": "Hello!"}),
    ];
    let prompt = messages_to_prompt(&messages);
    assert!(prompt.contains("[System Instructions]"));
    assert!(prompt.contains("You are helpful."));
    assert!(prompt.contains("[User]"));
    assert!(prompt.contains("Hello!"));
}
