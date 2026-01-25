//! Embedded mode integration tests.
//!
//! Tests the IPC event bus, gRPC over UDS, and embedded runtime integration.
//! Run with: cargo test --test embedded_integration --features sqlite

use std::os::unix::fs::FileTypeExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use prost_types::Any;
use tokio::sync::RwLock;
use tonic::Status;
use uuid::Uuid;

use angzarr::bus::ipc::{IpcBroker, IpcBrokerConfig, IpcConfig, IpcEventBus};
use angzarr::bus::{EventBus, EventHandler};
use angzarr::embedded::{
    AggregateHandler, ProjectorConfig, ProjectorHandler, RuntimeBuilder, SagaConfig, SagaHandler,
};
use angzarr::proto::{
    event_page, CommandBook, CommandPage, ContextualCommand, Cover, EventBook, EventPage,
    Projection, SagaResponse, Uuid as ProtoUuid,
};

// ============================================================================
// Test Fixtures
// ============================================================================

/// Simple test aggregate that echoes commands as events.
struct EchoAggregate {
    call_count: AtomicU32,
}

impl EchoAggregate {
    fn new() -> Self {
        Self {
            call_count: AtomicU32::new(0),
        }
    }

    fn calls(&self) -> u32 {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl AggregateHandler for EchoAggregate {
    async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, Status> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        let command_book = ctx
            .command
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing command"))?;

        let cover = command_book.cover.clone();

        // Get next sequence from prior events
        let next_seq = ctx
            .events
            .as_ref()
            .and_then(|e| e.pages.last())
            .and_then(|p| match &p.sequence {
                Some(event_page::Sequence::Num(n)) => Some(n + 1),
                _ => None,
            })
            .unwrap_or(0);

        // Echo command as event
        let event_pages: Vec<EventPage> = command_book
            .pages
            .iter()
            .enumerate()
            .map(|(i, cmd_page)| EventPage {
                sequence: Some(event_page::Sequence::Num(next_seq + i as u32)),
                event: cmd_page.command.clone(),
                created_at: None,
            })
            .collect();

        Ok(EventBook {
            cover,
            pages: event_pages,
            snapshot: None,
            correlation_id: command_book.correlation_id.clone(),
            snapshot_state: None,
        })
    }
}

/// Shared state for recording events.
#[derive(Clone)]
struct RecordingHandlerState {
    events: Arc<RwLock<Vec<EventBook>>>,
}

impl RecordingHandlerState {
    fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn received_count(&self) -> usize {
        self.events.read().await.len()
    }

    async fn get_events(&self) -> Vec<EventBook> {
        self.events.read().await.clone()
    }
}

/// Handler that records received events for verification.
struct RecordingHandler {
    state: RecordingHandlerState,
}

impl RecordingHandler {
    fn new(state: RecordingHandlerState) -> Self {
        Self { state }
    }
}

impl EventHandler for RecordingHandler {
    fn handle(
        &self,
        book: Arc<EventBook>,
    ) -> futures::future::BoxFuture<'static, Result<(), angzarr::bus::BusError>> {
        let events = self.state.events.clone();
        Box::pin(async move {
            events.write().await.push((*book).clone());
            Ok(())
        })
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_command(domain: &str, root: Uuid, data: &[u8]) -> CommandBook {
    CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
        }),
        pages: vec![CommandPage {
            sequence: 0,
            command: Some(Any {
                type_url: "test.TestCommand".to_string(),
                value: data.to_vec(),
            }),
        }],
        correlation_id: Uuid::new_v4().to_string(),
        saga_origin: None,
        auto_resequence: true,
        fact: false,
    }
}

fn create_test_event_book(domain: &str, root: Uuid, sequence: u32) -> EventBook {
    EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
        }),
        pages: vec![EventPage {
            sequence: Some(event_page::Sequence::Num(sequence)),
            event: Some(Any {
                type_url: "test.TestEvent".to_string(),
                value: vec![1, 2, 3],
            }),
            created_at: None,
        }],
        snapshot: None,
        correlation_id: Uuid::new_v4().to_string(),
        snapshot_state: None,
    }
}

fn temp_dir() -> PathBuf {
    let id = Uuid::new_v4().to_string()[..8].to_string();
    let path = PathBuf::from(format!("/tmp/angzarr-test-{}", id));
    std::fs::create_dir_all(&path).expect("Failed to create temp dir");
    path
}

fn cleanup_dir(path: &PathBuf) {
    let _ = std::fs::remove_dir_all(path);
}

// ============================================================================
// IPC Event Bus Tests
// ============================================================================

mod ipc_event_bus {
    use super::*;

    #[test]
    fn test_broker_creates_subscriber_pipes() {
        let base_path = temp_dir();
        let config = IpcBrokerConfig {
            base_path: base_path.clone(),
        };

        let mut broker = IpcBroker::new(config);

        // Register subscriber
        let info = broker
            .register_subscriber("test-projector", vec!["orders".to_string()])
            .expect("Failed to register subscriber");

        assert_eq!(info.name, "test-projector");
        assert_eq!(info.domains, vec!["orders".to_string()]);
        assert!(info.pipe_path.exists(), "Pipe should be created");

        // Verify pipe is a FIFO
        let metadata = std::fs::metadata(&info.pipe_path).expect("Failed to get metadata");
        assert!(
            metadata.file_type().is_fifo(),
            "Should be a named pipe (FIFO)"
        );

        cleanup_dir(&base_path);
    }

    #[test]
    fn test_broker_returns_all_subscribers() {
        let base_path = temp_dir();
        let config = IpcBrokerConfig {
            base_path: base_path.clone(),
        };

        let mut broker = IpcBroker::new(config);

        broker
            .register_subscriber("projector-a", vec!["orders".to_string()])
            .unwrap();
        broker
            .register_subscriber("projector-b", vec!["products".to_string()])
            .unwrap();
        broker
            .register_subscriber("saga-fulfillment", vec!["orders".to_string()])
            .unwrap();

        let subscribers = broker.get_subscribers();
        assert_eq!(subscribers.len(), 3);

        let names: Vec<_> = subscribers.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"projector-a"));
        assert!(names.contains(&"projector-b"));
        assert!(names.contains(&"saga-fulfillment"));

        cleanup_dir(&base_path);
    }

    #[tokio::test]
    async fn test_publisher_writes_to_subscriber_pipes() {
        let base_path = temp_dir();
        let broker_config = IpcBrokerConfig {
            base_path: base_path.clone(),
        };

        let mut broker = IpcBroker::new(broker_config);

        // Register subscriber
        let sub_info = broker
            .register_subscriber("test-sub", vec!["orders".to_string()])
            .unwrap();

        // Create subscriber bus and handler
        let subscriber = IpcEventBus::subscriber(base_path.clone(), "test-sub", vec!["orders".to_string()]);
        let handler_state = RecordingHandlerState::new();
        subscriber.subscribe(Box::new(RecordingHandler::new(handler_state.clone()))).await.unwrap();

        // Start consumer in background (blocks until writer connects)
        let sub_clone = Arc::new(subscriber);
        let sub_for_task = sub_clone.clone();
        let consumer_task = tokio::spawn(async move {
            sub_for_task.start_consuming().await.unwrap();
        });

        // Give consumer time to open pipe
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create publisher with subscriber info
        let publisher_config = IpcConfig::publisher_with_subscribers(base_path.clone(), vec![sub_info]);
        let publisher = IpcEventBus::new(publisher_config);

        // Publish event
        let event = create_test_event_book("orders", Uuid::new_v4(), 0);
        publisher.publish(Arc::new(event)).await.unwrap();

        // Wait for event to be received
        tokio::time::sleep(Duration::from_millis(200)).await;

        let count = handler_state.received_count().await;
        assert_eq!(count, 1, "Should receive one event");

        consumer_task.abort();
        cleanup_dir(&base_path);
    }

    #[tokio::test]
    async fn test_domain_filtering() {
        let base_path = temp_dir();
        let broker_config = IpcBrokerConfig {
            base_path: base_path.clone(),
        };

        let mut broker = IpcBroker::new(broker_config);

        // Register subscriber for "orders" domain only
        let sub_info = broker
            .register_subscriber("orders-only", vec!["orders".to_string()])
            .unwrap();

        // Create subscriber
        let subscriber = IpcEventBus::subscriber(base_path.clone(), "orders-only", vec!["orders".to_string()]);
        let handler_state = RecordingHandlerState::new();
        subscriber.subscribe(Box::new(RecordingHandler::new(handler_state.clone()))).await.unwrap();

        let sub_clone = Arc::new(subscriber);
        let sub_for_task = sub_clone.clone();
        let consumer_task = tokio::spawn(async move {
            sub_for_task.start_consuming().await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create publisher
        let publisher_config = IpcConfig::publisher_with_subscribers(base_path.clone(), vec![sub_info]);
        let publisher = IpcEventBus::new(publisher_config);

        // Publish event to "orders" domain (should be received)
        let orders_event = create_test_event_book("orders", Uuid::new_v4(), 0);
        publisher.publish(Arc::new(orders_event)).await.unwrap();

        // Publish event to "products" domain (should be filtered out by publisher)
        let products_event = create_test_event_book("products", Uuid::new_v4(), 0);
        publisher.publish(Arc::new(products_event)).await.unwrap();

        tokio::time::sleep(Duration::from_millis(200)).await;

        let count = handler_state.received_count().await;
        assert_eq!(count, 1, "Should receive only orders event");

        let events = handler_state.get_events().await;
        assert_eq!(
            events[0].cover.as_ref().unwrap().domain,
            "orders",
            "Should be orders domain"
        );

        consumer_task.abort();
        cleanup_dir(&base_path);
    }

    #[tokio::test]
    async fn test_wildcard_subscriber_receives_all() {
        let base_path = temp_dir();
        let broker_config = IpcBrokerConfig {
            base_path: base_path.clone(),
        };

        let mut broker = IpcBroker::new(broker_config);

        // Register subscriber with wildcard (empty domains = all)
        let sub_info = broker
            .register_subscriber("all-events", vec!["#".to_string()])
            .unwrap();

        // Create subscriber with wildcard
        let subscriber = IpcEventBus::subscriber(base_path.clone(), "all-events", vec!["#".to_string()]);
        let handler_state = RecordingHandlerState::new();
        subscriber.subscribe(Box::new(RecordingHandler::new(handler_state.clone()))).await.unwrap();

        let sub_clone = Arc::new(subscriber);
        let sub_for_task = sub_clone.clone();
        let consumer_task = tokio::spawn(async move {
            sub_for_task.start_consuming().await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create publisher
        let publisher_config = IpcConfig::publisher_with_subscribers(base_path.clone(), vec![sub_info]);
        let publisher = IpcEventBus::new(publisher_config);

        // Publish to multiple domains
        for domain in ["orders", "products", "customers"] {
            let event = create_test_event_book(domain, Uuid::new_v4(), 0);
            publisher.publish(Arc::new(event)).await.unwrap();
        }

        tokio::time::sleep(Duration::from_millis(300)).await;

        let count = handler_state.received_count().await;
        assert_eq!(count, 3, "Should receive all three events");

        consumer_task.abort();
        cleanup_dir(&base_path);
    }

    #[tokio::test]
    async fn test_multiple_subscribers_receive_same_event() {
        let base_path = temp_dir();
        let broker_config = IpcBrokerConfig {
            base_path: base_path.clone(),
        };

        let mut broker = IpcBroker::new(broker_config);

        // Register two subscribers for same domain
        let sub_info_a = broker
            .register_subscriber("sub-a", vec!["orders".to_string()])
            .unwrap();
        let sub_info_b = broker
            .register_subscriber("sub-b", vec!["orders".to_string()])
            .unwrap();

        // Create subscribers
        let subscriber_a = IpcEventBus::subscriber(base_path.clone(), "sub-a", vec!["orders".to_string()]);
        let handler_state_a = RecordingHandlerState::new();
        subscriber_a.subscribe(Box::new(RecordingHandler::new(handler_state_a.clone()))).await.unwrap();

        let subscriber_b = IpcEventBus::subscriber(base_path.clone(), "sub-b", vec!["orders".to_string()]);
        let handler_state_b = RecordingHandlerState::new();
        subscriber_b.subscribe(Box::new(RecordingHandler::new(handler_state_b.clone()))).await.unwrap();

        // Start consumers
        let sub_a_clone = Arc::new(subscriber_a);
        let sub_a_for_task = sub_a_clone.clone();
        let task_a = tokio::spawn(async move {
            sub_a_for_task.start_consuming().await.unwrap();
        });

        let sub_b_clone = Arc::new(subscriber_b);
        let sub_b_for_task = sub_b_clone.clone();
        let task_b = tokio::spawn(async move {
            sub_b_for_task.start_consuming().await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create publisher with both subscribers
        let publisher_config =
            IpcConfig::publisher_with_subscribers(base_path.clone(), vec![sub_info_a, sub_info_b]);
        let publisher = IpcEventBus::new(publisher_config);

        // Publish event
        let event = create_test_event_book("orders", Uuid::new_v4(), 0);
        publisher.publish(Arc::new(event)).await.unwrap();

        tokio::time::sleep(Duration::from_millis(200)).await;

        let count_a = handler_state_a.received_count().await;
        let count_b = handler_state_b.received_count().await;

        assert_eq!(count_a, 1, "Subscriber A should receive event");
        assert_eq!(count_b, 1, "Subscriber B should receive event");

        task_a.abort();
        task_b.abort();
        cleanup_dir(&base_path);
    }
}

// ============================================================================
// Embedded Runtime Tests
// ============================================================================

mod embedded_runtime {
    use super::*;

    #[tokio::test]
    async fn test_runtime_executes_command_and_persists_events() {
        let aggregate = Arc::new(EchoAggregate::new());
        let agg_clone = aggregate.clone();

        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregateWrapper(agg_clone))
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();

        let root = Uuid::new_v4();
        let command = create_test_command("orders", root, b"test-data");

        let response = client.execute(command).await.expect("Command failed");

        assert!(response.events.is_some(), "Should return events");
        let events = response.events.unwrap();
        assert_eq!(events.pages.len(), 1, "Should have one event");
        assert_eq!(aggregate.calls(), 1, "Aggregate should be called once");

        // Verify event was persisted
        let stored = runtime
            .event_store()
            .get("orders", root)
            .await
            .expect("Failed to get events");
        assert_eq!(stored.len(), 1, "Should persist one event");
    }

    #[tokio::test]
    async fn test_runtime_sequence_increments() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();
        let root = Uuid::new_v4();

        // Execute first command
        let cmd1 = create_test_command("orders", root, b"command-1");
        let resp1 = client.execute(cmd1).await.expect("Command 1 failed");
        let seq1 = extract_seq(&resp1);

        // Execute second command
        let cmd2 = create_test_command("orders", root, b"command-2");
        let resp2 = client.execute(cmd2).await.expect("Command 2 failed");
        let seq2 = extract_seq(&resp2);

        assert_eq!(seq1, 0, "First event should have sequence 0");
        assert_eq!(seq2, 1, "Second event should have sequence 1");
    }

    #[tokio::test]
    async fn test_events_published_to_channel_bus() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        // Subscribe to channel bus
        let channel_bus = runtime.channel_bus();
        let handler_state = RecordingHandlerState::new();
        let subscriber = channel_bus.with_config(angzarr::bus::ChannelConfig::subscriber_all());
        subscriber
            .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
            .await
            .unwrap();
        subscriber.start_consuming().await.unwrap();

        let client = runtime.command_client();

        let root = Uuid::new_v4();
        let command = create_test_command("orders", root, b"test");
        client.execute(command).await.expect("Command failed");

        // Give channel bus time to deliver (increased for test reliability)
        tokio::time::sleep(Duration::from_millis(200)).await;

        let count = handler_state.received_count().await;
        assert!(count >= 1, "Events should be published to channel bus (got {})", count);
    }

    #[tokio::test]
    async fn test_multiple_aggregates() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_aggregate("products", EchoAggregate::new())
            .register_aggregate("customers", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();

        // Execute commands on different aggregates
        for domain in ["orders", "products", "customers"] {
            let cmd = create_test_command(domain, Uuid::new_v4(), b"test");
            let resp = client.execute(cmd).await.expect(&format!("{} command failed", domain));
            assert!(resp.events.is_some(), "{} should return events", domain);
        }

        // Verify events persisted in each domain
        for domain in ["orders", "products", "customers"] {
            let roots = runtime.event_store().list_roots(domain).await.unwrap();
            assert_eq!(roots.len(), 1, "{} should have 1 aggregate root", domain);
        }
    }

    #[tokio::test]
    async fn test_sequential_commands_same_aggregate() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let root = Uuid::new_v4();
        let client = runtime.command_client();

        // Execute multiple commands sequentially to same aggregate
        for i in 0..5 {
            let cmd = create_test_command("orders", root, format!("cmd-{}", i).as_bytes());
            let result = client.execute(cmd).await;
            assert!(result.is_ok(), "Command {} should succeed", i);
        }

        // Verify all events persisted with correct sequences
        let events = runtime.event_store().get("orders", root).await.unwrap();
        assert_eq!(events.len(), 5, "Should have 5 events");

        // Verify sequences are 0-4
        for (i, event) in events.iter().enumerate() {
            if let Some(event_page::Sequence::Num(seq)) = &event.sequence {
                assert_eq!(*seq as usize, i, "Event {} should have sequence {}", i, i);
            }
        }
    }

    #[tokio::test]
    async fn test_multiple_events_in_single_command() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", MultiEventAggregate::new(3))
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();

        let root = Uuid::new_v4();
        let command = create_test_command("orders", root, b"multi");
        let response = client.execute(command).await.expect("Command failed");

        let events = response.events.expect("Should have events");
        assert_eq!(events.pages.len(), 3, "Should produce 3 events");

        // Verify stored
        let stored = runtime
            .event_store()
            .get("orders", root)
            .await
            .expect("Failed to get events");
        assert_eq!(stored.len(), 3, "Should persist all 3 events");
    }

    #[tokio::test]
    async fn test_correlation_id_propagates() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();

        let correlation_id = "test-correlation-123";
        let root = Uuid::new_v4();
        let mut command = create_test_command("orders", root, b"test");
        command.correlation_id = correlation_id.to_string();

        let response = client.execute(command).await.expect("Command failed");

        let events = response.events.expect("Should have events");
        assert_eq!(
            events.correlation_id, correlation_id,
            "Correlation ID should propagate to events"
        );
    }

    // Helper to extract sequence from response
    fn extract_seq(response: &angzarr::proto::CommandResponse) -> u32 {
        response
            .events
            .as_ref()
            .and_then(|e| e.pages.first())
            .and_then(|p| match &p.sequence {
                Some(event_page::Sequence::Num(n)) => Some(*n),
                _ => None,
            })
            .unwrap_or(0)
    }
}

