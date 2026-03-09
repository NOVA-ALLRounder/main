use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use super::ExecutionContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStep {
    pub name: String,
    pub tool: String,
    pub params: HashMap<String, String>,
    #[serde(default)]
    pub extract: HashMap<String, String>,
    #[serde(default)]
    pub on_fail: FailAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FailAction {
    #[default]
    Stop,
    Continue,
    Retry,
    Skip,
}

#[derive(Debug, Clone)]
pub struct ToolChain {
    pub name: String,
    pub steps: Vec<ToolStep>,
    pub context: ExecutionContext,
}

impl ToolChain {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            steps: Vec::new(),
            context: ExecutionContext::new(),
        }
    }

    pub fn add_step(&mut self, step: ToolStep) {
        self.steps.push(step);
    }

    pub fn from_json(json: &Value) -> Result<Self> {
        let name = json["name"].as_str().unwrap_or("unnamed");
        let mut chain = Self::new(name);

        if let Some(steps) = json["steps"].as_array() {
            for step_json in steps {
                let step: ToolStep = serde_json::from_value(step_json.clone())?;
                chain.add_step(step);
            }
        }

        Ok(chain)
    }
}
