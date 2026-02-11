// Load Testing - High event volume processing

#[cfg(test)]
mod load_tests {
    use local_os_agent::infrastructure::event_store::{
        EventStore, EventStoreConfig, DomainEvent, EventMetadata, EventType,
    };
    use std::path::PathBuf;
    use std::time::Instant;

    #[tokio::test]
    #[ignore] // Run with: cargo test --test load_test -- --ignored
    async fn test_high_volume_event_ingestion() {
        let config = EventStoreConfig {
            database_path: PathBuf::from("/tmp/steer_load_test.db"),
            enable_snapshots: true,
            snapshot_interval: 100,
        };

        let store = EventStore::new(config).await.unwrap();

        let num_events = 10_000;
        let start = Instant::now();

        // Ingest 10k events
        for i in 0..num_events {
            let event = DomainEvent::simple(
                format!("aggregate-{}", i % 1000), // 1000 aggregates
                "test",
                EventType::CommandReceived {
                    command: format!("test-{}", i),
                    source: "load_test".to_string(),
                },
            );

            store.append(&event).await.unwrap();
        }

        let duration = start.elapsed();
        let throughput = num_events as f64 / duration.as_secs_f64();

        println!("??Ingested {} events in {:?}", num_events, duration);
        println!("?諭?Throughput: {:.2} events/sec", throughput);

        // Target: > 1000 events/sec
        assert!(throughput > 1000.0, "Throughput too low: {}", throughput);

        // Cleanup
        std::fs::remove_file("/tmp/steer_load_test.db").ok();
    }

    #[tokio::test]
    #[ignore]
    async fn test_concurrent_event_writes() {
        use tokio::task;

        let config = EventStoreConfig {
            database_path: PathBuf::from("/tmp/steer_concurrent_test.db"),
            enable_snapshots: false,
            snapshot_interval: 100,
        };

        let store = EventStore::new(config).await.unwrap();
        let store = std::sync::Arc::new(store);

        let num_tasks = 10;
        let events_per_task = 1000;
        let start = Instant::now();

        let mut handles = vec![];

        for task_id in 0..num_tasks {
            let store_clone = store.clone();
            let handle = task::spawn(async move {
                for i in 0..events_per_task {
                    let event = DomainEvent::simple(
                        format!("task-{}-event-{}", task_id, i),
                        "concurrent_test",
                        EventType::CommandReceived {
                            command: format!("task-{}-cmd-{}", task_id, i),
                            source: "concurrent_test".to_string(),
                        },
                    );

                    store_clone.append(&event).await.unwrap();
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }

        let duration = start.elapsed();
        let total_events = num_tasks * events_per_task;
        let throughput = total_events as f64 / duration.as_secs_f64();

        println!(
            "??{} concurrent tasks wrote {} events in {:?}",
            num_tasks, total_events, duration
        );
        println!("?諭?Concurrent throughput: {:.2} events/sec", throughput);

        // Verify count
        let count = store.count().await.unwrap();
        assert_eq!(count, total_events as u64);

        // Cleanup
        std::fs::remove_file("/tmp/steer_concurrent_test.db").ok();
    }

    #[tokio::test]
    #[ignore]
    async fn test_replay_performance() {
        use local_os_agent::infrastructure::event_store::{EventReplay, ReplayOptions};

        let config = EventStoreConfig {
            database_path: PathBuf::from("/tmp/steer_replay_test.db"),
            enable_snapshots: false,
            snapshot_interval: 100,
        };

        let store = EventStore::new(config).await.unwrap();

        // Create aggregate with 1000 events
        let aggregate_id = "perf-test-aggregate";
        for i in 0..1000 {
            let event = DomainEvent::new(
                EventMetadata::new(aggregate_id, "test").with_version(i + 1),
                EventType::SkillExecutionCompleted {
                    skill: "test".to_string(),
                    action: "test".to_string(),
                    success: true,
                    duration_ms: 100,
                },
            );
            store.append(&event).await.unwrap();
        }

        // Replay
        let replay_engine = EventReplay::new(store);
        let start = Instant::now();

        let result = replay_engine
            .replay_aggregate(aggregate_id, ReplayOptions::default())
            .await
            .unwrap();

        let duration = start.elapsed();

        println!("??Replayed {} events in {:?}", result.events_replayed, duration);
        println!("?諭?Replay rate: {:.2} events/ms", result.events_replayed as f64 / duration.as_millis() as f64);

        // Target: < 100ms for 1000 events
        assert!(duration.as_millis() < 1000, "Replay too slow: {:?}", duration);

        // Cleanup
        std::fs::remove_file("/tmp/steer_replay_test.db").ok();
    }
}