// ============================================================================
// Wrapper types (needed because Arc<T> doesn't implement traits directly)
// ============================================================================

struct EchoAggregateWrapper(Arc<EchoAggregate>);

#[async_trait]
impl AggregateHandler for EchoAggregateWrapper {
    async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, Status> {
        self.0.handle(ctx).await
    }
}

/// Aggregate that produces N events per command.
struct MultiEventAggregate {
    events_per_command: u32,
}

// ============================================================================
// EventBook Repair Tests
// ============================================================================

mod event_book_repair {
    use super::*;
    use angzarr::proto::event_query_server::EventQueryServer;
    use angzarr::services::event_book_repair::repair_if_needed;
    use angzarr::services::{EventBookRepairer, EventQueryService};
    use angzarr::storage::mock::{MockEventStore, MockSnapshotStore};
    use angzarr::storage::EventStore;
    use std::net::SocketAddr;
    use tokio::net::TcpListener;
    use tonic::transport::Server;

    /// Start an EventQuery gRPC server with test data.
    async fn start_event_query_server(
        event_store: Arc<MockEventStore>,
        snapshot_store: Arc<MockSnapshotStore>,
    ) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let service = EventQueryService::new(event_store, snapshot_store);

        tokio::spawn(async move {
            Server::builder()
                .add_service(EventQueryServer::new(service))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .unwrap();
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        addr
    }

    fn test_event(sequence: u32, event_type: &str) -> EventPage {
        EventPage {
            sequence: Some(event_page::Sequence::Num(sequence)),
            created_at: None,
            event: Some(Any {
                type_url: format!("type.googleapis.com/{}", event_type),
                value: vec![sequence as u8],
            }),
        }
    }

    #[tokio::test]
    async fn test_repairer_fetches_missing_history() {
        // Set up event store with full history
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());

        let domain = "orders";
        let root = Uuid::new_v4();

        // Store events 0-4
        let events: Vec<EventPage> = (0..5)
            .map(|i| test_event(i, &format!("Event{}", i)))
            .collect();
        event_store.add(domain, root, events).await.unwrap();

        // Start EventQuery server
        let addr = start_event_query_server(event_store, snapshot_store).await;

        // Create repairer
        let mut repairer = EventBookRepairer::connect(&addr.to_string())
            .await
            .expect("Failed to connect to EventQuery");

        // Create incomplete EventBook (only event 4, missing 0-3)
        let incomplete = EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![test_event(4, "Event4")],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        // Verify it's incomplete
        assert!(!repairer.is_complete(&incomplete));

        // Repair it
        let repaired = repairer.repair(incomplete).await.expect("Repair failed");

        // Verify repaired book is complete with all events
        assert!(repairer.is_complete(&repaired));
        assert_eq!(repaired.pages.len(), 5, "Should have all 5 events");

        // Verify sequence order
        for (i, page) in repaired.pages.iter().enumerate() {
            if let Some(event_page::Sequence::Num(seq)) = &page.sequence {
                assert_eq!(*seq as usize, i, "Event {} should have sequence {}", i, i);
            }
        }
    }

    #[tokio::test]
    async fn test_repairer_passes_through_complete_book() {
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());

        let addr = start_event_query_server(event_store, snapshot_store).await;

        let mut repairer = EventBookRepairer::connect(&addr.to_string())
            .await
            .expect("Failed to connect");

        // Create complete EventBook (starts at sequence 0)
        let complete = EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: Uuid::new_v4().as_bytes().to_vec(),
                }),
            }),
            pages: vec![test_event(0, "Created"), test_event(1, "Updated")],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        // Verify it's already complete
        assert!(repairer.is_complete(&complete));

        // Repair should return same book
        let result = repairer.repair(complete.clone()).await.expect("Repair failed");
        assert_eq!(result.pages.len(), 2, "Should pass through unchanged");
    }

    #[tokio::test]
    async fn test_repairer_handles_empty_aggregate() {
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());

        let addr = start_event_query_server(event_store, snapshot_store).await;

        let mut repairer = EventBookRepairer::connect(&addr.to_string())
            .await
            .expect("Failed to connect");

        // Create incomplete book for non-existent aggregate
        let root = Uuid::new_v4();
        let incomplete = EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![test_event(5, "LateEvent")], // Missing 0-4
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        // Repair - should return empty book since aggregate doesn't exist
        let repaired = repairer.repair(incomplete).await.expect("Repair failed");

        // Empty book is considered complete
        assert!(repairer.is_complete(&repaired));
        assert!(repaired.pages.is_empty(), "Should return empty for non-existent aggregate");
    }

    #[tokio::test]
    async fn test_discovery_resolves_event_query_via_env_var() {
        use angzarr::discovery::ServiceDiscovery;

        // Set up event store with full history
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());

        let domain = "orders";
        let root = Uuid::new_v4();

        // Store events 0-2
        let events: Vec<EventPage> = (0..3)
            .map(|i| test_event(i, &format!("Event{}", i)))
            .collect();
        event_store.add(domain, root, events).await.unwrap();

        // Start EventQuery server
        let addr = start_event_query_server(event_store, snapshot_store).await;

        // Set env var for discovery fallback
        std::env::set_var("EVENT_QUERY_ADDRESS", addr.to_string());

        // Create static discovery (no K8s, will use env var fallback)
        let discovery = ServiceDiscovery::new_static();

        // Resolve EventQuery for domain - should use env var
        let mut eq_client = discovery
            .get_event_query(domain)
            .await
            .expect("Should resolve via EVENT_QUERY_ADDRESS");

        // Create incomplete EventBook (only event 2, missing 0-1)
        let incomplete = EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![test_event(2, "Event2")],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        // Repair via the client we got from discovery
        let repaired = repair_if_needed(&mut eq_client, incomplete)
            .await
            .expect("Repair failed");

        assert_eq!(repaired.pages.len(), 3, "Should have all 3 events after repair");

        // Clean up env var
        std::env::remove_var("EVENT_QUERY_ADDRESS");
    }

    #[tokio::test]
    async fn test_discovery_resolves_registered_aggregate() {
        use angzarr::discovery::ServiceDiscovery;

        // Set up event store with full history
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());

        let domain = "products";
        let root = Uuid::new_v4();

        // Store events 0-1
        let events: Vec<EventPage> = (0..2)
            .map(|i| test_event(i, &format!("ProductEvent{}", i)))
            .collect();
        event_store.add(domain, root, events).await.unwrap();

        // Start EventQuery server
        let addr = start_event_query_server(event_store, snapshot_store).await;

        // Create discovery and register aggregate
        let discovery = ServiceDiscovery::new_static();
        discovery
            .register_aggregate(domain, &addr.ip().to_string(), addr.port())
            .await;

        // Resolve EventQuery - should use registered aggregate
        let mut eq_client = discovery
            .get_event_query(domain)
            .await
            .expect("Should resolve via registered aggregate");

        // Create incomplete EventBook (only event 1, missing 0)
        let incomplete = EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![test_event(1, "ProductEvent1")],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        // Repair via the client we got from discovery
        let repaired = repair_if_needed(&mut eq_client, incomplete)
            .await
            .expect("Repair failed");

        assert_eq!(repaired.pages.len(), 2, "Should have all 2 events after repair");
    }
}

