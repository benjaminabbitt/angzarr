//! SyncMode integration tests.
//!
//! Tests verify the behavioral differences between ASYNC, SIMPLE, and CASCADE modes:
//! - ASYNC: No sync projectors, publish to bus (fire-and-forget)
//! - SIMPLE: Sync projectors called, publish to bus
//! - CASCADE: Sync projectors + sagas + PMs called, NO bus publishing

use crate::common::*;

/// Projector that records when it's called.
struct RecordingProjector {
    called: Arc<RwLock<Vec<EventBook>>>,
}

impl RecordingProjector {
    fn new() -> Self {
        Self {
            called: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn call_count(&self) -> usize {
        self.called.read().await.len()
    }
}

#[async_trait]
impl ProjectorHandler for RecordingProjector {
    async fn handle(
        &self,
        events: &EventBook,
        _mode: ProjectionMode,
    ) -> Result<Projection, Status> {
        self.called.write().await.push(events.clone());
        Ok(Projection {
            projector: "recording".to_string(),
            cover: events.cover.clone(),
            projection: Some(Any {
                type_url: "test.Projection".to_string(),
                value: b"projected".to_vec(),
            }),
            sequence: events.pages.len() as u32,
        })
    }
}

/// Wrapper for Arc<RecordingProjector>.
struct ProjectorWrapper(Arc<RecordingProjector>);

#[async_trait]
impl ProjectorHandler for ProjectorWrapper {
    async fn handle(&self, events: &EventBook, mode: ProjectionMode) -> Result<Projection, Status> {
        self.0.handle(events, mode).await
    }
}

// ============================================================================
// ASYNC Mode Tests
// ============================================================================

#[tokio::test]
async fn test_async_mode_skips_sync_projectors() {
    //! ASYNC mode should NOT call sync projectors.
    //!
    //! This is the "fire-and-forget" mode where we want minimal latency.
    //! Events are persisted and published to bus, but sync projectors are skipped.

    let projector = Arc::new(RecordingProjector::new());
    let projector_clone = projector.clone();

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_command_handler("orders", EchoAggregate::new())
        .register_projector(
            "sync-projector",
            ProjectorWrapper(projector_clone),
            ProjectorConfig::sync(),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");

    let client = runtime.command_client();
    let command = create_test_command("orders", Uuid::new_v4(), b"async-test", 0);

    // Use ASYNC mode - should skip sync projector
    let response = client.execute_async(command).await.expect("Command failed");

    // Projector should NOT have been called
    let call_count = projector.call_count().await;
    assert_eq!(call_count, 0, "ASYNC mode should NOT call sync projectors");

    // Response should have no projections
    assert!(
        response.projections.is_empty(),
        "ASYNC mode should not return projections"
    );

    // Events should be persisted (command succeeded)
    assert!(response.events.is_some(), "Events should be persisted");
}

#[tokio::test]
async fn test_async_mode_publishes_to_bus() {
    //! ASYNC mode should publish events to the bus for async processing.

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_command_handler("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    // Subscribe to bus using proper subscriber pattern
    let event_bus = runtime.event_bus();
    let bus_handler_state = RecordingHandlerState::new();
    let subscriber = event_bus
        .create_subscriber("test-sub", None)
        .await
        .expect("Failed to create subscriber");
    subscriber
        .subscribe(Box::new(RecordingHandler::new(bus_handler_state.clone())))
        .await
        .expect("Failed to subscribe");
    subscriber
        .start_consuming()
        .await
        .expect("Failed to start consuming");

    runtime.start().await.expect("Failed to start runtime");

    let client = runtime.command_client();
    let command = create_test_command("orders", Uuid::new_v4(), b"bus-test", 0);

    // Use ASYNC mode
    client.execute_async(command).await.expect("Command failed");

    // Give bus time to deliver
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Bus should have received events
    let bus_count = bus_handler_state.received_count().await;
    assert!(bus_count >= 1, "ASYNC mode should publish to bus");
}

// ============================================================================
// SIMPLE Mode Tests
// ============================================================================

#[tokio::test]
async fn test_simple_mode_calls_sync_projectors() {
    //! SIMPLE mode should call sync projectors and include projections in response.

    let projector = Arc::new(RecordingProjector::new());
    let projector_clone = projector.clone();

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_command_handler("orders", EchoAggregate::new())
        .register_projector(
            "sync-projector",
            ProjectorWrapper(projector_clone),
            ProjectorConfig::sync(),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");

    let client = runtime.command_client();
    let command = create_test_command("orders", Uuid::new_v4(), b"simple-test", 0);

    // Use SIMPLE mode (default execute())
    let response = client.execute(command).await.expect("Command failed");

    // Projector SHOULD have been called
    let call_count = projector.call_count().await;
    assert_eq!(call_count, 1, "SIMPLE mode should call sync projectors");

    // Response should include projection
    assert!(
        !response.projections.is_empty(),
        "SIMPLE mode should return projections"
    );
}

#[tokio::test]
async fn test_simple_mode_publishes_to_bus() {
    //! SIMPLE mode should publish events to the bus.

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_command_handler("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    // Subscribe to bus using proper subscriber pattern
    let event_bus = runtime.event_bus();
    let bus_handler_state = RecordingHandlerState::new();
    let subscriber = event_bus
        .create_subscriber("test-sub", None)
        .await
        .expect("Failed to create subscriber");
    subscriber
        .subscribe(Box::new(RecordingHandler::new(bus_handler_state.clone())))
        .await
        .expect("Failed to subscribe");
    subscriber
        .start_consuming()
        .await
        .expect("Failed to start consuming");

    runtime.start().await.expect("Failed to start runtime");

    let client = runtime.command_client();
    let command = create_test_command("orders", Uuid::new_v4(), b"bus-test", 0);

    // Use SIMPLE mode
    client.execute(command).await.expect("Command failed");

    // Give bus time to deliver
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Bus should have received events
    let bus_count = bus_handler_state.received_count().await;
    assert!(bus_count >= 1, "SIMPLE mode should publish to bus");
}

// ============================================================================
// CASCADE Mode Tests
// ============================================================================

#[tokio::test]
async fn test_cascade_mode_calls_sync_projectors() {
    //! CASCADE mode should call sync projectors.

    let projector = Arc::new(RecordingProjector::new());
    let projector_clone = projector.clone();

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_command_handler("orders", EchoAggregate::new())
        .register_projector(
            "sync-projector",
            ProjectorWrapper(projector_clone),
            ProjectorConfig::sync(),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");

    let client = runtime.command_client();
    let command = create_test_command("orders", Uuid::new_v4(), b"cascade-test", 0);

    // Use CASCADE mode
    let response = client
        .execute_cascade(command)
        .await
        .expect("Command failed");

    // Projector SHOULD have been called
    let call_count = projector.call_count().await;
    assert_eq!(call_count, 1, "CASCADE mode should call sync projectors");

    // Response should include projection
    assert!(
        !response.projections.is_empty(),
        "CASCADE mode should return projections"
    );
}

#[tokio::test]
async fn test_cascade_mode_skips_bus_publishing() {
    //! CASCADE mode should NOT publish events to the bus.
    //!
    //! Events flow through sync sagas instead, avoiding duplicate processing.

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_command_handler("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    // Subscribe to bus using proper subscriber pattern
    let event_bus = runtime.event_bus();
    let bus_handler_state = RecordingHandlerState::new();
    let subscriber = event_bus
        .create_subscriber("test-sub", None)
        .await
        .expect("Failed to create subscriber");
    subscriber
        .subscribe(Box::new(RecordingHandler::new(bus_handler_state.clone())))
        .await
        .expect("Failed to subscribe");
    subscriber
        .start_consuming()
        .await
        .expect("Failed to start consuming");

    runtime.start().await.expect("Failed to start runtime");

    let client = runtime.command_client();
    let command = create_test_command("orders", Uuid::new_v4(), b"no-bus-test", 0);

    // Use CASCADE mode
    client
        .execute_cascade(command)
        .await
        .expect("Command failed");

    // Give bus time if it were going to publish (it shouldn't)
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Bus should NOT have received events
    let bus_count = bus_handler_state.received_count().await;
    assert_eq!(bus_count, 0, "CASCADE mode should NOT publish to bus");
}

#[tokio::test]
async fn test_cascade_mode_calls_sync_sagas() {
    //! CASCADE mode should call sync sagas with the events.

    let saga_called = Arc::new(RwLock::new(Vec::<EventBook>::new()));
    let saga_called_clone = saga_called.clone();

    /// Saga that records when it's called.
    struct RecordingSaga {
        called: Arc<RwLock<Vec<EventBook>>>,
    }

    #[async_trait]
    impl SagaHandler for RecordingSaga {
        async fn handle(&self, source: &EventBook) -> Result<SagaResponse, Status> {
            self.called.write().await.push(source.clone());
            Ok(SagaResponse::default())
        }
    }

    struct SagaWrapper(Arc<RecordingSaga>);

    #[async_trait]
    impl SagaHandler for SagaWrapper {
        async fn handle(&self, source: &EventBook) -> Result<SagaResponse, Status> {
            self.0.handle(source).await
        }
    }

    let saga = Arc::new(RecordingSaga {
        called: saga_called_clone,
    });

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_command_handler("orders", EchoAggregate::new())
        .register_saga(
            "test-saga",
            SagaWrapper(saga.clone()),
            SagaConfig::new("orders", "orders"),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");

    let client = runtime.command_client();
    let command = create_test_command("orders", Uuid::new_v4(), b"saga-test", 0);

    // Use CASCADE mode
    client
        .execute_cascade(command)
        .await
        .expect("Command failed");

    // Saga SHOULD have been called synchronously
    let call_count = saga_called.read().await.len();
    assert_eq!(call_count, 1, "CASCADE mode should call sync sagas");
}

// ============================================================================
// End-to-End CASCADE Tests
// ============================================================================

#[tokio::test]
async fn test_cascade_end_to_end_saga_command_chain() {
    //! End-to-end test: command → saga → recursive command.
    //!
    //! Verifies CASCADE mode completes the full chain synchronously:
    //! 1. Execute command on "source" domain
    //! 2. Saga translates source events to "target" domain commands
    //! 3. Target command executes recursively (still in CASCADE)
    //! 4. Target events are produced

    use angzarr::proto::{command_page, AngzarrDeferredSequence};
    use angzarr::proto_ext::CoverExt;

    let target_events_received = Arc::new(RwLock::new(Vec::<EventBook>::new()));
    let target_events_clone = target_events_received.clone();

    /// Saga that translates source events to target commands.
    struct TranslatingSaga;

    #[async_trait]
    impl SagaHandler for TranslatingSaga {
        async fn handle(&self, source: &EventBook) -> Result<SagaResponse, Status> {
            // Produce a command for the "target" domain
            let target_root = Uuid::new_v4();
            let command = CommandBook {
                cover: Some(Cover {
                    domain: "target".to_string(),
                    root: Some(ProtoUuid {
                        value: target_root.as_bytes().to_vec(),
                    }),
                    ..Default::default()
                }),
                pages: vec![CommandPage {
                    header: Some(PageHeader {
                        sequence_type: Some(page_header::SequenceType::AngzarrDeferred(
                            AngzarrDeferredSequence {
                                source: source.cover.clone(),
                                source_seq: source
                                    .pages
                                    .first()
                                    .map(|p| p.sequence_num())
                                    .unwrap_or(0),
                            },
                        )),
                        ..Default::default()
                    }),
                    payload: Some(command_page::Payload::Command(Any {
                        type_url: "test.TargetCommand".to_string(),
                        value: b"target-command".to_vec(),
                    })),
                    merge_strategy: MergeStrategy::MergeCommutative as i32,
                }],
            };

            Ok(SagaResponse {
                commands: vec![command],
                ..Default::default()
            })
        }
    }

    /// Projector that records events from target domain.
    struct TargetRecordingProjector {
        events: Arc<RwLock<Vec<EventBook>>>,
    }

    #[async_trait]
    impl ProjectorHandler for TargetRecordingProjector {
        async fn handle(
            &self,
            events: &EventBook,
            _mode: ProjectionMode,
        ) -> Result<Projection, Status> {
            if events.domain() == "target" {
                self.events.write().await.push(events.clone());
            }
            Ok(Projection {
                projector: "target-recorder".to_string(),
                cover: events.cover.clone(),
                projection: None,
                sequence: events.pages.len() as u32,
            })
        }
    }

    struct SagaWrapper(Arc<TranslatingSaga>);
    struct TargetProjectorWrapper(Arc<TargetRecordingProjector>);

    #[async_trait]
    impl SagaHandler for SagaWrapper {
        async fn handle(&self, source: &EventBook) -> Result<SagaResponse, Status> {
            self.0.handle(source).await
        }
    }

    #[async_trait]
    impl ProjectorHandler for TargetProjectorWrapper {
        async fn handle(
            &self,
            events: &EventBook,
            mode: ProjectionMode,
        ) -> Result<Projection, Status> {
            self.0.handle(events, mode).await
        }
    }

    let saga = Arc::new(TranslatingSaga);
    let projector = Arc::new(TargetRecordingProjector {
        events: target_events_clone,
    });

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        // Source domain - where we send the initial command
        .register_command_handler("source", EchoAggregate::new())
        // Target domain - receives command from saga
        .register_command_handler("target", EchoAggregate::new())
        // Saga: source events → target commands
        .register_saga(
            "saga-source-target",
            SagaWrapper(saga),
            SagaConfig::new("source", "target"),
        )
        // Projector to observe target events
        .register_projector(
            "target-projector",
            TargetProjectorWrapper(projector),
            ProjectorConfig::sync(),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");

    let client = runtime.command_client();

    // Execute command on SOURCE domain with CASCADE mode
    let source_root = Uuid::new_v4();
    let command = create_test_command("source", source_root, b"trigger-cascade", 0);

    let response = client
        .execute_cascade(command)
        .await
        .expect("CASCADE command failed");

    // Verify source command produced events
    assert!(
        response.events.is_some(),
        "Source command should produce events"
    );

    // Verify projector received target events (synchronously!)
    // This proves the saga → target command chain completed
    let target_count = target_events_received.read().await.len();
    assert!(
        target_count >= 1,
        "Target domain should have received events from saga-triggered command (got {})",
        target_count
    );

    // The first target event should be from our cascade
    let target_events = target_events_received.read().await;
    assert_eq!(
        target_events[0].domain(),
        "target",
        "Event should be from target domain"
    );
}

// ============================================================================
// Mode Comparison Tests
// ============================================================================

#[tokio::test]
async fn test_mode_comparison_projector_behavior() {
    //! Compare projector invocation across all three modes.

    let projector = Arc::new(RecordingProjector::new());
    let projector_clone = projector.clone();

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_command_handler("orders", EchoAggregate::new())
        .register_projector(
            "sync-projector",
            ProjectorWrapper(projector_clone),
            ProjectorConfig::sync(),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");

    let client = runtime.command_client();

    // ASYNC: should NOT call projector
    let cmd1 = create_test_command("orders", Uuid::new_v4(), b"async", 0);
    client.execute_async(cmd1).await.expect("ASYNC failed");
    let count_after_async = projector.call_count().await;
    assert_eq!(count_after_async, 0, "ASYNC should not call projector");

    // SIMPLE: SHOULD call projector
    let cmd2 = create_test_command("orders", Uuid::new_v4(), b"simple", 0);
    client.execute(cmd2).await.expect("SIMPLE failed");
    let count_after_simple = projector.call_count().await;
    assert_eq!(count_after_simple, 1, "SIMPLE should call projector once");

    // CASCADE: SHOULD call projector
    let cmd3 = create_test_command("orders", Uuid::new_v4(), b"cascade", 0);
    client.execute_cascade(cmd3).await.expect("CASCADE failed");
    let count_after_cascade = projector.call_count().await;
    assert_eq!(
        count_after_cascade, 2,
        "CASCADE should call projector once more"
    );
}

#[tokio::test]
async fn test_mode_comparison_bus_behavior() {
    //! Compare bus publishing across all three modes.

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_command_handler("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    // Subscribe to bus using proper subscriber pattern
    let event_bus = runtime.event_bus();
    let bus_handler_state = RecordingHandlerState::new();
    let subscriber = event_bus
        .create_subscriber("test-sub", None)
        .await
        .expect("Failed to create subscriber");
    subscriber
        .subscribe(Box::new(RecordingHandler::new(bus_handler_state.clone())))
        .await
        .expect("Failed to subscribe");
    subscriber
        .start_consuming()
        .await
        .expect("Failed to start consuming");

    runtime.start().await.expect("Failed to start runtime");

    let client = runtime.command_client();

    // ASYNC: SHOULD publish to bus
    let cmd1 = create_test_command("orders", Uuid::new_v4(), b"async", 0);
    client.execute_async(cmd1).await.expect("ASYNC failed");
    tokio::time::sleep(Duration::from_millis(100)).await;
    let count_after_async = bus_handler_state.received_count().await;
    assert!(count_after_async >= 1, "ASYNC should publish to bus");

    // SIMPLE: SHOULD publish to bus
    let cmd2 = create_test_command("orders", Uuid::new_v4(), b"simple", 0);
    client.execute(cmd2).await.expect("SIMPLE failed");
    tokio::time::sleep(Duration::from_millis(100)).await;
    let count_after_simple = bus_handler_state.received_count().await;
    assert!(
        count_after_simple >= 2,
        "SIMPLE should publish to bus (got {})",
        count_after_simple
    );

    // CASCADE: should NOT publish to bus
    let count_before_cascade = bus_handler_state.received_count().await;
    let cmd3 = create_test_command("orders", Uuid::new_v4(), b"cascade", 0);
    client.execute_cascade(cmd3).await.expect("CASCADE failed");
    tokio::time::sleep(Duration::from_millis(100)).await;
    let count_after_cascade = bus_handler_state.received_count().await;
    assert_eq!(
        count_after_cascade, count_before_cascade,
        "CASCADE should NOT publish to bus"
    );
}
