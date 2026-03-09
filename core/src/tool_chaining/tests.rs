use super::*;

#[test]
fn test_context_substitution() {
    let mut ctx = ExecutionContext::new();
    ctx.set("name", "John");
    ctx.set("value", "123");

    assert_eq!(ctx.substitute("Hello {{name}}!"), "Hello John!");
    assert_eq!(ctx.substitute("Value is $value"), "Value is 123");
}

#[test]
fn test_tool_result_chaining() {
    let mut ctx = ExecutionContext::new();

    let result1 = ToolResult::success("calc", serde_json::json!({"result": 579}))
        .with_data("calc_result", "579");

    ctx.add_result(result1);

    assert_eq!(ctx.get("calc_result"), Some(&"579".to_string()));
}