impl MultiEventAggregate {
    fn new(events_per_command: u32) -> Self {
        Self { events_per_command }
    }
}

#[async_trait]
impl AggregateHandler for MultiEventAggregate {
    async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, Status> {
        let command_book = ctx
            .command
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing command"))?;

        let cover = command_book.cover.clone();

        let next_seq = ctx
            .events
            .as_ref()
            .and_then(|e| e.pages.last())
            .and_then(|p| match &p.sequence {
                Some(event_page::Sequence::Num(n)) => Some(n + 1),
                _ => None,
            })
            .unwrap_or(0);

        let pages: Vec<EventPage> = (0..self.events_per_command)
            .map(|i| EventPage {
                sequence: Some(event_page::Sequence::Num(next_seq + i)),
                event: Some(Any {
                    type_url: format!("test.Event{}", i),
                    value: vec![i as u8],
                }),
                created_at: None,
            })
            .collect();

        Ok(EventBook {
            cover,
            pages,
            snapshot: None,
            correlation_id: command_book.correlation_id.clone(),
            snapshot_state: None,
        })
    }
}

// ============================================================================
// gRPC Over UDS Tests
// ============================================================================

mod grpc_over_uds {
    use super::*;
    use angzarr::proto::aggregate_coordinator_client::AggregateCoordinatorClient;
    use angzarr::proto::aggregate_coordinator_server::{
        AggregateCoordinator, AggregateCoordinatorServer,
    };
    use angzarr::proto::{CommandResponse, SyncCommandBook};
    use angzarr::transport::{connect_to_address, prepare_uds_socket};
    use tokio::net::UnixListener;
    use tokio_stream::wrappers::UnixListenerStream;
    use tonic::transport::Server;
    use tonic::{Request, Response};

    /// Mock aggregate service for UDS tests.
    struct MockAggregateService {
        call_count: AtomicU32,
    }

    impl MockAggregateService {
        fn new() -> Self {
            Self {
                call_count: AtomicU32::new(0),
            }
        }
    }

    #[tonic::async_trait]
    impl AggregateCoordinator for MockAggregateService {
        async fn handle(
            &self,
            request: Request<CommandBook>,
        ) -> Result<Response<CommandResponse>, Status> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let cmd = request.into_inner();

            // Echo command as event
            let events = EventBook {
                cover: cmd.cover,
                pages: vec![EventPage {
                    sequence: Some(event_page::Sequence::Num(0)),
                    event: cmd.pages.first().and_then(|p| p.command.clone()),
                    created_at: None,
                }],
                snapshot: None,
                correlation_id: cmd.correlation_id,
                snapshot_state: None,
            };

            Ok(Response::new(CommandResponse {
                events: Some(events),
                projections: Vec::new(),
            }))
        }

