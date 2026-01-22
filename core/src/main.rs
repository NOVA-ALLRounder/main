mod schema;
mod state_machine;
mod policy;
mod ipc;

use crate::schema::{AgentCommand, AgentAction};
use tokio::time::{sleep, Duration};
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Starting Local OS Agent Core...");

    let (tx, mut rx) = ipc::AdapterClient::spawn()?;
    
    // Give adapter a moment to spin up
    sleep(Duration::from_secs(1)).await;

    // Phase 1: Test Observation
    println!(" Sending UiSnapshot request...");
    let req_id = Uuid::new_v4().to_string();
    let cmd = AgentCommand {
        id: req_id.clone(),
        action: AgentAction::UiSnapshot { scope: None },
    };

    tx.send(cmd).await.expect("Failed to send command");

    // Wait for response
    if let Some(resp) = rx.recv().await {
        println!("Received Response: {:?}", resp);
        if resp.request_id == req_id {
            println!("Verification Passed: Request ID matches.");
            if let Some(data) = resp.data {
                println!("Snapshot Data: {}", serde_json::to_string_pretty(&data)?);
            }
        } else {
            println!("Verification Failed: Request ID mismatch.");
        }
    } else {
        println!("Channel closed or no response.");
    }

    // Phase 2: Test Policy & Act
    let mut policy = policy::PolicyEngine::new();
    
    // 2.1 Try Click while LOCKED
    println!("\n[Test] Attempting Click while LOCKED...");
    let click_cmd = AgentAction::UiClick { element_id: "fake_id".to_string(), double_click: false };
    
    match policy.check(&click_cmd) {
        Ok(_) => println!("Allowed (Unexpected)"),
        Err(e) => println!("Blocked (Expected): {}", e),
    }

    // 2.2 Unlock and Try Click
    println!("\n[Test] Unlocking and Attempting Click...");
    policy.unlock();
    
    match policy.check(&click_cmd) {
        Ok(_) => {
            println!("Policy Passed. Sending Command...");
            // In a real scenario, we would send it here.
            // For MVP verification, we just print success.
            // Note: Sending 'fake_id' to adapter will result in error from adapter (Element not found), 
            // but the Policy check is what we are testing here.
            
            let req_id = Uuid::new_v4().to_string();
            let cmd = AgentCommand {
                id: req_id.clone(),
                action: click_cmd,
            };
            tx.send(cmd).await.expect("Failed to send command");
             // Response handling loop omitted for brevity in this specific test block
        },
        Err(e) => println!("Blocked (Unexpected): {}", e),
    }

    // Give some time for adapter to process (and likely fail due to fake_id)
    sleep(Duration::from_secs(1)).await;
    
    Ok(()) // End of main
}
