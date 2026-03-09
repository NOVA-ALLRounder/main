use anyhow::Result;

use super::{CrossAppBridge, ExecutionContext, ToolResult};

/// Execute a complex multi-app scenario
pub async fn execute_scenario(description: &str) -> Result<ExecutionContext> {
    let mut ctx = ExecutionContext::new();

    println!("🎯 [Scenario] Starting: {}", description);

    ctx.set("scenario", description);
    ctx.current_app = CrossAppBridge::get_frontmost_app().ok();

    Ok(ctx)
}

/// Read content from an app (generic)
pub fn read_from_app(app_name: &str, ctx: &mut ExecutionContext) -> Result<ToolResult> {
    CrossAppBridge::switch_to_app(app_name)?;
    std::thread::sleep(std::time::Duration::from_millis(300));

    let script = r#"tell application "System Events"
        keystroke "a" using command down
        delay 0.1
        keystroke "c" using command down
    end tell"#;

    std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .status()?;

    std::thread::sleep(std::time::Duration::from_millis(200));

    let content = CrossAppBridge::get_clipboard()?;
    ctx.set("last_read", &content);

    Ok(ToolResult::success(
        "read_from_app",
        serde_json::json!({
            "app": app_name,
            "content": content
        }),
    )
    .with_data("content", &content))
}

/// Write content to an app
pub fn write_to_app(
    app_name: &str,
    content: &str,
    ctx: &mut ExecutionContext,
) -> Result<ToolResult> {
    let resolved_content = ctx.substitute(content);

    CrossAppBridge::switch_to_app(app_name)?;
    std::thread::sleep(std::time::Duration::from_millis(300));

    CrossAppBridge::copy_to_clipboard(&resolved_content)?;
    CrossAppBridge::paste()?;

    Ok(ToolResult::success(
        "write_to_app",
        serde_json::json!({
            "app": app_name,
            "written": resolved_content.len()
        }),
    ))
}

/// Calculate expression
pub fn calculate(expression: &str, ctx: &mut ExecutionContext) -> Result<ToolResult> {
    let _output = std::process::Command::new("bc")
        .arg("-l")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?
        .wait_with_output()?;

    let output = std::process::Command::new("python3")
        .arg("-c")
        .arg(format!("print({})", expression))
        .output()?;

    let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
    ctx.set("calc_result", &result);

    Ok(ToolResult::success(
        "calculate",
        serde_json::json!({
            "expression": expression,
            "result": result
        }),
    )
    .with_data("result", &result))
}
