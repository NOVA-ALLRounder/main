use crate::schema::AgentAction;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SecurityLevel {
    Safe,
    Caution,
    Critical,
}

pub struct PolicyEngine {
    pub write_lock: bool,
}

impl PolicyEngine {
    pub fn new() -> Self {
        Self { write_lock: true } // Default Locked
    }

    pub fn check(&self, action: &AgentAction) -> Result<(), String> {
        let level = self.classify(action);
        match level {
            SecurityLevel::Safe => Ok(()),
            SecurityLevel::Caution => {
                if self.write_lock {
                    Err("Write Lock Engaged: Action requires approval.".to_string())
                } else {
                    Ok(())
                }
            }
            SecurityLevel::Critical => {
                Err("Critical Action: Requires explicit 2FA/Confirmation (Not implemented).".to_string())
            }
        }
    }

    fn classify(&self, action: &AgentAction) -> SecurityLevel {
        match action {
            AgentAction::UiSnapshot { .. } | AgentAction::UiFind { .. } => SecurityLevel::Safe,
            AgentAction::UiClick { .. } | AgentAction::KeyboardType { .. } => SecurityLevel::Caution,
            AgentAction::Terminate => SecurityLevel::Critical,
        }
    }
    
    pub fn unlock(&mut self) {
        self.write_lock = false;
        println!("[Policy] Write Lock UNLOCKED.");
    }
    
    pub fn lock(&mut self) {
        self.write_lock = true;
        println!("[Policy] Write Lock ENGAGED.");
    }
}
