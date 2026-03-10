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
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn reset_memory_tables() {
    crate::db::clear_request_memory_for_tests();
    crate::db::clear_execution_memory_for_tests();
    crate::db::clear_memory_admin_events_for_tests();
    crate::db::clear_launch_ops_events_for_tests();
    crate::db::clear_nl_runs_for_tests();
}

struct TestEnvGuard {
    entries: Vec<(String, Option<String>)>,
}

impl TestEnvGuard {
    fn capture(keys: &[&str]) -> Self {
        Self {
            entries: keys
                .iter()
                .map(|key| (key.to_string(), std::env::var(key).ok()))
                .collect(),
        }
    }
}

impl Drop for TestEnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.entries.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

async fn bind_test_listener_or_skip(context: &str) -> Option<tokio::net::TcpListener> {
    match tokio::net::TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => Some(listener),
        Err(error) if error.kind() == ErrorKind::PermissionDenied => {
            eprintln!(
                "skipping {context}: loopback listener bind is not permitted in this environment"
            );
            None
        }
        Err(error) => panic!("bind test listener for {context}: {error}"),
    }
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