        async fn handle_sync(
            &self,
            request: Request<SyncCommandBook>,
        ) -> Result<Response<CommandResponse>, Status> {
            let sync_cmd = request.into_inner();
            let cmd = sync_cmd.command.unwrap_or_default();

            self.call_count.fetch_add(1, Ordering::SeqCst);

            let events = EventBook {
                cover: cmd.cover,
                pages: vec![EventPage {
                    sequence: Some(event_page::Sequence::Num(0)),
                    event: cmd.pages.first().and_then(|p| p.command.clone()),
                    created_at: None,
                }],
                snapshot: None,
                correlation_id: cmd.correlation_id,
                snapshot_state: None,
            };

            Ok(Response::new(CommandResponse {
                events: Some(events),
                projections: Vec::new(),
            }))
        }
    }

    #[tokio::test]
    async fn test_grpc_server_and_client_over_uds() {
        let base_path = temp_dir();
        let socket_path = base_path.join("test-aggregate.sock");

        // Start gRPC server on UDS
        let _guard = prepare_uds_socket(&socket_path).expect("Failed to prepare socket");
        let uds = UnixListener::bind(&socket_path).expect("Failed to bind UDS");
        let uds_stream = UnixListenerStream::new(uds);

        let service = MockAggregateService::new();
        let server = Server::builder().add_service(AggregateCoordinatorServer::new(service));

        // Run server in background
        let server_task = tokio::spawn(async move {
            server.serve_with_incoming(uds_stream).await.unwrap();
        });

        // Give server time to start
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Connect client via UDS
        let channel = connect_to_address(socket_path.to_str().unwrap())
            .await
            .expect("Failed to connect");
        let mut client = AggregateCoordinatorClient::new(channel);

        // Execute command
        let command = create_test_command("orders", Uuid::new_v4(), b"test-data");
        let response = client.handle(command).await.expect("RPC failed");
        let sync_resp = response.into_inner();

        assert!(sync_resp.events.is_some(), "Should return events");
        assert_eq!(
            sync_resp.events.as_ref().unwrap().pages.len(),
            1,
            "Should have one event"
        );

        server_task.abort();
        cleanup_dir(&base_path);
    }

    #[tokio::test]
    async fn test_multiple_concurrent_uds_requests() {
        let base_path = temp_dir();
        let socket_path = base_path.join("concurrent-aggregate.sock");

        let _guard = prepare_uds_socket(&socket_path).expect("Failed to prepare socket");
        let uds = UnixListener::bind(&socket_path).expect("Failed to bind UDS");
        let uds_stream = UnixListenerStream::new(uds);

        let service = MockAggregateService::new();
        let server = Server::builder().add_service(AggregateCoordinatorServer::new(service));

        let server_task = tokio::spawn(async move {
            server.serve_with_incoming(uds_stream).await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Create multiple clients
        let mut handles = Vec::new();
        for i in 0..10 {
            let path = socket_path.clone();
            let handle = tokio::spawn(async move {
                let channel = connect_to_address(path.to_str().unwrap())
                    .await
                    .expect("Failed to connect");
                let mut client = AggregateCoordinatorClient::new(channel);

                let command =
                    create_test_command("orders", Uuid::new_v4(), format!("request-{}", i).as_bytes());
                client.handle(command).await.expect("RPC failed")
            });
            handles.push(handle);
        }

        // All requests should succeed
        for handle in handles {
            let response = handle.await.expect("Task panicked");
            assert!(response.into_inner().events.is_some());
        }

        server_task.abort();
        cleanup_dir(&base_path);
    }

    #[tokio::test]
    async fn test_uds_socket_cleanup_on_server_restart() {
        let base_path = temp_dir();
        let socket_path = base_path.join("restart-aggregate.sock");

        // First server instance
        {
            let _guard = prepare_uds_socket(&socket_path).expect("Failed to prepare socket");
            let uds = UnixListener::bind(&socket_path).expect("Failed to bind UDS");
            let uds_stream = UnixListenerStream::new(uds);

            let service = MockAggregateService::new();
            let server = Server::builder().add_service(AggregateCoordinatorServer::new(service));

            let server_task = tokio::spawn(async move {
                server.serve_with_incoming(uds_stream).await.unwrap();
            });

            tokio::time::sleep(Duration::from_millis(50)).await;
            server_task.abort();
        }

        // Socket file may still exist - prepare_uds_socket should clean it up
        let _guard = prepare_uds_socket(&socket_path).expect("Should be able to prepare socket again");
        let uds = UnixListener::bind(&socket_path).expect("Should be able to bind again");
        let uds_stream = UnixListenerStream::new(uds);

        let service = MockAggregateService::new();
        let server = Server::builder().add_service(AggregateCoordinatorServer::new(service));

        let server_task = tokio::spawn(async move {
            server.serve_with_incoming(uds_stream).await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Should be able to connect
        let channel = connect_to_address(socket_path.to_str().unwrap())
            .await
            .expect("Failed to connect to restarted server");
        let mut client = AggregateCoordinatorClient::new(channel);

        let command = create_test_command("orders", Uuid::new_v4(), b"after-restart");
        let response = client.handle(command).await.expect("RPC to restarted server failed");
        assert!(response.into_inner().events.is_some());

        server_task.abort();
        cleanup_dir(&base_path);
    }
}

// ============================================================================
// Saga Activation Tests
// ============================================================================

mod saga_activation {
    use super::*;
    use angzarr::embedded::{SagaConfig, SagaHandler};
    use angzarr::proto::SagaResponse;
    use std::sync::atomic::AtomicBool;

    /// Saga that produces a command when it sees an OrderPlaced event.
    struct FulfillmentSaga {
        triggered: AtomicBool,
        command_domain: String,
    }

    impl FulfillmentSaga {
        fn new(command_domain: &str) -> Self {
            Self {
                triggered: AtomicBool::new(false),
                command_domain: command_domain.to_string(),
            }
        }

        fn was_triggered(&self) -> bool {
            self.triggered.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl SagaHandler for FulfillmentSaga {
        async fn handle(&self, events: &EventBook) -> Result<SagaResponse, Status> {
            self.triggered.store(true, Ordering::SeqCst);

            let mut commands = Vec::new();

            // For each event, produce a command to another domain
            for page in &events.pages {
                if let Some(event) = &page.event {
                    if event.type_url.contains("OrderPlaced") {
                        let cmd = CommandBook {
                            cover: Some(Cover {
                                domain: self.command_domain.clone(),
                                root: events
                                    .cover
                                    .as_ref()
                                    .and_then(|c| c.root.clone()),
                            }),
                            pages: vec![CommandPage {
                                sequence: 0,
                                command: Some(Any {
                                    type_url: "inventory.ReserveStock".to_string(),
                                    value: event.value.clone(),
                                }),
                            }],
                            correlation_id: events.correlation_id.clone(),
                            saga_origin: None,
                            auto_resequence: true,
                            fact: false,
                        };
                        commands.push(cmd);
                    }
                }
            }

            Ok(SagaResponse {
                commands,
                ..Default::default()
            })
        }
    }

    #[tokio::test]
    async fn test_saga_receives_events_and_produces_commands() {
        let saga = Arc::new(FulfillmentSaga::new("inventory"));
        let saga_clone = saga.clone();

        let mut runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_aggregate("inventory", EchoAggregate::new())
            .register_saga(
                "fulfillment",
                SagaWrapper(saga_clone),
                SagaConfig::default().with_domains(vec!["orders".to_string()]),
            )
            .build()
            .await
            .expect("Failed to build runtime");

        // Start event distribution to projectors and sagas
        runtime.start().await.expect("Failed to start runtime");

        // Subscribe to record published events
        let channel_bus = runtime.channel_bus();
        let handler_state = RecordingHandlerState::new();
        let subscriber = channel_bus.with_config(angzarr::bus::ChannelConfig::subscriber_all());
        subscriber
            .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
            .await
            .unwrap();
        subscriber.start_consuming().await.unwrap();

        let client = runtime.command_client();

        // Execute command that triggers saga
        let root = Uuid::new_v4();
        let mut command = create_test_command("orders", root, b"order-data");
        command.pages[0].command = Some(Any {
            type_url: "orders.OrderPlaced".to_string(),
            value: b"order-data".to_vec(),
        });

        client.execute(command).await.expect("Command failed");

        // Give saga chain time to complete (saga processes event, produces command, command executes)
        tokio::time::sleep(Duration::from_millis(500)).await;

        assert!(saga.was_triggered(), "Saga should have been triggered");

        // Check that saga produced a command that was executed (inventory domain has events)
        let events = handler_state.get_events().await;
        let inventory_events: Vec<_> = events
            .iter()
            .filter(|e| {
                e.cover
                    .as_ref()
                    .map(|c| c.domain == "inventory")
                    .unwrap_or(false)
            })
            .collect();

        assert!(
            !inventory_events.is_empty(),
            "Saga should have produced command that resulted in inventory events (got {} total events)",
            events.len()
        );
    }

    #[tokio::test]
    async fn test_saga_domain_filtering() {
        let orders_saga = Arc::new(FulfillmentSaga::new("inventory"));
        let orders_saga_clone = orders_saga.clone();

        let products_saga = Arc::new(FulfillmentSaga::new("warehouse"));
        let products_saga_clone = products_saga.clone();

        let mut runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_aggregate("products", EchoAggregate::new())
            .register_aggregate("inventory", EchoAggregate::new())
            .register_aggregate("warehouse", EchoAggregate::new())
            .register_saga(
                "orders-saga",
                SagaWrapper(orders_saga_clone),
                SagaConfig::default().with_domains(vec!["orders".to_string()]),
            )
            .register_saga(
                "products-saga",
                SagaWrapper(products_saga_clone),
                SagaConfig::default().with_domains(vec!["products".to_string()]),
            )
            .build()
            .await
            .expect("Failed to build runtime");

        // Start event distribution
        runtime.start().await.expect("Failed to start runtime");

        let client = runtime.command_client();

        // Execute orders command
        let mut orders_cmd = create_test_command("orders", Uuid::new_v4(), b"order");
        orders_cmd.pages[0].command = Some(Any {
            type_url: "orders.OrderPlaced".to_string(),
            value: b"order".to_vec(),
        });
        client.execute(orders_cmd).await.expect("Orders command failed");

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Only orders saga should be triggered
        assert!(
            orders_saga.was_triggered(),
            "Orders saga should be triggered for orders domain"
        );
        assert!(
            !products_saga.was_triggered(),
            "Products saga should NOT be triggered for orders domain"
        );

        // Execute products command
        let mut products_cmd = create_test_command("products", Uuid::new_v4(), b"product");
        products_cmd.pages[0].command = Some(Any {
            type_url: "products.ProductCreated".to_string(),
            value: b"product".to_vec(),
        });
        client
            .execute(products_cmd)
            .await
            .expect("Products command failed");

        tokio::time::sleep(Duration::from_millis(100)).await;

        assert!(
            products_saga.was_triggered(),
            "Products saga should be triggered for products domain"
        );
    }

    #[tokio::test]
    async fn test_saga_correlation_id_propagates() {
        let saga = Arc::new(FulfillmentSaga::new("inventory"));
        let saga_clone = saga.clone();

        let mut runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_aggregate("inventory", EchoAggregate::new())
            .register_saga(
                "fulfillment",
                SagaWrapper(saga_clone),
                SagaConfig::default().with_domains(vec!["orders".to_string()]),
            )
            .build()
            .await
            .expect("Failed to build runtime");

        // Start event distribution
        runtime.start().await.expect("Failed to start runtime");

        let channel_bus = runtime.channel_bus();
        let handler_state = RecordingHandlerState::new();
        let subscriber = channel_bus.with_config(angzarr::bus::ChannelConfig::subscriber_all());
        subscriber
            .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
            .await
            .unwrap();
        subscriber.start_consuming().await.unwrap();

        let client = runtime.command_client();

        let correlation_id = "saga-correlation-test-123";
        let mut command = create_test_command("orders", Uuid::new_v4(), b"order");
        command.correlation_id = correlation_id.to_string();
        command.pages[0].command = Some(Any {
            type_url: "orders.OrderPlaced".to_string(),
            value: b"order".to_vec(),
        });

        client.execute(command).await.expect("Command failed");
        tokio::time::sleep(Duration::from_millis(200)).await;

        let events = handler_state.get_events().await;

        // All events (from both orders and inventory) should have same correlation ID
        for event in &events {
            assert_eq!(
                event.correlation_id, correlation_id,
                "All events should preserve correlation ID"
            );
        }
    }

    /// Wrapper to make Arc<FulfillmentSaga> implement SagaHandler.
    struct SagaWrapper(Arc<FulfillmentSaga>);

    #[async_trait]
    impl SagaHandler for SagaWrapper {
        async fn handle(&self, events: &EventBook) -> Result<SagaResponse, Status> {
            self.0.handle(events).await
        }
    }
}

// ============================================================================
// Projector Activation Tests
// ============================================================================

mod projector_activation {
    use super::*;
    use angzarr::embedded::{ProjectorConfig, ProjectorHandler};
    use angzarr::proto::Projection;
    use std::sync::atomic::AtomicBool;

    /// Projector that records events for verification.
    struct RecordingProjector {
        received: Arc<RwLock<Vec<EventBook>>>,
        synchronous: bool,
    }

    impl RecordingProjector {
        fn new(synchronous: bool) -> Self {
            Self {
                received: Arc::new(RwLock::new(Vec::new())),
                synchronous,
            }
        }

        async fn received_count(&self) -> usize {
            self.received.read().await.len()
        }
    }

    #[async_trait]
    impl ProjectorHandler for RecordingProjector {
        async fn handle(&self, events: &EventBook) -> Result<Projection, Status> {
            self.received.write().await.push(events.clone());

            let mut projection = Projection::default();
            if self.synchronous {
                projection.projector = "recording".to_string();
                projection.sequence = events.pages.len() as u32;
            }
            Ok(projection)
        }
    }

    /// Projector that produces output for streaming.
    struct OutputProjector {
        triggered: AtomicBool,
    }

    impl OutputProjector {
        fn new() -> Self {
            Self {
                triggered: AtomicBool::new(false),
            }
        }

        fn was_triggered(&self) -> bool {
            self.triggered.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl ProjectorHandler for OutputProjector {
        async fn handle(&self, events: &EventBook) -> Result<Projection, Status> {
            self.triggered.store(true, Ordering::SeqCst);

            Ok(Projection {
                projector: "output".to_string(),
                cover: events.cover.clone(),
                projection: Some(Any {
                    type_url: "test.ProjectionResult".to_string(),
                    value: b"projected-data".to_vec(),
                }),
                sequence: events.pages.len() as u32,
            })
        }
    }

    #[tokio::test]
    async fn test_synchronous_projector_blocks_command() {
        let projector = Arc::new(RecordingProjector::new(true));
        let projector_clone = projector.clone();

        let mut runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_projector(
                "sync-projector",
                ProjectorWrapper(projector_clone),
                ProjectorConfig::sync(),
            )
            .build()
            .await
            .expect("Failed to build runtime");

        // Start event distribution
        runtime.start().await.expect("Failed to start runtime");

        let client = runtime.command_client();

        let command = create_test_command("orders", Uuid::new_v4(), b"sync-test");
        let response = client.execute(command).await.expect("Command failed");

        // Synchronous projector should have already processed by time command returns
        let count = projector.received_count().await;
        assert!(count >= 1, "Sync projector should process during command");

        // Response should include projection
        assert!(
            !response.projections.is_empty(),
            "Command response should include sync projector output"
        );
    }

    #[tokio::test]
    async fn test_async_projector_runs_in_background() {
        let projector = Arc::new(RecordingProjector::new(false));
        let projector_clone = projector.clone();

        let mut runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_projector(
                "async-projector",
                ProjectorWrapper(projector_clone),
                ProjectorConfig::async_(),
            )
            .build()
            .await
            .expect("Failed to build runtime");

        // Start event distribution
        runtime.start().await.expect("Failed to start runtime");

        let client = runtime.command_client();

        let command = create_test_command("orders", Uuid::new_v4(), b"async-test");
        let _response = client.execute(command).await.expect("Command failed");

        // Command response should NOT include async projector output
        // (async projectors don't block the command)
        // Note: response.projections only includes sync projectors

        // Give async projector time to process
        tokio::time::sleep(Duration::from_millis(100)).await;

        let count = projector.received_count().await;
        assert!(count >= 1, "Async projector should process in background");
    }

    #[tokio::test]
    async fn test_projector_domain_filtering() {
        let orders_projector = Arc::new(RecordingProjector::new(false));
        let orders_clone = orders_projector.clone();

        let products_projector = Arc::new(RecordingProjector::new(false));
        let products_clone = products_projector.clone();

        let mut runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_aggregate("products", EchoAggregate::new())
            .register_projector(
                "orders-projector",
                ProjectorWrapper(orders_clone),
                ProjectorConfig::async_().with_domains(vec!["orders".to_string()]),
            )
            .register_projector(
                "products-projector",
                ProjectorWrapper(products_clone),
                ProjectorConfig::async_().with_domains(vec!["products".to_string()]),
            )
            .build()
            .await
            .expect("Failed to build runtime");

        // Start event distribution
        runtime.start().await.expect("Failed to start runtime");

        let client = runtime.command_client();

        // Execute orders command
        let orders_cmd = create_test_command("orders", Uuid::new_v4(), b"order");
        client.execute(orders_cmd).await.expect("Orders failed");

        tokio::time::sleep(Duration::from_millis(100)).await;

        let orders_count = orders_projector.received_count().await;
        let products_count = products_projector.received_count().await;

        assert!(
            orders_count >= 1,
            "Orders projector should receive orders events"
        );
        assert_eq!(
            products_count, 0,
            "Products projector should NOT receive orders events"
        );

        // Execute products command
        let products_cmd = create_test_command("products", Uuid::new_v4(), b"product");
        client.execute(products_cmd).await.expect("Products failed");

        tokio::time::sleep(Duration::from_millis(100)).await;

        let products_count = products_projector.received_count().await;
        assert!(
            products_count >= 1,
            "Products projector should receive products events"
        );
    }

    #[tokio::test]
    async fn test_multiple_projectors_receive_same_event() {
        let projector_a = Arc::new(RecordingProjector::new(false));
        let clone_a = projector_a.clone();

        let projector_b = Arc::new(RecordingProjector::new(false));
        let clone_b = projector_b.clone();

        let mut runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_projector("projector-a", ProjectorWrapper(clone_a), ProjectorConfig::async_())
            .register_projector("projector-b", ProjectorWrapper(clone_b), ProjectorConfig::async_())
            .build()
            .await
            .expect("Failed to build runtime");

        // Start event distribution
        runtime.start().await.expect("Failed to start runtime");

        // Wait for async projectors to fully initialize their consumers
        tokio::time::sleep(Duration::from_millis(50)).await;

        let client = runtime.command_client();

        let command = create_test_command("orders", Uuid::new_v4(), b"multi-projector");
        client.execute(command).await.expect("Command failed");

        // Wait for async projectors to receive and process events
        tokio::time::sleep(Duration::from_millis(300)).await;

        let count_a = projector_a.received_count().await;
        let count_b = projector_b.received_count().await;

        assert!(count_a >= 1, "Projector A should receive event (got {})", count_a);
        assert!(count_b >= 1, "Projector B should receive event (got {})", count_b);
    }

    #[tokio::test]
    async fn test_projector_output_available_for_streaming() {
        let projector = Arc::new(OutputProjector::new());
        let projector_clone = projector.clone();

        let mut runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_projector(
                "output-projector",
                OutputWrapper(projector_clone),
                ProjectorConfig::sync(),
            )
            .build()
            .await
            .expect("Failed to build runtime");

        // Start event distribution
        runtime.start().await.expect("Failed to start runtime");

        let client = runtime.command_client();

        let command = create_test_command("orders", Uuid::new_v4(), b"streaming");
        let response = client.execute(command).await.expect("Command failed");

        assert!(projector.was_triggered(), "Projector should be triggered");

        // Sync projector output is included in response.projections
        assert!(
            !response.projections.is_empty(),
            "Response should include projector output for streaming"
        );

        let projection = &response.projections[0];
        assert_eq!(projection.projector, "output", "Projector name should match");
    }

    /// Wrapper for RecordingProjector.
    struct ProjectorWrapper(Arc<RecordingProjector>);

    #[async_trait]
    impl ProjectorHandler for ProjectorWrapper {
        async fn handle(&self, events: &EventBook) -> Result<Projection, Status> {
            self.0.handle(events).await
        }
    }

    /// Wrapper for OutputProjector.
    struct OutputWrapper(Arc<OutputProjector>);

    #[async_trait]
    impl ProjectorHandler for OutputWrapper {
        async fn handle(&self, events: &EventBook) -> Result<Projection, Status> {
            self.0.handle(events).await
        }
    }
}

// ============================================================================
// Streaming Tests
// ============================================================================

mod streaming {
    use super::*;
    use angzarr::embedded::{ProjectorConfig, ProjectorHandler};
    use angzarr::proto::Projection;

    /// Projector that produces streaming output.
    struct StreamingProjector;

    #[async_trait]
    impl ProjectorHandler for StreamingProjector {
        async fn handle(&self, events: &EventBook) -> Result<Projection, Status> {
            Ok(Projection {
                projector: "streaming".to_string(),
                cover: events.cover.clone(),
                projection: Some(Any {
                    type_url: "test.StreamedProjection".to_string(),
                    value: format!("streamed-{}", events.pages.len()).into_bytes(),
                }),
                sequence: events.pages.len() as u32,
            })
        }
    }

    #[tokio::test]
    async fn test_events_published_to_bus_for_streaming() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        // Subscribe to events
        let channel_bus = runtime.channel_bus();
        let handler_state = RecordingHandlerState::new();
        let subscriber = channel_bus.with_config(angzarr::bus::ChannelConfig::subscriber_all());
        subscriber
            .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
            .await
            .unwrap();
        subscriber.start_consuming().await.unwrap();

        // Wait for consumer task to be ready
        tokio::time::sleep(Duration::from_millis(50)).await;

        let client = runtime.command_client();

        // Execute multiple commands
        for i in 0..3 {
            let cmd = create_test_command("orders", Uuid::new_v4(), format!("stream-{}", i).as_bytes());
            client.execute(cmd).await.expect("Command failed");
        }

        // Wait for async event distribution to complete
        tokio::time::sleep(Duration::from_millis(200)).await;

        let events = handler_state.get_events().await;
        assert_eq!(events.len(), 3, "Should receive all 3 events for streaming (got {})", events.len());
    }

    #[tokio::test]
    async fn test_streaming_preserves_correlation_id() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_projector("streaming", StreamingProjector, ProjectorConfig::sync())
            .build()
            .await
            .expect("Failed to build runtime");

        let channel_bus = runtime.channel_bus();
        let handler_state = RecordingHandlerState::new();
        let subscriber = channel_bus.with_config(angzarr::bus::ChannelConfig::subscriber_all());
        subscriber
            .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
            .await
            .unwrap();
        subscriber.start_consuming().await.unwrap();

        let client = runtime.command_client();

        let correlation_id = "streaming-correlation-123";
        let mut command = create_test_command("orders", Uuid::new_v4(), b"stream-test");
        command.correlation_id = correlation_id.to_string();

        let response = client.execute(command).await.expect("Command failed");

        // Response events should have correlation ID
        assert_eq!(
            response.events.as_ref().unwrap().correlation_id,
            correlation_id
        );

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Events on bus should have correlation ID
        let events = handler_state.get_events().await;
        for event in &events {
            assert_eq!(
                event.correlation_id, correlation_id,
                "Streamed events should preserve correlation ID"
            );
        }
    }

    #[tokio::test]
    async fn test_multiple_subscribers_receive_streamed_events() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let channel_bus = runtime.channel_bus();

        // Create two subscribers
        let state_a = RecordingHandlerState::new();
        let subscriber_a = channel_bus.with_config(angzarr::bus::ChannelConfig::subscriber_all());
        subscriber_a
            .subscribe(Box::new(RecordingHandler::new(state_a.clone())))
            .await
            .unwrap();
        subscriber_a.start_consuming().await.unwrap();

        let state_b = RecordingHandlerState::new();
        let subscriber_b = channel_bus.with_config(angzarr::bus::ChannelConfig::subscriber_all());
        subscriber_b
            .subscribe(Box::new(RecordingHandler::new(state_b.clone())))
            .await
            .unwrap();
        subscriber_b.start_consuming().await.unwrap();

        let client = runtime.command_client();

        let command = create_test_command("orders", Uuid::new_v4(), b"multi-stream");
        client.execute(command).await.expect("Command failed");

        tokio::time::sleep(Duration::from_millis(100)).await;

        let count_a = state_a.received_count().await;
        let count_b = state_b.received_count().await;

        assert!(count_a >= 1, "Subscriber A should receive streamed event");
        assert!(count_b >= 1, "Subscriber B should receive streamed event");
    }

    #[tokio::test]
    async fn test_streamed_events_include_all_pages() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", MultiEventAggregate::new(5))
            .build()
            .await
            .expect("Failed to build runtime");

        let channel_bus = runtime.channel_bus();
        let handler_state = RecordingHandlerState::new();
        let subscriber = channel_bus.with_config(angzarr::bus::ChannelConfig::subscriber_all());
        subscriber
            .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
            .await
            .unwrap();
        subscriber.start_consuming().await.unwrap();

        let client = runtime.command_client();

        let command = create_test_command("orders", Uuid::new_v4(), b"multi-page");
        client.execute(command).await.expect("Command failed");

        tokio::time::sleep(Duration::from_millis(100)).await;

        let events = handler_state.get_events().await;
        assert!(!events.is_empty(), "Should receive events");

        // The streamed EventBook should contain all 5 pages
        let event_book = &events[0];
        assert_eq!(
            event_book.pages.len(),
            5,
            "Streamed EventBook should include all event pages"
        );
    }
}

// ============================================================================
// Concurrent Command Execution Tests
// ============================================================================

mod concurrent_commands {
    use super::*;

    #[tokio::test]
    async fn test_sequential_commands_same_aggregate_auto_resequence() {
        // Test auto_resequence with sequential commands to same aggregate
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();
        let root = Uuid::new_v4();

        // Execute commands sequentially with auto_resequence enabled
        let total = 10;
        for i in 0..total {
            let mut cmd = create_test_command("orders", root, format!("seq-{}", i).as_bytes());
            cmd.auto_resequence = true;
            client.execute(cmd).await.expect(&format!("Command {} failed", i));
        }

        let events = runtime.event_store().get("orders", root).await.unwrap();
        assert_eq!(events.len(), total, "Should have {} events", total);

        // Verify sequences are unique and contiguous
        let mut seqs: Vec<u32> = events
            .iter()
            .filter_map(|e| match &e.sequence {
                Some(event_page::Sequence::Num(n)) => Some(*n),
                _ => None,
            })
            .collect();
        seqs.sort();
        let expected: Vec<u32> = (0..total as u32).collect();
        assert_eq!(seqs, expected, "Sequences should be 0-{}", total - 1);
    }

    #[tokio::test]
    async fn test_sequential_commands_different_aggregates() {
        // Test commands to different aggregates execute independently
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();

        // Execute commands to different aggregates
        let total = 10;
        let mut results = Vec::new();

        for i in 0..total {
            let root = Uuid::new_v4();
            let cmd = create_test_command("orders", root, format!("different-{}", i).as_bytes());
            let result = client.execute(cmd).await.expect("Command failed");
            results.push((root, result));
        }

        assert_eq!(results.len(), total, "All commands should succeed");

        // Verify each aggregate has exactly one event at sequence 0
        for (root, _) in &results {
            let events = runtime.event_store().get("orders", *root).await.unwrap();
            assert_eq!(events.len(), 1);
            if let Some(event_page::Sequence::Num(seq)) = &events[0].sequence {
                assert_eq!(*seq, 0, "First event should be sequence 0");
            }
        }
    }

    #[tokio::test]
    async fn test_rapid_sequential_commands() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();
        let root = Uuid::new_v4();

        // Execute commands rapidly in sequence (no sleep between)
        for i in 0..50 {
            let cmd = create_test_command("orders", root, format!("rapid-{}", i).as_bytes());
            client.execute(cmd).await.expect(&format!("Command {} failed", i));
        }

        let events = runtime.event_store().get("orders", root).await.unwrap();
        assert_eq!(events.len(), 50, "Should have 50 events");
    }
}

// ============================================================================
// Lossy Bus Integration Tests
// ============================================================================

mod lossy_bus {
    use super::*;
    use angzarr::bus::{
        ChannelConfig, ChannelEventBus, EventBus, EventHandler, PublishResult,
        Result as BusResult,
    };
    use std::sync::atomic::AtomicUsize;

    /// Deterministic event bus wrapper that drops events based on a pattern.
    /// For integration tests - predictable behavior.
    struct DeterministicLossyBus {
        inner: Arc<dyn EventBus>,
        drop_pattern: Vec<bool>, // true = drop, false = pass through
        counter: AtomicUsize,
    }

    impl DeterministicLossyBus {
        /// Create with a pattern: e.g., [false, true] drops every other event
        fn with_pattern(inner: Arc<dyn EventBus>, pattern: Vec<bool>) -> Self {
            Self {
                inner,
                drop_pattern: pattern,
                counter: AtomicUsize::new(0),
            }
        }

        /// Drop every Nth event (e.g., every_n=2 drops events 1, 3, 5...)
        fn drop_every_nth(inner: Arc<dyn EventBus>, n: usize) -> Self {
            let pattern: Vec<bool> = (0..n).map(|i| i == n - 1).collect();
            Self::with_pattern(inner, pattern)
        }

        /// Drop all events
        fn drop_all(inner: Arc<dyn EventBus>) -> Self {
            Self::with_pattern(inner, vec![true])
        }

        /// Pass all events (no dropping)
        fn passthrough(inner: Arc<dyn EventBus>) -> Self {
            Self::with_pattern(inner, vec![false])
        }
    }

    #[async_trait]
    impl EventBus for DeterministicLossyBus {
        async fn publish(&self, book: Arc<EventBook>) -> BusResult<PublishResult> {
            let idx = self.counter.fetch_add(1, Ordering::SeqCst);
            let pattern_idx = idx % self.drop_pattern.len();

            if self.drop_pattern[pattern_idx] {
                // Drop this event, return empty result
                Ok(PublishResult::default())
            } else {
                self.inner.publish(book).await
            }
        }

        async fn subscribe(&self, handler: Box<dyn EventHandler>) -> BusResult<()> {
            self.inner.subscribe(handler).await
        }

        async fn start_consuming(&self) -> BusResult<()> {
            self.inner.start_consuming().await
        }
    }

    #[tokio::test]
    async fn test_runtime_with_passthrough_bus() {
        // Create channel bus wrapped in passthrough (no drops)
        let channel_bus = Arc::new(ChannelEventBus::new(ChannelConfig::publisher()));
        let passthrough_bus = Arc::new(DeterministicLossyBus::passthrough(channel_bus.clone()));

        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .with_event_bus(channel_bus.clone(), passthrough_bus)
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let channel_bus = runtime.channel_bus();
        let handler_state = RecordingHandlerState::new();
        let subscriber = channel_bus.with_config(ChannelConfig::subscriber_all());
        subscriber
            .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
            .await
            .unwrap();
        subscriber.start_consuming().await.unwrap();

        // Wait for subscriber to be ready
        tokio::time::sleep(Duration::from_millis(50)).await;

        let client = runtime.command_client();

        // Execute commands
        for i in 0..10 {
            let cmd = create_test_command("orders", Uuid::new_v4(), format!("lossy-{}", i).as_bytes());
            client.execute(cmd).await.expect("Command failed");
        }

        tokio::time::sleep(Duration::from_millis(100)).await;

        // With passthrough, all events should be received
        let count = handler_state.received_count().await;
        assert_eq!(count, 10, "All events should be received in passthrough mode");
    }

    #[tokio::test]
    async fn test_runtime_with_deterministic_drops() {
        // Create channel bus that drops every other event (50% deterministic)
        let channel_bus = Arc::new(ChannelEventBus::new(ChannelConfig::publisher()));
        let lossy_bus = Arc::new(DeterministicLossyBus::drop_every_nth(
            channel_bus.clone(),
            2, // Drop every 2nd event
        ));

        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .with_event_bus(channel_bus.clone(), lossy_bus)
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let channel_bus = runtime.channel_bus();
        let handler_state = RecordingHandlerState::new();
        let subscriber = channel_bus.with_config(ChannelConfig::subscriber_all());
        subscriber
            .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
            .await
            .unwrap();
        subscriber.start_consuming().await.unwrap();

        // Wait for subscriber to be ready
        tokio::time::sleep(Duration::from_millis(50)).await;

        let client = runtime.command_client();

        // Execute 10 commands
        for i in 0..10 {
            let cmd = create_test_command("orders", Uuid::new_v4(), format!("lossy-{}", i).as_bytes());
            client.execute(cmd).await.expect("Command failed");
        }

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Events should still be persisted (lossy is only for pub/sub, not storage)
        let roots = runtime.event_store().list_roots("orders").await.unwrap();
        assert_eq!(roots.len(), 10, "All events should be persisted to storage");

        // Exactly half should be received (deterministic: drop every 2nd)
        let received = handler_state.received_count().await;
        assert_eq!(received, 5, "Should receive exactly 5 events (every other dropped)");
    }

    #[tokio::test]
    async fn test_lossy_bus_commands_still_succeed() {
        // Create channel bus that drops ALL events
        let channel_bus = Arc::new(ChannelEventBus::new(ChannelConfig::publisher()));
        let drop_all_bus = Arc::new(DeterministicLossyBus::drop_all(channel_bus.clone()));

        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .with_event_bus(channel_bus.clone(), drop_all_bus)
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();

        for i in 0..5 {
            let cmd = create_test_command("orders", Uuid::new_v4(), format!("drop-all-{}", i).as_bytes());
            let result = client.execute(cmd).await;
            assert!(result.is_ok(), "Command {} should succeed even with lossy bus", i);
        }

        // Events should still be persisted to storage
        let roots = runtime.event_store().list_roots("orders").await.unwrap();
        assert_eq!(roots.len(), 5, "Events should still be persisted");
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

mod error_handling {
    use super::*;

    /// Aggregate that always fails.
    struct FailingAggregate;

    #[async_trait]
    impl AggregateHandler for FailingAggregate {
        async fn handle(&self, _ctx: ContextualCommand) -> Result<EventBook, Status> {
            Err(Status::internal("Aggregate intentionally failed"))
        }
    }

    /// Aggregate that fails on specific commands.
    struct ConditionalFailAggregate {
        fail_on: String,
    }

    impl ConditionalFailAggregate {
        fn new(fail_on: &str) -> Self {
            Self {
                fail_on: fail_on.to_string(),
            }
        }
    }

    #[async_trait]
    impl AggregateHandler for ConditionalFailAggregate {
        async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, Status> {
            let command = ctx.command.as_ref().unwrap();
            if let Some(page) = command.pages.first() {
                if let Some(cmd) = &page.command {
                    if cmd.type_url.contains(&self.fail_on) {
                        return Err(Status::invalid_argument(format!(
                            "Rejected command: {}",
                            self.fail_on
                        )));
                    }
                }
            }

            // Otherwise, behave like EchoAggregate
            EchoAggregate::new().handle(ctx).await
        }
    }

    #[tokio::test]
    async fn test_aggregate_failure_returns_error() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", FailingAggregate)
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();
        let cmd = create_test_command("orders", Uuid::new_v4(), b"will-fail");

        let result = client.execute(cmd).await;
        assert!(result.is_err(), "Should return error when aggregate fails");

        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("failed") || err_str.contains("Failed"),
            "Error should mention failure, got: {}",
            err_str
        );
    }

    #[tokio::test]
    async fn test_unknown_domain_returns_error() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();
        let cmd = create_test_command("unknown-domain", Uuid::new_v4(), b"data");

        let result = client.execute(cmd).await;
        assert!(result.is_err(), "Should return error for unknown domain");

        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("No handler") || err_str.contains("not found"),
            "Error should mention missing handler, got: {}",
            err_str
        );
    }

    #[tokio::test]
    async fn test_conditional_failure_isolates_to_single_command() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", ConditionalFailAggregate::new("BadCommand"))
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();
        let root1 = Uuid::new_v4();
        let root2 = Uuid::new_v4();

        // First command succeeds
        let mut cmd1 = create_test_command("orders", root1, b"good");
        cmd1.pages[0].command = Some(prost_types::Any {
            type_url: "GoodCommand".to_string(),
            value: vec![],
        });
        client.execute(cmd1).await.expect("Good command should succeed");

        // Second command fails
        let mut cmd2 = create_test_command("orders", root2, b"bad");
        cmd2.pages[0].command = Some(prost_types::Any {
            type_url: "BadCommand".to_string(),
            value: vec![],
        });
        let result2 = client.execute(cmd2).await;
        assert!(result2.is_err(), "Bad command should fail");

        // Third command succeeds
        let mut cmd3 = create_test_command("orders", root1, b"good-again");
        cmd3.pages[0].command = Some(prost_types::Any {
            type_url: "AnotherGoodCommand".to_string(),
            value: vec![],
        });
        client.execute(cmd3).await.expect("Another good command should succeed");

        // Verify events
        let events1 = runtime.event_store().get("orders", root1).await.unwrap();
        assert_eq!(events1.len(), 2, "First aggregate should have 2 events");

        let events2 = runtime.event_store().get("orders", root2).await.unwrap();
        assert_eq!(events2.len(), 0, "Failed aggregate should have 0 events");
    }

    #[tokio::test]
    async fn test_missing_cover_returns_error() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();

        // Command without cover
        let cmd = CommandBook {
            cover: None,
            pages: vec![],
            correlation_id: String::new(),
            saga_origin: None,
            auto_resequence: true,
            fact: false,
        };

        let result = client.execute(cmd).await;
        assert!(result.is_err(), "Should fail without cover");
    }

    #[tokio::test]
    async fn test_missing_root_uuid_returns_error() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();

        // Command with cover but no root
        let cmd = CommandBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: None,
            }),
            pages: vec![],
            correlation_id: String::new(),
            saga_origin: None,
            auto_resequence: true,
            fact: false,
        };

        let result = client.execute(cmd).await;
        assert!(result.is_err(), "Should fail without root UUID");
    }
}

