use crate::approval_gate;
use crate::controller::heuristics;
use crate::nl_automation::{ApprovalContext, ExecutionResult, Plan, StepType};
use crate::platform::{app_matches_role, app_role_aliases, current_platform, AppRole};

use crate::browser_automation;
use crate::visual_driver::{SmartStep, UiAction, VisualDriver};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::Duration;

#[cfg(target_os = "macos")]
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputCollisionPolicy {
    Ignore,
    Pause,
    Abort,
}

impl InputCollisionPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ignore => "ignore",
            Self::Pause => "pause",
            Self::Abort => "abort",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ExecutionOptions {
    pub enforce_browser_focus: bool,
    pub input_collision_policy: InputCollisionPolicy,
}

impl Default for ExecutionOptions {
    fn default() -> Self {
        Self {
            enforce_browser_focus: false,
            input_collision_policy: InputCollisionPolicy::Ignore,
        }
    }
}

impl ExecutionOptions {
    pub fn strict() -> Self {
        Self {
            enforce_browser_focus: true,
            input_collision_policy: InputCollisionPolicy::Abort,
        }
    }

    pub fn test() -> Self {
        Self {
            enforce_browser_focus: true,
            input_collision_policy: InputCollisionPolicy::Pause,
        }
    }

    pub fn fast() -> Self {
        Self {
            enforce_browser_focus: false,
            input_collision_policy: InputCollisionPolicy::Ignore,
        }
    }
}

#[path = "execution_controller/support.rs"]
mod support;

use support::*;

#[cfg(test)]
#[path = "execution_controller/tests.rs"]
mod tests;

#[path = "execution_controller/execution.rs"]
mod execution;

pub use execution::execute_plan;
