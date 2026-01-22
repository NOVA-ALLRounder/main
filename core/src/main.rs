mod schema;
mod state_machine;
mod policy;
mod ipc;

use crate::schema::{AgentCommand, AgentAction};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt};
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("🤖 Local OS Agent Started!");
    println!("--------------------------------------------------");
    println!("Type 'help' for commands. (Needs Accessibility Permissions)");
    println!("--------------------------------------------------");

    // 1. Start Adapter
    let (tx, mut rx) = ipc::AdapterClient::spawn()?;
    let mut policy = policy::PolicyEngine::new(); // Starts LOCKED

    // 2. Start Response Listener (Background)
    tokio::spawn(async move {
        while let Some(resp) = rx.recv().await {
            println!("\n[Agent] Received Action Result ({})", resp.status);
            if let Some(err) = resp.error {
                println!("   ❌ Error: {}", err);
            }
            if let Some(data) = resp.data {
                // Pretty print data if it's not empty
                let json = serde_json::to_string_pretty(&data).unwrap_or_default();
                if json != "{}" {
                    println!("   📄 Data: {}", json);
                }
            }
            print!("> "); // Re-prompt
            let _ = io::stdout().flush().await; 
        }
    });

    // 3. User Input Loop (REPL)
    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin);
    let mut buffer = String::new();

    print!("> ");
    let _ = io::stdout().flush().await;

    while reader.read_line(&mut buffer).await? > 0 {
        let input = buffer.trim().to_string();
        buffer.clear();

        if input.is_empty() {
            print!("> ");
            let _ = io::stdout().flush().await;
            continue;
        }

        let parts: Vec<&str> = input.split_whitespace().collect();
        let cmd = parts[0];

        match cmd {
            "help" => {
                println!("Available commands:");
                println!("  snap             - Capture UI Snapshot (Observe)");
                println!("  click <id>       - Click an element by ID");
                println!("  type <text>      - Type text (requires focus)");
                println!("  unlock           - Disable Write Lock (Safety)");
                println!("  lock             - Enable Write Lock (Safety)");
                println!("  quit             - Exit");
            }
            "quit" | "exit" => break,
            "lock" => policy.lock(),
            "unlock" => policy.unlock(),
            "snap" => {
                 let req_id = Uuid::new_v4().to_string();
                 let action = AgentAction::UiSnapshot { scope: None };
                 // Policy Check
                 if let Err(e) = policy.check(&action) {
                     println!("⛔️ Policy Blocked: {}", e);
                 } else {
                     println!("sending snapshot request...");
                     tx.send(AgentCommand { id: req_id, action }).await?;
                 }
            }
            "click" => {
                if parts.len() < 2 {
                    println!("Usage: click <element_id>");
                } else {
                    let req_id = Uuid::new_v4().to_string();
                    let action = AgentAction::UiClick { element_id: parts[1].to_string(), double_click: false };
                    if let Err(e) = policy.check(&action) {
                        println!("⛔️ Policy Blocked: {}", e);
                    } else {
                         println!("sending click request...");
                        tx.send(AgentCommand { id: req_id, action }).await?;
                    }
                }
            }
            "type" => {
                if parts.len() < 2 {
                    println!("Usage: type <text>");
                } else {
                    let text = parts[1..].join(" ");
                    let req_id = Uuid::new_v4().to_string();
                    let action = AgentAction::KeyboardType { text, submit: true };
                     if let Err(e) = policy.check(&action) {
                        println!("⛔️ Policy Blocked: {}", e);
                     } else {
                        println!("sending type request...");
                        tx.send(AgentCommand { id: req_id, action }).await?;
                    }
                }
            }
            _ => println!("Unknown command. Type 'help'."),
        }

        print!("> ");
        let _ = io::stdout().flush().await;
    }

    Ok(())
}