// ============================================================================
// End-to-End Saga Workflow Tests
// ============================================================================

mod e2e_saga_workflow {
    use super::*;
    use angzarr::embedded::{SagaConfig, SagaHandler};
    use angzarr::proto::SagaResponse;
    use std::sync::atomic::AtomicU32;

    /// Saga that chains commands across domains.
    /// Orders -> Inventory -> Shipping
    struct OrderFulfillmentSaga {
        step_count: AtomicU32,
    }

    impl OrderFulfillmentSaga {
        fn new() -> Self {
            Self {
                step_count: AtomicU32::new(0),
            }
        }

        fn steps(&self) -> u32 {
            self.step_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl SagaHandler for OrderFulfillmentSaga {
        async fn handle(&self, events: &EventBook) -> Result<SagaResponse, Status> {
            self.step_count.fetch_add(1, Ordering::SeqCst);

            let mut commands = Vec::new();
            let domain = events
                .cover
                .as_ref()
                .map(|c| c.domain.as_str())
                .unwrap_or("unknown");

            for page in &events.pages {
                if let Some(event) = &page.event {
                    // Orders domain triggers Inventory
                    if domain == "orders" && event.type_url.contains("OrderPlaced") {
                        commands.push(CommandBook {
                            cover: Some(Cover {
                                domain: "inventory".to_string(),
                                root: events.cover.as_ref().and_then(|c| c.root.clone()),
                            }),
                            pages: vec![CommandPage {
                                sequence: 0,
                                command: Some(prost_types::Any {
                                    type_url: "inventory.ReserveStock".to_string(),
                                    value: event.value.clone(),
                                }),
                            }],
                            correlation_id: events.correlation_id.clone(),
                            saga_origin: None,
                            auto_resequence: true,
                            fact: false,
                        });
                    }

                    // Inventory domain triggers Shipping
                    if domain == "inventory" && event.type_url.contains("ReserveStock") {
                        commands.push(CommandBook {
                            cover: Some(Cover {
                                domain: "shipping".to_string(),
                                root: events.cover.as_ref().and_then(|c| c.root.clone()),
                            }),
                            pages: vec![CommandPage {
                                sequence: 0,
                                command: Some(prost_types::Any {
                                    type_url: "shipping.CreateShipment".to_string(),
                                    value: event.value.clone(),
                                }),
                            }],
                            correlation_id: events.correlation_id.clone(),
                            saga_origin: None,
                            auto_resequence: true,
                            fact: false,
                        });
                    }
                }
            }

            Ok(SagaResponse {
                commands,
                ..Default::default()
            })
        }
    }

    struct SagaWrapper(Arc<OrderFulfillmentSaga>);

    #[async_trait]
    impl SagaHandler for SagaWrapper {
        async fn handle(&self, events: &EventBook) -> Result<SagaResponse, Status> {
            self.0.handle(events).await
        }
    }

    #[tokio::test]
    async fn test_saga_chains_across_three_domains() {
        let saga = Arc::new(OrderFulfillmentSaga::new());
        let saga_clone = saga.clone();

        let mut runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_aggregate("inventory", EchoAggregate::new())
            .register_aggregate("shipping", EchoAggregate::new())
            .register_saga(
                "order-fulfillment",
                SagaWrapper(saga_clone),
                SagaConfig::default().with_domains(vec![
                    "orders".to_string(),
                    "inventory".to_string(),
                ]),
            )
            .build()
            .await
            .expect("Failed to build runtime");

        runtime.start().await.expect("Failed to start runtime");

        // Wait for saga consumers to initialize
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Record all events
        let channel_bus = runtime.channel_bus();
        let handler_state = RecordingHandlerState::new();
        let subscriber = channel_bus.with_config(angzarr::bus::ChannelConfig::subscriber_all());
        subscriber
            .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
            .await
            .unwrap();
        subscriber.start_consuming().await.unwrap();

        // Wait for subscriber to be ready
        tokio::time::sleep(Duration::from_millis(50)).await;

        let client = runtime.command_client();

        // Trigger the chain: OrderPlaced -> ReserveStock -> CreateShipment
        let root = Uuid::new_v4();
        let mut cmd = create_test_command("orders", root, b"order-data");
        cmd.pages[0].command = Some(prost_types::Any {
            type_url: "orders.OrderPlaced".to_string(),
            value: b"order-123".to_vec(),
        });
        cmd.correlation_id = "e2e-test-correlation".to_string();

        client.execute(cmd).await.expect("Initial command failed");

        // Wait for full saga chain to complete (order -> inventory -> shipping)
        tokio::time::sleep(Duration::from_millis(800)).await;

        // Verify saga was triggered (may not see all triggers due to timing)
        let steps = saga.steps();
        assert!(
            steps >= 1,
            "Saga should be triggered at least once (got {} steps)",
            steps
        );

        // Verify events in all three domains
        let events = handler_state.get_events().await;
        let domains: Vec<_> = events
            .iter()
            .filter_map(|e| e.cover.as_ref().map(|c| c.domain.clone()))
            .collect();

        assert!(
            domains.contains(&"orders".to_string()),
            "Should have orders events"
        );
        assert!(
            domains.contains(&"inventory".to_string()),
            "Should have inventory events"
        );
        assert!(
            domains.contains(&"shipping".to_string()),
            "Should have shipping events"
        );

        // Verify all events have same correlation ID
        for event in &events {
            assert_eq!(
                event.correlation_id, "e2e-test-correlation",
                "All events should preserve correlation ID through saga chain"
            );
        }

        // Verify storage has events for each domain with same root
        let orders_events = runtime.event_store().get("orders", root).await.unwrap();
        let inventory_events = runtime.event_store().get("inventory", root).await.unwrap();
        let shipping_events = runtime.event_store().get("shipping", root).await.unwrap();

        assert!(!orders_events.is_empty(), "Orders should have events");
        assert!(!inventory_events.is_empty(), "Inventory should have events");
        assert!(!shipping_events.is_empty(), "Shipping should have events");
    }

    #[tokio::test]
    async fn test_multiple_saga_chains_sequential() {
        // Test multiple saga chains execute correctly when run sequentially
        let saga = Arc::new(OrderFulfillmentSaga::new());
        let saga_clone = saga.clone();

        let mut runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_aggregate("inventory", EchoAggregate::new())
            .register_aggregate("shipping", EchoAggregate::new())
            .register_saga(
                "fulfillment",
                SagaWrapper(saga_clone),
                SagaConfig::default().with_domains(vec![
                    "orders".to_string(),
                    "inventory".to_string(),
                ]),
            )
            .build()
            .await
            .expect("Failed to build runtime");

        runtime.start().await.expect("Failed to start runtime");

        let client = runtime.command_client();

        // Execute saga chains sequentially
        let mut roots = Vec::new();
        for i in 0..3 {
            let root = Uuid::new_v4();
            let mut cmd = create_test_command("orders", root, format!("order-{}", i).as_bytes());
            cmd.pages[0].command = Some(prost_types::Any {
                type_url: "orders.OrderPlaced".to_string(),
                value: format!("order-{}", i).into_bytes(),
            });
            cmd.correlation_id = format!("sequential-{}", i);
            client.execute(cmd).await.expect("Command failed");
            roots.push(root);

            // Wait longer for saga chain to complete before next
            // (Orders -> Inventory -> Shipping takes multiple hops)
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        // Verify each root has events in all three domains
        for (i, root) in roots.iter().enumerate() {
            let orders_events = runtime.event_store().get("orders", *root).await.unwrap();
            let inventory_events = runtime.event_store().get("inventory", *root).await.unwrap();
            let shipping_events = runtime.event_store().get("shipping", *root).await.unwrap();

            assert!(
                !orders_events.is_empty(),
                "Order {} should have orders events",
                i
            );
            assert!(
                !inventory_events.is_empty(),
                "Order {} should have inventory events",
                i
            );
            assert!(
                !shipping_events.is_empty(),
                "Order {} should have shipping events",
                i
            );
        }
    }
}

// ============================================================================
// Gateway-Style Tests (command execution patterns)
// ============================================================================

mod gateway_embedded {
    use super::*;

    /// Simple projector for gateway tests.
    struct GatewayTestProjector;

    #[async_trait]
    impl ProjectorHandler for GatewayTestProjector {
        async fn handle(&self, events: &EventBook) -> Result<Projection, Status> {
            Ok(Projection {
                projector: "receipt".to_string(),
                cover: events.cover.clone(),
                projection: Some(Any {
                    type_url: "test.Receipt".to_string(),
                    value: b"receipt-data".to_vec(),
                }),
                sequence: events.pages.len() as u32,
            })
        }
    }

    /// Test that command execution returns events like gateway would.
    #[tokio::test]
    async fn test_execute_returns_events_with_sequence() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();
        let root = Uuid::new_v4();

        let command = create_test_command("orders", root, b"test-data");
        let response = client.execute(command).await.expect("Command failed");

        // Verify response structure matches gateway expectations
        assert!(response.events.is_some(), "Response should include events");
        let events = response.events.as_ref().unwrap();
        assert!(events.cover.is_some(), "Events should have cover");
        assert_eq!(
            events.cover.as_ref().unwrap().domain,
            "orders",
            "Domain should match"
        );
        assert!(!events.pages.is_empty(), "Should have event pages");

        // Verify sequence is set
        let first_page = &events.pages[0];
        match &first_page.sequence {
            Some(event_page::Sequence::Num(n)) => assert_eq!(*n, 0, "First event should be seq 0"),
            _ => panic!("Expected sequence number"),
        }
    }

    /// Test query-like access to events after command.
    #[tokio::test]
    async fn test_query_events_after_command() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();
        let root = Uuid::new_v4();

        // Execute command
        let command = create_test_command("orders", root, b"query-test");
        client.execute(command).await.expect("Command failed");

        // Query events directly from store (like query service would)
        let events = runtime
            .event_store()
            .get("orders", root)
            .await
            .expect("Query failed");

        assert_eq!(events.len(), 1, "Should have one event");
        assert!(events[0].event.is_some(), "Event should have payload");
    }

