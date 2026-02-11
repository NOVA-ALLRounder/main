// Event Store - Complete audit trail with event sourcing
//
// Event Sourcing Pattern:
// - All state changes are captured as immutable events
// - Events are append-only (never updated or deleted)
// - Current state can be reconstructed by replaying events
// - Perfect audit trail for compliance (GDPR, SOC2, etc.)

pub mod store;
pub mod events;
pub mod replay;

pub use store::{EventStore, EventStoreConfig};
pub use events::{DomainEvent, EventMetadata, EventType};
pub use replay::{EventReplay, ReplayOptions};

use anyhow::Result;

/// Initialize event store
pub async fn init_event_store(config: EventStoreConfig) -> Result<EventStore> {
    EventStore::new(config).await
}

/// Shutdown event store gracefully
pub async fn shutdown_event_store(store: &EventStore) -> Result<()> {
    store.flush().await
}
