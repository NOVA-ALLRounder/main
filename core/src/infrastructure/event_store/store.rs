// Event Store - Append-only storage for domain events

use super::events::{AggregateId, DomainEvent, EventVersion};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

/// Event store configuration
#[derive(Debug, Clone)]
pub struct EventStoreConfig {
    pub database_path: PathBuf,
    pub enable_snapshots: bool,
    pub snapshot_interval: u64, // Create snapshot every N events
}

impl Default for EventStoreConfig {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self {
            database_path: PathBuf::from(home).join(".steer/event_store.db"),
            enable_snapshots: true,
            snapshot_interval: 100,
        }
    }
}

/// Event store - Append-only storage
#[derive(Clone)]
pub struct EventStore {
    _config: EventStoreConfig,
    conn: Arc<Mutex<Connection>>,
    _snapshot_cache: Arc<RwLock<std::collections::HashMap<AggregateId, Snapshot>>>,
}

/// Snapshot of aggregate state at a point in time
#[derive(Debug, Clone)]
struct Snapshot {
    _aggregate_id: AggregateId,
    _version: EventVersion,
    _state: serde_json::Value,
    _created_at: DateTime<Utc>,
}

impl EventStore {
    /// Create new event store
    pub async fn new(config: EventStoreConfig) -> Result<Self> {
        // Ensure directory exists
        if let Some(parent) = config.database_path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create event store directory")?;
        }

        // Open database connection
        let conn = Connection::open(&config.database_path)
            .context("Failed to open event store database")?;