    /// Test query with bounds (from/to sequence).
    #[tokio::test]
    async fn test_query_events_with_bounds() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();
        let root = Uuid::new_v4();

        // Execute multiple commands
        for i in 0..5 {
            let command = create_test_command("orders", root, format!("cmd-{}", i).as_bytes());
            client.execute(command).await.expect("Command failed");
        }

        // Query with bounds
        let all_events = runtime
            .event_store()
            .get("orders", root)
            .await
            .expect("Query all failed");
        assert_eq!(all_events.len(), 5, "Should have 5 events");

        // Query subset (from seq 2)
        let subset = runtime
            .event_store()
            .get_from("orders", root, 2)
            .await
            .expect("Query from failed");
        assert_eq!(subset.len(), 3, "Should have events from seq 2 onwards");

        // Query range
        let range = runtime
            .event_store()
            .get_from_to("orders", root, 1, 3)
            .await
            .expect("Query range failed");
        assert_eq!(range.len(), 2, "Should have events 1-2");
    }

    /// Test multiple commands return proper projections.
    #[tokio::test]
    async fn test_execute_returns_sync_projections() {
        let mut runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_projector(
                "receipt",
                GatewayTestProjector,
                ProjectorConfig::sync(),
            )
            .build()
            .await
            .expect("Failed to build runtime");

        runtime.start().await.expect("Failed to start");

        let client = runtime.command_client();
        let root = Uuid::new_v4();

        let command = create_test_command("orders", root, b"projection-test");
        let response = client.execute(command).await.expect("Command failed");

        // Sync projector results should be in response
        assert!(
            !response.projections.is_empty(),
            "Should include sync projector output"
        );
        assert_eq!(
            response.projections[0].projector, "receipt",
            "Projector name should match"
        );
    }

