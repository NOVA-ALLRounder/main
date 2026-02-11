use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OperationMode {
    Observe,
    Copilot,
    Autopilot,
}

impl OperationMode {
    pub fn from_str(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "autopilot" => Self::Autopilot,
            "copilot" => Self::Copilot,
            _ => Self::Observe,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Copilot => "copilot",
            Self::Autopilot => "autopilot",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeControl {
    pub mode: OperationMode,
    pub emergency_stop: bool,
}

impl RuntimeControl {
    pub fn from_env() -> Self {
        let mode = std::env::var("STEER_OPERATION_MODE")
            .ok()
            .map(|v| OperationMode::from_str(&v))
            .unwrap_or(OperationMode::Autopilot);

        let emergency_stop = std::env::var("STEER_EMERGENCY_STOP")
            .ok()
            .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);

        Self {
            mode,
            emergency_stop,
        }
    }

    pub fn allow_automation(&self) -> bool {
        !self.emergency_stop && self.mode != OperationMode::Observe
    }
}
