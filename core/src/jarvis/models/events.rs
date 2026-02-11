// JARVIS Event definitions (Event-driven architecture)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JarvisEvent {
    // System events
    AppSwitched {
        from: String,
        to: String,
        timestamp: i64,
    },
    WindowFocused {
        title: String,
        timestamp: i64,
    },
    UserActivity {
        event_type: ActivityType,
        timestamp: i64,
    },

    // Pattern events
    PatternDetected {
        pattern_id: String,
        pattern_type: String,
        confidence: f32,
    },
    AutomationSuggested {
        suggestion_id: String,
        title: String,
    },

    // Workflow events
    WorkflowCreated {
        workflow_id: String,
        name: String,
    },
    WorkflowExecuted {
        workflow_id: String,
        success: bool,
    },

    // User events
    CommandReceived {
        command: String,
        source: String,
    },
    ApprovalGiven {
        approval_id: String,
        approved: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivityType {
    AppLaunch,
    AppSwitch,
    WindowFocus,
    Click,
    Type,
    Scroll,
}

impl JarvisEvent {
    pub fn timestamp(&self) -> Option<i64> {
        match self {
            Self::AppSwitched { timestamp, .. } => Some(*timestamp),
            Self::WindowFocused { timestamp, .. } => Some(*timestamp),
            Self::UserActivity { timestamp, .. } => Some(*timestamp),
            _ => None,
        }
    }
}