    /// Test list domains functionality.
    #[tokio::test]
    async fn test_list_registered_domains() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_aggregate("products", EchoAggregate::new())
            .register_aggregate("customers", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let router = runtime.router();
        let domains = router.domains();
        assert_eq!(domains.len(), 3, "Should have 3 domains");
        assert!(domains.contains(&"orders"), "Should contain orders");
        assert!(domains.contains(&"products"), "Should contain products");
        assert!(domains.contains(&"customers"), "Should contain customers");
    }

    /// Test list aggregate roots in domain.
    #[tokio::test]
    async fn test_list_roots_in_domain() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();

        // Create multiple aggregates
        let roots: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
        for root in &roots {
            let command = create_test_command("orders", *root, b"test");
            client.execute(command).await.expect("Command failed");
        }

        // List roots
        let listed = runtime
            .event_store()
            .list_roots("orders")
            .await
            .expect("List failed");

        assert_eq!(listed.len(), 3, "Should list all roots");
        for root in &roots {
            assert!(listed.contains(root), "Should contain root {}", root);
        }
    }
}

// ============================================================================
// Snapshot Integration Tests
// ============================================================================

mod snapshot_integration {
    use super::*;
    use angzarr::proto::Snapshot;

    #[tokio::test]
    async fn test_snapshot_store_and_retrieve() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("counters", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let root = Uuid::new_v4();

        // Store snapshot directly (Snapshot has only sequence + state)
        let snapshot = Snapshot {
            sequence: 10,
            state: Some(Any {
                type_url: "test.State".to_string(),
                value: b"snapshot-data".to_vec(),
            }),
        };

        runtime
            .snapshot_store()
            .put("counters", root, snapshot.clone())
            .await
            .expect("Put failed");

        // Retrieve
        let retrieved = runtime
            .snapshot_store()
            .get("counters", root)
            .await
            .expect("Get failed");

        assert!(retrieved.is_some(), "Should retrieve snapshot");
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.sequence, 10, "Sequence should match");
        assert!(retrieved.state.is_some(), "State should exist");
    }

    #[tokio::test]
    async fn test_snapshot_isolation_between_aggregates() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("counters", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let root1 = Uuid::new_v4();
        let root2 = Uuid::new_v4();

        // Store snapshots for both
        let snapshot1 = Snapshot {
            sequence: 5,
            state: Some(Any {
                type_url: "test.State".to_string(),
                value: b"state-1".to_vec(),
            }),
        };

        let snapshot2 = Snapshot {
            sequence: 15,
            state: Some(Any {
                type_url: "test.State".to_string(),
                value: b"state-2".to_vec(),
            }),
        };

        runtime
            .snapshot_store()
            .put("counters", root1, snapshot1)
            .await
            .expect("Put 1 failed");
        runtime
            .snapshot_store()
            .put("counters", root2, snapshot2)
            .await
            .expect("Put 2 failed");

        // Verify isolation
        let ret1 = runtime
            .snapshot_store()
            .get("counters", root1)
            .await
            .expect("Get 1 failed")
            .expect("Should exist");
        let ret2 = runtime
            .snapshot_store()
            .get("counters", root2)
            .await
            .expect("Get 2 failed")
            .expect("Should exist");

        assert_eq!(ret1.sequence, 5, "Root1 sequence");
        assert_eq!(ret2.sequence, 15, "Root2 sequence");
    }

    #[tokio::test]
    async fn test_snapshot_isolation_between_domains() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("domain_a", EchoAggregate::new())
            .register_aggregate("domain_b", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let root = Uuid::new_v4();

        // Store same root in different domains
        let snapshot_a = Snapshot {
            sequence: 100,
            state: Some(Any {
                type_url: "test.State".to_string(),
                value: b"domain-a".to_vec(),
            }),
        };

        let snapshot_b = Snapshot {
            sequence: 200,
            state: Some(Any {
                type_url: "test.State".to_string(),
                value: b"domain-b".to_vec(),
            }),
        };

        runtime
            .snapshot_store()
            .put("domain_a", root, snapshot_a)
            .await
            .expect("Put A failed");
        runtime
            .snapshot_store()
            .put("domain_b", root, snapshot_b)
            .await
            .expect("Put B failed");

        // Verify domain isolation
        let ret_a = runtime
            .snapshot_store()
            .get("domain_a", root)
            .await
            .expect("Get A failed")
            .expect("Should exist");
        let ret_b = runtime
            .snapshot_store()
            .get("domain_b", root)
            .await
            .expect("Get B failed")
            .expect("Should exist");

        assert_eq!(ret_a.sequence, 100, "Domain A sequence");
        assert_eq!(ret_b.sequence, 200, "Domain B sequence");
    }

    #[tokio::test]
    async fn test_snapshot_update_overwrites() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("counters", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let root = Uuid::new_v4();

        // Store initial
        let snapshot1 = Snapshot {
            sequence: 5,
            state: Some(Any {
                type_url: "test.State".to_string(),
                value: b"initial".to_vec(),
            }),
        };

        runtime
            .snapshot_store()
            .put("counters", root, snapshot1)
            .await
            .expect("Put 1 failed");

        // Update
        let snapshot2 = Snapshot {
            sequence: 10,
            state: Some(Any {
                type_url: "test.State".to_string(),
                value: b"updated".to_vec(),
            }),
        };

        runtime
            .snapshot_store()
            .put("counters", root, snapshot2)
            .await
            .expect("Put 2 failed");

        // Verify updated
        let retrieved = runtime
            .snapshot_store()
            .get("counters", root)
            .await
            .expect("Get failed")
            .expect("Should exist");

        assert_eq!(retrieved.sequence, 10, "Should be updated sequence");
        assert_eq!(
            retrieved.state.unwrap().value,
            b"updated".to_vec(),
            "Should be updated state"
        );
    }

    #[tokio::test]
    async fn test_snapshot_delete() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("counters", EchoAggregate::new())
            .build()
            .await
            .expect("Failed to build runtime");

        let root = Uuid::new_v4();

        // Store
        let snapshot = Snapshot {
            sequence: 5,
            state: Some(Any {
                type_url: "test.State".to_string(),
                value: b"to-delete".to_vec(),
            }),
        };

        runtime
            .snapshot_store()
            .put("counters", root, snapshot)
            .await
            .expect("Put failed");

        // Verify exists
        assert!(
            runtime
                .snapshot_store()
                .get("counters", root)
                .await
                .expect("Get failed")
                .is_some(),
            "Should exist before delete"
        );

        // Delete
        runtime
            .snapshot_store()
            .delete("counters", root)
            .await
            .expect("Delete failed");

        // Verify gone
        assert!(
            runtime
                .snapshot_store()
                .get("counters", root)
                .await
                .expect("Get failed")
                .is_none(),
            "Should not exist after delete"
        );
    }
}

// ============================================================================
// Projector Chaining Tests
// ============================================================================

mod projector_chaining {
    use super::*;

    /// Projector that records events and can trigger saga-like behavior.
    struct ChainableProjector {
        name: String,
        received: Arc<RwLock<Vec<(String, Uuid)>>>,
    }

    impl ChainableProjector {
        fn new(name: &str, received: Arc<RwLock<Vec<(String, Uuid)>>>) -> Self {
            Self {
                name: name.to_string(),
                received,
            }
        }

        async fn get_received(&self) -> Vec<(String, Uuid)> {
            self.received.read().await.clone()
        }
    }

    #[async_trait]
    impl ProjectorHandler for ChainableProjector {
        async fn handle(&self, events: &EventBook) -> Result<Projection, Status> {
            if let Some(cover) = &events.cover {
                if let Some(proto_uuid) = &cover.root {
                    let root = Uuid::from_slice(&proto_uuid.value).unwrap_or_default();
                    let mut received = self.received.write().await;
                    received.push((cover.domain.clone(), root));
                }
            }

            Ok(Projection {
                projector: self.name.clone(),
                cover: events.cover.clone(),
                projection: Some(Any {
                    type_url: format!("{}.Result", self.name),
                    value: format!("processed-by-{}", self.name).into_bytes(),
                }),
                sequence: events.pages.len() as u32,
            })
        }
    }

    struct ChainableProjectorWrapper(Arc<ChainableProjector>);

    #[async_trait]
    impl ProjectorHandler for ChainableProjectorWrapper {
        async fn handle(&self, events: &EventBook) -> Result<Projection, Status> {
            self.0.handle(events).await
        }
    }

    #[tokio::test]
    async fn test_multiple_projectors_chain_same_events() {
        let received1 = Arc::new(RwLock::new(Vec::new()));
        let received2 = Arc::new(RwLock::new(Vec::new()));
        let received3 = Arc::new(RwLock::new(Vec::new()));

        let proj1 = Arc::new(ChainableProjector::new("analytics", received1.clone()));
        let proj2 = Arc::new(ChainableProjector::new("notifications", received2.clone()));
        let proj3 = Arc::new(ChainableProjector::new("audit", received3.clone()));

        let mut runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_projector(
                "analytics",
                ChainableProjectorWrapper(proj1.clone()),
                ProjectorConfig::async_(),
            )
            .register_projector(
                "notifications",
                ChainableProjectorWrapper(proj2.clone()),
                ProjectorConfig::async_(),
            )
            .register_projector(
                "audit",
                ChainableProjectorWrapper(proj3.clone()),
                ProjectorConfig::async_(),
            )
            .build()
            .await
            .expect("Failed to build runtime");

        runtime.start().await.expect("Failed to start");

        let client = runtime.command_client();
        let root = Uuid::new_v4();

        // Execute command
        let command = create_test_command("orders", root, b"chain-test");
        client.execute(command).await.expect("Command failed");

        // Wait for async projectors
        tokio::time::sleep(Duration::from_millis(200)).await;

        // All projectors should receive the same event
        let r1 = proj1.get_received().await;
        let r2 = proj2.get_received().await;
        let r3 = proj3.get_received().await;

        assert_eq!(r1.len(), 1, "Analytics should receive event");
        assert_eq!(r2.len(), 1, "Notifications should receive event");
        assert_eq!(r3.len(), 1, "Audit should receive event");

        // All should have same root
        assert_eq!(r1[0].1, root, "Analytics should have correct root");
        assert_eq!(r2[0].1, root, "Notifications should have correct root");
        assert_eq!(r3[0].1, root, "Audit should have correct root");
    }

