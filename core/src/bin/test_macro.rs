use local_os_agent::llm_gateway::LLMClient;
use local_os_agent::dynamic_controller::DynamicController;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("🧪 Starting Macro Recorder Test...");
    
    // 1. Initialize
    local_os_agent::db::init()?; // Initialize DB
    let llm = LLMClient::new()?;
    let controller = DynamicController::new(llm);

    // 2. Goal: Record
    println!("🎥 Phase 1: Recording 'test_routine'");
    // We ask it to check the file list safely (ls -la) then save it.
    // Actually, let's ask it to 'read' the screen then save the routine. Reading is safe.
    controller.surf("Wait for 2 seconds (action 'wait'). Then save the routine as 'test_routine'. Then done.").await?;

    println!("--------------------------------------------------");

    // 3. Goal: Replay
    println!("▶️ Phase 2: Replaying 'test_routine'");
    controller.surf("Replay the routine named 'test_routine'. Then click done.").await?;

    println!("✅ Test Execution Complete. Check logs for 'Saved Routine' and 'Replayed Routine'.");
    Ok(())
}
