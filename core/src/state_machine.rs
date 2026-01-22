use crate::schema::AgentAction;

#[derive(Debug)]
pub enum AgentState {
    Idle,
    Observing,
    Deciding { snapshot: serde_json::Value },
    Authorizing { pending_action: AgentAction },
    Acting { approved_action: AgentAction },
    Verifying { executed_action: AgentAction },
    Terminated { reason: String },
}

pub struct AgentCore {
    state: AgentState,
    // policy_engine: PolicyEngine,
}

impl AgentCore {
    pub async fn run_cycle(&mut self) {
        // Loop logic implementation (Refer to Step 3 logic)
        // Observe -> LLM Call -> Policy Check -> Act -> Verify -> Loop
        println!("Agent running cycle...");
    }
}