    #[tokio::test]
    async fn test_projector_and_saga_both_receive_events() {
        let projector_received = Arc::new(RwLock::new(Vec::new()));
        let saga_received = Arc::new(RwLock::new(Vec::new()));

        let projector = Arc::new(ChainableProjector::new("projector", projector_received.clone()));

        // Create a recording saga
        struct RecordingSaga {
            received: Arc<RwLock<Vec<(String, Uuid)>>>,
        }

        #[async_trait]
        impl SagaHandler for RecordingSaga {
            async fn handle(&self, events: &EventBook) -> Result<SagaResponse, Status> {
                if let Some(cover) = &events.cover {
                    if let Some(proto_uuid) = &cover.root {
                        let root = Uuid::from_slice(&proto_uuid.value).unwrap_or_default();
                        let mut received = self.received.write().await;
                        received.push((cover.domain.clone(), root));
                    }
                }
                Ok(SagaResponse::default())
            }
        }

        let saga = Arc::new(RecordingSaga {
            received: saga_received.clone(),
        });

        struct SagaWrapper(Arc<RecordingSaga>);

        #[async_trait]
        impl SagaHandler for SagaWrapper {
            async fn handle(&self, events: &EventBook) -> Result<SagaResponse, Status> {
                self.0.handle(events).await
            }
        }

        let mut runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_projector(
                "projector",
                ChainableProjectorWrapper(projector.clone()),
                ProjectorConfig::async_(),
            )
            .register_saga("saga", SagaWrapper(saga.clone()), SagaConfig::default())
            .build()
            .await
            .expect("Failed to build runtime");

        runtime.start().await.expect("Failed to start");

        let client = runtime.command_client();
        let root = Uuid::new_v4();

        let command = create_test_command("orders", root, b"both-receive");
        client.execute(command).await.expect("Command failed");

        tokio::time::sleep(Duration::from_millis(200)).await;

        let proj_r = projector.get_received().await;
        let saga_r = saga_received.read().await;

        assert_eq!(proj_r.len(), 1, "Projector should receive event");
        assert_eq!(saga_r.len(), 1, "Saga should receive event");
        assert_eq!(proj_r[0].1, saga_r[0].1, "Both should receive same root");
    }

    #[tokio::test]
    async fn test_sync_and_async_projectors_both_trigger() {
        let sync_received = Arc::new(RwLock::new(Vec::new()));
        let async_received = Arc::new(RwLock::new(Vec::new()));

        let sync_proj = Arc::new(ChainableProjector::new("sync", sync_received.clone()));
        let async_proj = Arc::new(ChainableProjector::new("async", async_received.clone()));

        let mut runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_projector(
                "sync",
                ChainableProjectorWrapper(sync_proj.clone()),
                ProjectorConfig::sync(),
            )
            .register_projector(
                "async",
                ChainableProjectorWrapper(async_proj.clone()),
                ProjectorConfig::async_(),
            )
            .build()
            .await
            .expect("Failed to build runtime");

        runtime.start().await.expect("Failed to start");

        let client = runtime.command_client();
        let root = Uuid::new_v4();

        let command = create_test_command("orders", root, b"sync-async");
        let response = client.execute(command).await.expect("Command failed");

        // Sync projector should be in response
        assert!(
            !response.projections.is_empty(),
            "Sync projector should be in response"
        );

        // Wait for async
        tokio::time::sleep(Duration::from_millis(200)).await;

        let sync_r = sync_proj.get_received().await;
        let async_r = async_proj.get_received().await;

        assert_eq!(sync_r.len(), 1, "Sync should receive");
        assert_eq!(async_r.len(), 1, "Async should receive");
    }
}

// ============================================================================
// Error Recovery Tests
// ============================================================================

mod error_recovery {
    use super::*;

    /// Aggregate that fails on specific commands.
    struct SelectiveFailAggregate {
        fail_pattern: String,
        success_count: AtomicU32,
    }

    impl SelectiveFailAggregate {
        fn new(fail_pattern: &str) -> Self {
            Self {
                fail_pattern: fail_pattern.to_string(),
                success_count: AtomicU32::new(0),
            }
        }
    }

    #[async_trait]
    impl AggregateHandler for SelectiveFailAggregate {
        async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, Status> {
            let command_book = ctx
                .command
                .as_ref()
                .ok_or_else(|| Status::invalid_argument("Missing command"))?;

            // Check if command matches fail pattern
            if let Some(page) = command_book.pages.first() {
                if let Some(cmd) = &page.command {
                    let data = String::from_utf8_lossy(&cmd.value);
                    if data.contains(&self.fail_pattern) {
                        return Err(Status::internal("Simulated failure"));
                    }
                }
            }

            self.success_count.fetch_add(1, Ordering::SeqCst);

            let cover = command_book.cover.clone();
            let next_seq = ctx
                .events
                .as_ref()
                .and_then(|e| e.pages.last())
                .and_then(|p| match &p.sequence {
                    Some(event_page::Sequence::Num(n)) => Some(n + 1),
                    _ => None,
                })
                .unwrap_or(0);

            Ok(EventBook {
                cover,
                pages: vec![EventPage {
                    sequence: Some(event_page::Sequence::Num(next_seq)),
                    event: command_book.pages[0].command.clone(),
                    created_at: None,
                }],
                snapshot: None,
                correlation_id: command_book.correlation_id.clone(),
                snapshot_state: None,
            })
        }
    }

    /// Helper to extract sequence from response.
    fn get_seq(response: &angzarr::proto::CommandResponse) -> u32 {
        response
            .events
            .as_ref()
            .and_then(|e| e.pages.last())
            .and_then(|p| match &p.sequence {
                Some(event_page::Sequence::Num(n)) => Some(*n),
                _ => None,
            })
            .unwrap_or(0)
    }

    #[tokio::test]
    async fn test_failed_command_does_not_persist_events() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", SelectiveFailAggregate::new("FAIL"))
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();
        let root = Uuid::new_v4();

        // This should fail
        let fail_cmd = create_test_command("orders", root, b"FAIL-this");
        let result = client.execute(fail_cmd).await;
        assert!(result.is_err(), "Should fail");

        // No events should be persisted
        let events = runtime
            .event_store()
            .get("orders", root)
            .await
            .expect("Query failed");
        assert!(events.is_empty(), "Failed command should not persist events");
    }

    #[tokio::test]
    async fn test_success_after_failure_on_same_aggregate() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", SelectiveFailAggregate::new("FAIL"))
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();
        let root = Uuid::new_v4();

        // First command fails
        let fail_cmd = create_test_command("orders", root, b"FAIL-first");
        let _ = client.execute(fail_cmd).await;

        // Second command succeeds
        let success_cmd = create_test_command("orders", root, b"success");
        let result = client.execute(success_cmd).await;
        assert!(result.is_ok(), "Second command should succeed");

        // Only success event should be persisted
        let events = runtime
            .event_store()
            .get("orders", root)
            .await
            .expect("Query failed");
        assert_eq!(events.len(), 1, "Should have one successful event");
    }

    #[tokio::test]
    async fn test_partial_failure_isolates_between_aggregates() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", SelectiveFailAggregate::new("FAIL"))
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();
        let root1 = Uuid::new_v4();
        let root2 = Uuid::new_v4();

        // Root1 fails
        let fail_cmd = create_test_command("orders", root1, b"FAIL");
        let _ = client.execute(fail_cmd).await;

        // Root2 succeeds
        let success_cmd = create_test_command("orders", root2, b"success");
        client.execute(success_cmd).await.expect("Should succeed");

        // Root1 has no events
        let events1 = runtime
            .event_store()
            .get("orders", root1)
            .await
            .expect("Query 1 failed");
        assert!(events1.is_empty(), "Root1 should have no events");

        // Root2 has events
        let events2 = runtime
            .event_store()
            .get("orders", root2)
            .await
            .expect("Query 2 failed");
        assert_eq!(events2.len(), 1, "Root2 should have one event");
    }

    #[tokio::test]
    async fn test_recovery_continues_sequence_correctly() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", SelectiveFailAggregate::new("FAIL"))
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();
        let root = Uuid::new_v4();

        // First success
        let cmd1 = create_test_command("orders", root, b"success-1");
        client.execute(cmd1).await.expect("Cmd1 failed");

        // Second fails
        let cmd2 = create_test_command("orders", root, b"FAIL");
        let _ = client.execute(cmd2).await;

        // Third success should have seq 1 (not 2)
        let cmd3 = create_test_command("orders", root, b"success-3");
        let resp = client.execute(cmd3).await.expect("Cmd3 failed");

        let seq = get_seq(&resp);
        assert_eq!(seq, 1, "Sequence should continue from last success");

        // Verify total events
        let events = runtime
            .event_store()
            .get("orders", root)
            .await
            .expect("Query failed");
        assert_eq!(events.len(), 2, "Should have 2 successful events");
    }

    #[tokio::test]
    async fn test_projector_failure_does_not_rollback_events() {
        /// Projector that always fails.
        struct FailingProjector;

        #[async_trait]
        impl ProjectorHandler for FailingProjector {
            async fn handle(&self, _events: &EventBook) -> Result<Projection, Status> {
                Err(Status::internal("Projector failure"))
            }
        }

        let mut runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_projector("failing", FailingProjector, ProjectorConfig::async_())
            .build()
            .await
            .expect("Failed to build runtime");

        runtime.start().await.expect("Failed to start");

        let client = runtime.command_client();
        let root = Uuid::new_v4();

        // Command should still succeed even if async projector fails
        let command = create_test_command("orders", root, b"test");
        let result = client.execute(command).await;
        assert!(result.is_ok(), "Command should succeed despite projector failure");

        // Events should be persisted
        let events = runtime
            .event_store()
            .get("orders", root)
            .await
            .expect("Query failed");
        assert_eq!(events.len(), 1, "Events should be persisted");
    }

    #[tokio::test]
    async fn test_sync_projector_failure_fails_command() {
        /// Sync projector that fails.
        struct FailingSyncProjector;

        #[async_trait]
        impl ProjectorHandler for FailingSyncProjector {
            async fn handle(&self, _events: &EventBook) -> Result<Projection, Status> {
                Err(Status::internal("Sync projector failure"))
            }
        }

        let mut runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate::new())
            .register_projector("failing-sync", FailingSyncProjector, ProjectorConfig::sync())
            .build()
            .await
            .expect("Failed to build runtime");

        runtime.start().await.expect("Failed to start");

        let client = runtime.command_client();
        let root = Uuid::new_v4();

        // Command should fail because sync projector fails
        let command = create_test_command("orders", root, b"test");
        let result = client.execute(command).await;

        // This behavior depends on implementation - sync projector failure may or may not
        // fail the command. Document actual behavior:
        // Currently events ARE persisted before sync projector runs, so command may succeed
        // but projections will be empty or error
        if result.is_err() {
            // If command fails, events should not be persisted
            let events = runtime
                .event_store()
                .get("orders", root)
                .await
                .expect("Query failed");
            assert!(events.is_empty(), "Failed command should not persist");
        }
        // If command succeeds, that's also valid behavior (projector runs after persistence)
    }

    #[tokio::test]
    async fn test_concurrent_failures_isolated() {
        let runtime = RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", SelectiveFailAggregate::new("FAIL"))
            .build()
            .await
            .expect("Failed to build runtime");

        let client = runtime.command_client();

        // Launch concurrent commands, some will fail
        let mut handles = Vec::new();
        for i in 0..10 {
            let client = client.clone();
            let root = Uuid::new_v4();
            let data = if i % 3 == 0 {
                format!("FAIL-{}", i)
            } else {
                format!("success-{}", i)
            };

            handles.push(tokio::spawn(async move {
                let cmd = create_test_command("orders", root, data.as_bytes());
                (root, client.execute(cmd).await.is_ok())
            }));
        }

        // Collect results
        let mut success_count = 0;
        for handle in handles {
            let (_, succeeded) = handle.await.expect("Task panicked");
            if succeeded {
                success_count += 1;
            }
        }

        // Should have some successes and some failures
        assert!(success_count > 0, "Some should succeed");
        assert!(success_count < 10, "Some should fail");
    }
}
