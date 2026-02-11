// Environment Observers - Monitor emails, files, system events

pub mod email_observer;
pub mod file_observer;
pub mod system_observer;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Observation from any observer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub source: String,
    pub event_type: String,
    pub priority: Priority,
    pub data: Value,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Medium,
    High,
    Urgent,
}

/// Trait for all observers
#[async_trait::async_trait]
pub trait Observer: Send + Sync {
    async fn observe(&self) -> Result<Vec<Observation>>;
    fn name(&self) -> &str;
}

/// Observer manager - Coordinates all observers
pub struct ObserverManager {
    observers: Vec<Box<dyn Observer>>,
}

impl ObserverManager {
    pub fn new() -> Self {
        Self {
            observers: Vec::new(),
        }
    }

    pub fn register(&mut self, observer: Box<dyn Observer>) {
        log::info!("?諭?Registered observer: {}", observer.name());
        self.observers.push(observer);
    }

    /// Collect observations from all observers
    pub async fn collect_all(&self) -> Result<Vec<Observation>> {
        let mut all_observations = Vec::new();

        for observer in &self.observers {
            match observer.observe().await {
                Ok(obs) => {
                    log::debug!("Collected {} observations from {}", obs.len(), observer.name());
                    all_observations.extend(obs);
                }
                Err(e) => {
                    log::warn!("Observer {} failed: {}", observer.name(), e);
                }
            }
        }

        // Sort by priority and timestamp
        all_observations.sort_by(|a, b| {
            let priority_cmp = priority_value(&b.priority).cmp(&priority_value(&a.priority));
            if priority_cmp == std::cmp::Ordering::Equal {
                b.timestamp.cmp(&a.timestamp)
            } else {
                priority_cmp
            }
        });

        Ok(all_observations)
    }
}

fn priority_value(p: &Priority) -> u8 {
    match p {
        Priority::Urgent => 4,
        Priority::High => 3,
        Priority::Medium => 2,
        Priority::Low => 1,
    }
}
