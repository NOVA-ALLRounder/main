// Domain Events - Immutable records of what happened

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Event ID (globally unique)
pub type EventId = String;

/// Aggregate ID (entity this event belongs to)
pub type AggregateId = String;

/// Event version number (for optimistic locking)
pub type EventVersion = u64;

/// Domain event types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventType {
    // Command events
    CommandReceived {
        command: String,
        source: String,
    },
    CommandParsed {
        skill: String,
        action: String,
    },
    CommandExecuted {
        skill: String,
        action: String,
        success: bool,
    },
    CommandFailed {
        skill: String,
        action: String,
        error: String,
    },

    // Skill events
    SkillRegistered {
        skill_name: String,
    },
    SkillExecutionStarted {
        skill: String,
        action: String,
        session_key: String,
    },
    SkillExecutionCompleted {
        skill: String,
        action: String,
        success: bool,
        duration_ms: u64,
    },

    // Policy events
    PolicyCheckPerformed {
        policy: String,
        action: String,
        allowed: bool,
    },
    ApprovalRequested {
        action: String,
        approval_id: String,
    },
    ApprovalGranted {
        approval_id: String,
        approver: String,
    },
    ApprovalRejected {
        approval_id: String,
        rejector: String,
        reason: String,
    },

    // LLM events
    LlmCallStarted {
        model: String,
        prompt_tokens: Option<u32>,
    },
    LlmCallCompleted {
        model: String,
        prompt_tokens: u32,
        completion_tokens: u32,
        duration_ms: u64,
    },
    LlmCallFailed {
        model: String,
        error: String,
    },

    // Session events
    SessionCreated {
        session_key: String,
        user_id: Option<String>,
    },
    SessionEnded {
        session_key: String,
        duration_ms: u64,
    },

    // System events
    SystemStarted {
        version: String,
    },
    SystemShutdown {
        reason: String,
    },
    HealthCheckPerformed {
        component: String,
        healthy: bool,
    },

    // Pattern events
    PatternDetected {
        pattern_type: String,
        confidence: f64,
    },
    RecommendationGenerated {
        recommendation_id: String,
        title: String,
        confidence: f64,
    },
    RecommendationApproved {
        recommendation_id: String,
    },
    RecommendationRejected {
        recommendation_id: String,
    },

    // User events (for GDPR compliance)
    UserDataExported {
        user_id: String,
        export_path: String,
    },
    UserDataErased {
        user_id: String,
        erased_records: u64,
    },
}

/// Event metadata (envelope)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Unique event ID
    pub event_id: EventId,

    /// ID of the aggregate this event belongs to
    pub aggregate_id: AggregateId,

    /// Type of aggregate (e.g., "command", "skill", "session")
    pub aggregate_type: String,

    /// Event version (for optimistic locking)
    pub version: EventVersion,

    /// When this event occurred
    pub occurred_at: DateTime<Utc>,

    /// Who/what caused this event
    pub causation_id: Option<EventId>, // ID of event that caused this event
    pub correlation_id: Option<String>, // ID linking related events together
    pub user_id: Option<String>,

    /// Additional context
    pub metadata: HashMap<String, serde_json::Value>,
}

impl EventMetadata {
    pub fn new(aggregate_id: impl Into<String>, aggregate_type: impl Into<String>) -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            aggregate_id: aggregate_id.into(),
            aggregate_type: aggregate_type.into(),
            version: 1,
            occurred_at: Utc::now(),
            causation_id: None,
            correlation_id: None,
            user_id: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_version(mut self, version: EventVersion) -> Self {
        self.version = version;
        self
    }

    pub fn with_causation(mut self, causation_id: impl Into<String>) -> Self {
        self.causation_id = Some(causation_id.into());
        self
    }

    pub fn with_correlation(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Complete domain event (metadata + payload)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEvent {
    #[serde(flatten)]
    pub metadata: EventMetadata,
    pub event_type: EventType,
}

impl DomainEvent {
    pub fn new(metadata: EventMetadata, event_type: EventType) -> Self {
        Self {
            metadata,
            event_type,
        }
    }

    /// Quick constructor for simple events
    pub fn simple(
        aggregate_id: impl Into<String>,
        aggregate_type: impl Into<String>,
        event_type: EventType,
    ) -> Self {
        Self {
            metadata: EventMetadata::new(aggregate_id, aggregate_type),
            event_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_metadata_creation() {
        let meta = EventMetadata::new("cmd-123", "command")
            .with_version(5)
            .with_user("user-456")
            .with_correlation("session-789");

        assert_eq!(meta.aggregate_id, "cmd-123");
        assert_eq!(meta.aggregate_type, "command");
        assert_eq!(meta.version, 5);
        assert_eq!(meta.user_id, Some("user-456".to_string()));
        assert_eq!(meta.correlation_id, Some("session-789".to_string()));
    }

    #[test]
    fn test_domain_event_serialization() {
        let event = DomainEvent::simple(
            "skill-exec-1",
            "skill_execution",
            EventType::SkillExecutionStarted {
                skill: "computer_use".to_string(),
                action: "screenshot".to_string(),
                session_key: "session-123".to_string(),
            },
        );

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: DomainEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(event.metadata.aggregate_id, deserialized.metadata.aggregate_id);
    }

    #[test]
    fn test_event_types() {
        let events = vec![
            EventType::CommandReceived {
                command: "test".to_string(),
                source: "cli".to_string(),
            },
            EventType::SkillExecutionCompleted {
                skill: "email".to_string(),
                action: "send".to_string(),
                success: true,
                duration_ms: 1500,
            },
            EventType::ApprovalGranted {
                approval_id: "apr-123".to_string(),
                approver: "user-1".to_string(),
            },
        ];

        for event_type in events {
            let json = serde_json::to_string(&event_type).unwrap();
            let _deserialized: EventType = serde_json::from_str(&json).unwrap();
        }
    }
}
