// Event Replay - Reconstruct state from events

use super::events::DomainEvent;
use super::store::EventStore;
use anyhow::Result;
use serde_json::Value;

/// Options for event replay
#[derive(Debug, Clone)]
pub struct ReplayOptions {
    /// Maximum number of events to replay
    pub max_events: Option<usize>,

    /// Start from specific version
    pub from_version: Option<u64>,

    /// Stop at specific version
    pub to_version: Option<u64>,

    /// Fast-forward mode (skip intermediate states)
    pub fast_forward: bool,
}

impl Default for ReplayOptions {
    fn default() -> Self {
        Self {
            max_events: None,
            from_version: None,
            to_version: None,
            fast_forward: false,
        }
    }
}

/// Event replay engine
pub struct EventReplay {
    store: EventStore,
}

impl EventReplay {
    pub fn new(store: EventStore) -> Self {
        Self { store }
    }

    /// Replay events for an aggregate and reconstruct state
    pub async fn replay_aggregate(
        &self,
        aggregate_id: &str,
        options: ReplayOptions,
    ) -> Result<ReplayResult> {
        let mut events = self.store.get_events(aggregate_id).await?;

        // Apply filters
        if let Some(from_version) = options.from_version {
            events.retain(|e| e.metadata.version >= from_version);
        }
        if let Some(to_version) = options.to_version {
            events.retain(|e| e.metadata.version <= to_version);
        }
        if let Some(max_events) = options.max_events {
            events.truncate(max_events);
        }

        let total_events = events.len();
        let start_time = std::time::Instant::now();

        // Apply events to reconstruct state
        let mut state = serde_json::json!({});
        for event in &events {
            state = self.apply_event(&state, event)?;
        }

        let duration = start_time.elapsed();

        Ok(ReplayResult {
            aggregate_id: aggregate_id.to_string(),
            events_replayed: total_events,
            final_state: state,
            duration_ms: duration.as_millis() as u64,
        })
    }

    /// Replay correlated events (e.g., entire session)
    pub async fn replay_correlation(
        &self,
        correlation_id: &str,
        options: ReplayOptions,
    ) -> Result<CorrelationReplayResult> {
        let mut events = self.store.get_correlated_events(correlation_id).await?;

        if let Some(max_events) = options.max_events {
            events.truncate(max_events);
        }

        let total_events = events.len();
        let start_time = std::time::Instant::now();

        // Group events by aggregate
        let mut aggregates: std::collections::HashMap<String, Vec<DomainEvent>> =
            std::collections::HashMap::new();

        for event in events {
            aggregates
                .entry(event.metadata.aggregate_id.clone())
                .or_insert_with(Vec::new)
                .push(event);
        }

        // Replay each aggregate
        let mut aggregate_states = std::collections::HashMap::new();
        for (aggregate_id, aggregate_events) in aggregates {
            let mut state = serde_json::json!({});
            for event in &aggregate_events {
                state = self.apply_event(&state, event)?;
            }
            aggregate_states.insert(aggregate_id, state);
        }

        let duration = start_time.elapsed();

        Ok(CorrelationReplayResult {
            correlation_id: correlation_id.to_string(),
            events_replayed: total_events,
            aggregates: aggregate_states,
            duration_ms: duration.as_millis() as u64,
        })
    }

    /// Apply a single event to state
    fn apply_event(&self, current_state: &Value, event: &DomainEvent) -> Result<Value> {
        use super::events::EventType;

        let mut state = current_state.clone();

        // Update state based on event type
        match &event.event_type {
            EventType::CommandReceived { command, source } => {
                state["last_command"] = serde_json::json!(command);
                state["command_source"] = serde_json::json!(source);
            }
            EventType::CommandExecuted { success, .. } => {
                state["last_execution_success"] = serde_json::json!(success);
            }
            EventType::SkillExecutionCompleted {
                skill,
                action,
                success,
                duration_ms,
            } => {
                if state.get("executions").is_none() {
                    state["executions"] = serde_json::json!([]);
                }
                if let Some(execs) = state["executions"].as_array_mut() {
                    execs.push(serde_json::json!({
                        "skill": skill,
                        "action": action,
                        "success": success,
                        "duration_ms": duration_ms,
                    }));
                }
            }
            EventType::SessionCreated {
                session_key,
                user_id,
            } => {
                state["session_key"] = serde_json::json!(session_key);
                state["user_id"] = serde_json::json!(user_id);
                state["session_active"] = serde_json::json!(true);
            }
            EventType::SessionEnded { duration_ms, .. } => {
                state["session_active"] = serde_json::json!(false);
                state["total_duration_ms"] = serde_json::json!(duration_ms);
            }
            _ => {
                // Generic: store event type
                if state.get("event_count").is_none() {
                    state["event_count"] = serde_json::json!(0);
                }
                if let Some(count) = state["event_count"].as_u64() {
                    state["event_count"] = serde_json::json!(count + 1);
                }
            }
        }

        // Update metadata
        state["version"] = serde_json::json!(event.metadata.version);
        state["last_updated"] = serde_json::json!(event.metadata.occurred_at.to_rfc3339());

        Ok(state)
    }

    /// Export all events as JSON (for audit/backup)
    pub async fn export_events(
        &self,
        aggregate_id: &str,
        output_path: &std::path::Path,
    ) -> Result<usize> {
        let events = self.store.get_events(aggregate_id).await?;
        let json = serde_json::to_string_pretty(&events)?;

        std::fs::write(output_path, json)?;

        log::info!(
            "?踰 Exported {} events to {:?}",
            events.len(),
            output_path
        );

        Ok(events.len())
    }
}

/// Result of replaying events for a single aggregate
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReplayResult {
    pub aggregate_id: String,
    pub events_replayed: usize,
    pub final_state: Value,
    pub duration_ms: u64,
}

/// Result of replaying correlated events
#[derive(Debug, Clone, serde::Serialize)]
pub struct CorrelationReplayResult {
    pub correlation_id: String,
    pub events_replayed: usize,
    pub aggregates: std::collections::HashMap<String, Value>,
    pub duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::event_store::{
        events::{EventMetadata, EventType},
        store::EventStoreConfig,
    };
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_replay_aggregate() {
        let config = EventStoreConfig {
            database_path: PathBuf::from(":memory:"),
            enable_snapshots: false,
            snapshot_interval: 100,
        };

        let store = EventStore::new(config).await.unwrap();
        let replay_engine = EventReplay::new(store.clone());

        // Create events
        let session_id = "session-123";
        let events = vec![
            DomainEvent::new(
                EventMetadata::new(session_id, "session").with_version(1),
                EventType::SessionCreated {
                    session_key: session_id.to_string(),
                    user_id: Some("user-1".to_string()),
                },
            ),
            DomainEvent::new(
                EventMetadata::new(session_id, "session").with_version(2),
                EventType::SessionEnded {
                    session_key: session_id.to_string(),
                    duration_ms: 5000,
                },
            ),
        ];

        for event in events {
            store.append(&event).await.unwrap();
        }

        // Replay
        let result = replay_engine
            .replay_aggregate(session_id, ReplayOptions::default())
            .await
            .unwrap();

        assert_eq!(result.events_replayed, 2);
        assert_eq!(result.final_state["session_active"], false);
        assert_eq!(result.final_state["total_duration_ms"], 5000);
    }
}
