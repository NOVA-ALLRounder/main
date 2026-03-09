use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};

use super::admin::*;
use super::agent::*;
use super::chat::*;
use super::chat_support::*;
use super::operational::*;
use super::recommendations::*;
use super::AppState;
use crate::nl_automation::{IntentType, Plan, PlanStep, StepType};
use crate::release_gate;
use serde_json::json;
use serial_test::serial;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn reset_memory_tables() {
    crate::db::clear_request_memory_for_tests();
    crate::db::clear_execution_memory_for_tests();
    crate::db::clear_memory_admin_events_for_tests();
    crate::db::clear_launch_ops_events_for_tests();
    crate::db::clear_nl_runs_for_tests();
}

fn test_plan(intent: IntentType) -> Plan {
    Plan {
        plan_id: format!("plan-{}", uuid::Uuid::new_v4()),
        intent,
        slots: HashMap::new(),
        steps: vec![PlanStep {
            step_id: "extract-1".to_string(),
            step_type: StepType::Extract,
            description: "Extract final result".to_string(),
            data: json!({}),
        }],
    }
}

fn test_plan_with_descriptions(intent: IntentType, descriptions: &[&str]) -> Plan {
    Plan {
        plan_id: format!("plan-{}", uuid::Uuid::new_v4()),
        intent,
        slots: HashMap::new(),
        steps: descriptions
            .iter()
            .enumerate()
            .map(|(idx, desc)| PlanStep {
                step_id: format!("step-{}", idx + 1),
                step_type: StepType::Extract,
                description: (*desc).to_string(),
                data: json!({}),
            })
            .collect(),
    }
}

mod business;
mod chat;
mod contracts;
mod memory;
mod operational;
mod recommendations;