        // Create schema
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS events (
                event_id TEXT PRIMARY KEY,
                aggregate_id TEXT NOT NULL,
                aggregate_type TEXT NOT NULL,
                version INTEGER NOT NULL,
                occurred_at TEXT NOT NULL,
                causation_id TEXT,
                correlation_id TEXT,
                user_id TEXT,
                event_type TEXT NOT NULL,
                event_data TEXT NOT NULL,
                metadata TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(aggregate_id, version)
            )
            "#,
            [],
        )?;

        // Create indexes
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_events_aggregate ON events(aggregate_id, version)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_events_correlation ON events(correlation_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_events_occurred ON events(occurred_at)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_events_user ON events(user_id)",
            [],
        )?;

        // Snapshots table (optional optimization)
        if config.enable_snapshots {
            conn.execute(
                r#"
                CREATE TABLE IF NOT EXISTS snapshots (
                    aggregate_id TEXT PRIMARY KEY,
                    version INTEGER NOT NULL,
                    state TEXT NOT NULL,
                    created_at TEXT NOT NULL
                )
                "#,
                [],
            )?;
        }

        log::info!("??Event store initialized at {:?}", config.database_path);

        Ok(Self {
            _config: config,
            conn: Arc::new(Mutex::new(conn)),
            _snapshot_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        })
    }

    /// Append new event to the store
    pub async fn append(&self, event: &DomainEvent) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        let event_data = serde_json::to_string(&event.event_type)?;
        let metadata = serde_json::to_string(&event.metadata.metadata)?;

        conn.execute(
            r#"
            INSERT INTO events (
                event_id, aggregate_id, aggregate_type, version,
                occurred_at, causation_id, correlation_id, user_id,
                event_type, event_data, metadata
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                event.metadata.event_id,
                event.metadata.aggregate_id,
                event.metadata.aggregate_type,
                event.metadata.version as i64,
                event.metadata.occurred_at.to_rfc3339(),
                event.metadata.causation_id,
                event.metadata.correlation_id,
                event.metadata.user_id,
                format!("{:?}", event.event_type), // Simplified type name
                event_data,
                metadata,
            ],
        ).context("Failed to append event")?;

        // Update metrics
        crate::shared::telemetry::metrics::EVENTS_PROCESSED
            .with_label_values(&[&event.metadata.aggregate_type, "event_store"])
            .inc();

        Ok(())
    }

    /// Get all events for an aggregate
    pub async fn get_events(&self, aggregate_id: &str) -> Result<Vec<DomainEvent>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            r#"
            SELECT event_id, aggregate_id, aggregate_type, version,
                   occurred_at, causation_id, correlation_id, user_id,
                   event_data, metadata
            FROM events
            WHERE aggregate_id = ?1
            ORDER BY version ASC
            "#,
        )?;

        let events = stmt
            .query_map(params![aggregate_id], |row| {
                let event_data: String = row.get(8)?;
                let metadata_str: String = row.get(9)?;

                let event_type: super::events::EventType = serde_json::from_str(&event_data)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

                let metadata_map: std::collections::HashMap<String, serde_json::Value> =
                    serde_json::from_str(&metadata_str)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

                let occurred_at_str: String = row.get(4)?;
                let occurred_at = DateTime::parse_from_rfc3339(&occurred_at_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
                    .with_timezone(&Utc);

                Ok(DomainEvent {
                    metadata: super::events::EventMetadata {
                        event_id: row.get(0)?,
                        aggregate_id: row.get(1)?,
                        aggregate_type: row.get(2)?,
                        version: row.get::<_, i64>(3)? as u64,
                        occurred_at,
                        causation_id: row.get(5)?,
                        correlation_id: row.get(6)?,
                        user_id: row.get(7)?,
                        metadata: metadata_map,
                    },
                    event_type,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Get events by correlation ID (all related events)
    pub async fn get_correlated_events(&self, correlation_id: &str) -> Result<Vec<DomainEvent>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            r#"
            SELECT event_id, aggregate_id, aggregate_type, version,
                   occurred_at, causation_id, correlation_id, user_id,
                   event_data, metadata
            FROM events
            WHERE correlation_id = ?1
            ORDER BY occurred_at ASC
            "#,
        )?;

        let events = stmt
            .query_map(params![correlation_id], |row| {
                let event_data: String = row.get(8)?;
                let metadata_str: String = row.get(9)?;

                let event_type: super::events::EventType = serde_json::from_str(&event_data)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

                let metadata_map: std::collections::HashMap<String, serde_json::Value> =
                    serde_json::from_str(&metadata_str)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

                let occurred_at_str: String = row.get(4)?;
                let occurred_at = DateTime::parse_from_rfc3339(&occurred_at_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
                    .with_timezone(&Utc);

                Ok(DomainEvent {
                    metadata: super::events::EventMetadata {
                        event_id: row.get(0)?,
                        aggregate_id: row.get(1)?,
                        aggregate_type: row.get(2)?,
                        version: row.get::<_, i64>(3)? as u64,
                        occurred_at,
                        causation_id: row.get(5)?,
                        correlation_id: row.get(6)?,
                        user_id: row.get(7)?,
                        metadata: metadata_map,
                    },
                    event_type,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Get all events for a user (GDPR compliance)
    pub async fn get_user_events(&self, user_id: &str) -> Result<Vec<DomainEvent>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            r#"
            SELECT event_id, aggregate_id, aggregate_type, version,
                   occurred_at, causation_id, correlation_id, user_id,
                   event_data, metadata
            FROM events
            WHERE user_id = ?1
            ORDER BY occurred_at ASC
            "#,
        )?;

        let events = stmt
            .query_map(params![user_id], |row| {
                let event_data: String = row.get(8)?;
                let metadata_str: String = row.get(9)?;

                let event_type: super::events::EventType = serde_json::from_str(&event_data)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

                let metadata_map: std::collections::HashMap<String, serde_json::Value> =
                    serde_json::from_str(&metadata_str)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

                let occurred_at_str: String = row.get(4)?;
                let occurred_at = DateTime::parse_from_rfc3339(&occurred_at_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
                    .with_timezone(&Utc);

                Ok(DomainEvent {
                    metadata: super::events::EventMetadata {
                        event_id: row.get(0)?,
                        aggregate_id: row.get(1)?,
                        aggregate_type: row.get(2)?,
                        version: row.get::<_, i64>(3)? as u64,
                        occurred_at,
                        causation_id: row.get(5)?,
                        correlation_id: row.get(6)?,
                        user_id: row.get(7)?,
                        metadata: metadata_map,
                    },
                    event_type,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Delete all events for a user (GDPR right to erasure)
    pub async fn erase_user_data(&self, user_id: &str) -> Result<u64> {
        let conn = self.conn.lock().unwrap();

        let deleted = conn.execute("DELETE FROM events WHERE user_id = ?1", params![user_id])?;

        log::info!("?肉딀닼? Erased {} events for user {}", deleted, user_id);

        Ok(deleted as u64)
    }

    /// Flush any pending writes
    pub async fn flush(&self) -> Result<()> {
        // SQLite auto-commits, so nothing to do here
        // But this hook is useful for other backends (Kafka, etc.)
        Ok(())
    }

    /// Get event count
    pub async fn count(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;
        Ok(count as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::event_store::events::{EventMetadata, EventType};

    #[tokio::test]
    async fn test_event_store_lifecycle() {
        let config = EventStoreConfig {
            database_path: PathBuf::from(":memory:"),
            enable_snapshots: false,
            snapshot_interval: 100,
        };

        let store = EventStore::new(config).await.unwrap();

        // Append event
        let event = DomainEvent::simple(
            "cmd-1",
            "command",
            EventType::CommandReceived {
                command: "test".to_string(),
                source: "test".to_string(),
            },
        );

        store.append(&event).await.unwrap();

        // Retrieve events
        let events = store.get_events("cmd-1").await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].metadata.aggregate_id, "cmd-1");

        // Count
        let count = store.count().await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_correlated_events() {
        let config = EventStoreConfig {
            database_path: PathBuf::from(":memory:"),
            enable_snapshots: false,
            snapshot_interval: 100,
        };

        let store = EventStore::new(config).await.unwrap();

        let correlation_id = "session-123";

        // Append multiple correlated events
        for i in 0..3 {
            let event = DomainEvent::new(
                EventMetadata::new(format!("cmd-{}", i), "command")
                    .with_correlation(correlation_id),
                EventType::CommandReceived {
                    command: format!("test-{}", i),
                    source: "test".to_string(),
                },
            );
            store.append(&event).await.unwrap();
        }

        // Retrieve correlated events
        let events = store.get_correlated_events(correlation_id).await.unwrap();
        assert_eq!(events.len(), 3);
    }
}
