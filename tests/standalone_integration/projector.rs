//! Projector integration tests — activation, domain filtering, and chaining.

use crate::common::*;
use angzarr::proto::Projection;
use angzarr::standalone::{ProjectionMode, ProjectorConfig, ProjectorHandler};
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
    async fn handle(&self, events: &EventBook, _mode: ProjectionMode) -> Result<Projection, Status> {
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
    async fn handle(&self, events: &EventBook, _mode: ProjectionMode) -> Result<Projection, Status> {
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

    let command = create_test_command("orders", Uuid::new_v4(), b"sync-test", 0);
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

    let command = create_test_command("orders", Uuid::new_v4(), b"async-test", 0);
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
    let orders_cmd = create_test_command("orders", Uuid::new_v4(), b"order", 0);
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
    let products_cmd = create_test_command("products", Uuid::new_v4(), b"product", 0);
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
        .register_projector(
            "projector-a",
            ProjectorWrapper(clone_a),
            ProjectorConfig::async_(),
        )
        .register_projector(
            "projector-b",
            ProjectorWrapper(clone_b),
            ProjectorConfig::async_(),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    // Start event distribution
    runtime.start().await.expect("Failed to start runtime");

    // Wait for async projectors to fully initialize their consumers
    tokio::time::sleep(Duration::from_millis(50)).await;

    let client = runtime.command_client();

    let command = create_test_command("orders", Uuid::new_v4(), b"multi-projector", 0);
    client.execute(command).await.expect("Command failed");

    // Wait for async projectors to receive and process events
    tokio::time::sleep(Duration::from_millis(300)).await;

    let count_a = projector_a.received_count().await;
    let count_b = projector_b.received_count().await;

    assert!(
        count_a >= 1,
        "Projector A should receive event (got {})",
        count_a
    );
    assert!(
        count_b >= 1,
        "Projector B should receive event (got {})",
        count_b
    );
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

    let command = create_test_command("orders", Uuid::new_v4(), b"streaming", 0);
    let response = client.execute(command).await.expect("Command failed");

    assert!(projector.was_triggered(), "Projector should be triggered");

    // Sync projector output is included in response.projections
    assert!(
        !response.projections.is_empty(),
        "Response should include projector output for streaming"
    );

    let projection = &response.projections[0];
    assert_eq!(
        projection.projector, "output",
        "Projector name should match"
    );
}

/// Wrapper for RecordingProjector.
struct ProjectorWrapper(Arc<RecordingProjector>);

#[async_trait]
impl ProjectorHandler for ProjectorWrapper {
    async fn handle(&self, events: &EventBook, mode: ProjectionMode) -> Result<Projection, Status> {
        self.0.handle(events, mode).await
    }
}

/// Wrapper for OutputProjector.
struct OutputWrapper(Arc<OutputProjector>);

#[async_trait]
impl ProjectorHandler for OutputWrapper {
    async fn handle(&self, events: &EventBook, mode: ProjectionMode) -> Result<Projection, Status> {
        self.0.handle(events, mode).await
    }
}

// --- Projector chaining tests ---

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
    async fn handle(&self, events: &EventBook, _mode: ProjectionMode) -> Result<Projection, Status> {
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
    async fn handle(&self, events: &EventBook, mode: ProjectionMode) -> Result<Projection, Status> {
        self.0.handle(events, mode).await
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
    let command = create_test_command("orders", root, b"chain-test", 0);
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

    let projector = Arc::new(ChainableProjector::new(
        "projector",
        projector_received.clone(),
    ));

    // Create a recording saga
    struct RecordingSaga {
        received: Arc<RwLock<Vec<(String, Uuid)>>>,
    }

    #[async_trait]
    impl SagaHandler for RecordingSaga {
        async fn prepare(&self, _source: &EventBook) -> Result<Vec<Cover>, Status> {
            Ok(vec![]) // No destination state needed
        }

        async fn execute(
            &self,
            source: &EventBook,
            _destinations: &[EventBook],
        ) -> Result<SagaResponse, Status> {
            if let Some(cover) = &source.cover {
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
        async fn prepare(&self, source: &EventBook) -> Result<Vec<Cover>, Status> {
            self.0.prepare(source).await
        }

        async fn execute(
            &self,
            source: &EventBook,
            destinations: &[EventBook],
        ) -> Result<SagaResponse, Status> {
            self.0.execute(source, destinations).await
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
        .register_saga(
            "saga",
            SagaWrapper(saga.clone()),
            SagaConfig::new("orders", "orders"),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    let command = create_test_command("orders", root, b"both-receive", 0);
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

    let command = create_test_command("orders", root, b"sync-async", 0);
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
