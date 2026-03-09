use super::*;

#[test]
fn test_init_creates_table() {
    let result = init();
    assert!(result.is_ok());
}

#[test]
fn test_insert_event() {
    init().ok(); // Might error if already init

    let test_event = r#"{"type":"test","source":"unit_test"}"#;
    let insert_result = insert_event(test_event);
    assert!(insert_result.is_ok());
}
