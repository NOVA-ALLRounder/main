// Event Bus for pub/sub pattern (Clawdbot-inspired)

use crate::jarvis::models::JarvisEvent;
use anyhow::Result;
use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

pub type EventHandlerId = usize;

#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: &JarvisEvent) -> Result<()>;
}

#[derive(Clone)]
pub struct EventBus {
    handlers: Arc<DashMap<EventHandlerId, Arc<dyn EventHandler>>>,
    tx: mpsc::UnboundedSender<JarvisEvent>,
    next_id: Arc<std::sync::atomic::AtomicUsize>,
}

impl EventBus {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<JarvisEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();

        let bus = Self {
            handlers: Arc::new(DashMap::new()),
            tx,
            next_id: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        };

        (bus, rx)
    }

    /// Subscribe to events with a handler
    pub fn subscribe(&self, handler: Arc<dyn EventHandler>) -> EventHandlerId {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.handlers.insert(id, handler);
        id
    }

    /// Unsubscribe a handler
    pub fn unsubscribe(&self, id: EventHandlerId) {
        self.handlers.remove(&id);
    }

    /// Publish an event to all handlers
    pub fn publish(&self, event: JarvisEvent) {
        if let Err(e) = self.tx.send(event) {
            log::error!("Failed to publish event: {}", e);
        }
    }

    /// Process events from the receiver
    pub async fn process_events(&self, event: JarvisEvent) {
        // Clone handlers to avoid holding the lock
        let handlers: Vec<_> = self.handlers.iter().map(|entry| entry.value().clone()).collect();

        for handler in handlers {
            if let Err(e) = handler.handle(&event).await {
                log::error!("Event handler error: {}", e);
            }
        }
    }

    /// Get subscriber count
    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new().0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TestHandler {
        call_count: Arc<AtomicU32>,
    }

    #[async_trait]
    impl EventHandler for TestHandler {
        async fn handle(&self, _event: &JarvisEvent) -> Result<()> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_event_bus() {
        let (bus, mut rx) = EventBus::new();
        let call_count = Arc::new(AtomicU32::new(0));

        let handler = Arc::new(TestHandler {
            call_count: call_count.clone(),
        });

        let _id = bus.subscribe(handler);

        // Publish event
        bus.publish(JarvisEvent::CommandReceived {
            command: "test".to_string(),
            source: "test".to_string(),
        });

        // Process event
        if let Some(event) = rx.recv().await {
            bus.process_events(event).await;
        }

        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }
}
