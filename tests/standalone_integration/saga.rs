//! Saga integration tests — activation, filtering, cascading, and e2e workflows.

use crate::common::*;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use angzarr::proto::{command_page, event_page, page_header, MergeStrategy, PageHeader};
use async_trait::async_trait;
use prost_types::Any;
use tonic::Status;
use uuid::Uuid;

// ============================================================================
// Fixtures: saga_activation
// ============================================================================

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
    async fn handle(&self, source: &EventBook) -> Result<SagaResponse, Status> {
        self.triggered.store(true, Ordering::SeqCst);

        let mut commands = Vec::new();

        // For each event, produce a command to another domain
        let source_correlation_id = source
            .cover
            .as_ref()
            .map(|c| c.correlation_id.clone())
            .unwrap_or_default();

        for page in &source.pages {
            if let Some(event_page::Payload::Event(event)) = &page.payload {
                if event.type_url.contains("OrderPlaced") {
                    let cmd = CommandBook {
                        cover: Some(Cover {
                            domain: self.command_domain.clone(),
                            root: source.cover.as_ref().and_then(|c| c.root.clone()),
                            correlation_id: source_correlation_id.clone(),
                            edition: None,
                        }),
                        pages: vec![CommandPage {
                            header: Some(PageHeader {
                                sequence_type: Some(page_header::SequenceType::Sequence(0)),
                            }),
                            payload: Some(command_page::Payload::Command(Any {
                                type_url: "inventory.ReserveStock".to_string(),
                                value: event.value.clone(),
                            })),
                            merge_strategy: MergeStrategy::MergeCommutative as i32,
                        }],
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

/// Wrapper to make Arc<FulfillmentSaga> implement SagaHandler.
struct SagaWrapper(Arc<FulfillmentSaga>);

#[async_trait]
impl SagaHandler for SagaWrapper {
    async fn handle(&self, source: &EventBook) -> Result<SagaResponse, Status> {
        self.0.handle(source).await
    }
}

// ============================================================================
// Fixtures: e2e_saga_workflow
// ============================================================================

/// Saga that handles orders -> inventory.
struct OrdersToInventorySaga {
    step_count: Arc<AtomicU32>,
}

impl OrdersToInventorySaga {
    fn new(step_count: Arc<AtomicU32>) -> Self {
        Self { step_count }
    }
}

#[async_trait]
impl SagaHandler for OrdersToInventorySaga {
    async fn handle(&self, source: &EventBook) -> Result<SagaResponse, Status> {
        self.step_count.fetch_add(1, Ordering::SeqCst);

        let source_correlation_id = source
            .cover
            .as_ref()
            .map(|c| c.correlation_id.clone())
            .unwrap_or_default();

        let mut commands = Vec::new();

        for page in &source.pages {
            if let Some(event_page::Payload::Event(event)) = &page.payload {
                if event.type_url.contains("OrderPlaced") {
                    commands.push(CommandBook {
                        cover: Some(Cover {
                            domain: "inventory".to_string(),
                            root: source.cover.as_ref().and_then(|c| c.root.clone()),
                            correlation_id: source_correlation_id.clone(),
                            edition: None,
                        }),
                        pages: vec![CommandPage {
                            header: Some(PageHeader {
                                sequence_type: Some(page_header::SequenceType::Sequence(0)),
                            }),
                            payload: Some(command_page::Payload::Command(Any {
                                type_url: "inventory.ReserveStock".to_string(),
                                value: event.value.clone(),
                            })),
                            merge_strategy: MergeStrategy::MergeCommutative as i32,
                        }],
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

/// Saga that handles inventory -> shipping.
struct InventoryToShippingSaga {
    step_count: Arc<AtomicU32>,
}

impl InventoryToShippingSaga {
    fn new(step_count: Arc<AtomicU32>) -> Self {
        Self { step_count }
    }
}

#[async_trait]
impl SagaHandler for InventoryToShippingSaga {
    async fn handle(&self, source: &EventBook) -> Result<SagaResponse, Status> {
        self.step_count.fetch_add(1, Ordering::SeqCst);

        let source_correlation_id = source
            .cover
            .as_ref()
            .map(|c| c.correlation_id.clone())
            .unwrap_or_default();

        let mut commands = Vec::new();

        for page in &source.pages {
            if let Some(event_page::Payload::Event(event)) = &page.payload {
                if event.type_url.contains("ReserveStock") {
                    commands.push(CommandBook {
                        cover: Some(Cover {
                            domain: "shipping".to_string(),
                            root: source.cover.as_ref().and_then(|c| c.root.clone()),
                            correlation_id: source_correlation_id.clone(),
                            edition: None,
                        }),
                        pages: vec![CommandPage {
                            header: Some(PageHeader {
                                sequence_type: Some(page_header::SequenceType::Sequence(0)),
                            }),
                            payload: Some(command_page::Payload::Command(Any {
                                type_url: "shipping.CreateShipment".to_string(),
                                value: event.value.clone(),
                            })),
                            merge_strategy: MergeStrategy::MergeCommutative as i32,
                        }],
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

struct OrdersToInventoryWrapper(Arc<OrdersToInventorySaga>);

#[async_trait]
impl SagaHandler for OrdersToInventoryWrapper {
    async fn handle(&self, source: &EventBook) -> Result<SagaResponse, Status> {
        self.0.handle(source).await
    }
}

struct InventoryToShippingWrapper(Arc<InventoryToShippingSaga>);

#[async_trait]
impl SagaHandler for InventoryToShippingWrapper {
    async fn handle(&self, source: &EventBook) -> Result<SagaResponse, Status> {
        self.0.handle(source).await
    }
}

// ============================================================================
// Tests: saga activation
// ============================================================================

#[tokio::test]
async fn test_saga_receives_events_and_produces_commands() {
    let saga = Arc::new(FulfillmentSaga::new("inventory"));
    let saga_clone = saga.clone();

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_command_handler("orders", EchoAggregate::new())
        .register_command_handler("inventory", EchoAggregate::new())
        .register_saga(
            "fulfillment",
            SagaWrapper(saga_clone),
            SagaConfig::new("orders", "inventory"),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    // Start event distribution to projectors and sagas
    runtime.start().await.expect("Failed to start runtime");

    // Subscribe to record published events
    let event_bus = runtime.event_bus();
    let handler_state = RecordingHandlerState::new();
    let subscriber = event_bus.create_subscriber("test-sub", None).await.unwrap();
    subscriber
        .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
        .await
        .unwrap();
    subscriber.start_consuming().await.unwrap();

    let client = runtime.command_client();

    // Execute command that triggers saga
    let root = Uuid::new_v4();
    let mut command = create_test_command("orders", root, b"order-data", 0);
    command.pages[0].payload = Some(command_page::Payload::Command(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order-data".to_vec(),
    }));

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
        .register_command_handler("orders", EchoAggregate::new())
        .register_command_handler("products", EchoAggregate::new())
        .register_command_handler("inventory", EchoAggregate::new())
        .register_command_handler("warehouse", EchoAggregate::new())
        .register_saga(
            "orders-saga",
            SagaWrapper(orders_saga_clone),
            SagaConfig::new("orders", "inventory"),
        )
        .register_saga(
            "products-saga",
            SagaWrapper(products_saga_clone),
            SagaConfig::new("products", "warehouse"),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    // Start event distribution
    runtime.start().await.expect("Failed to start runtime");

    let client = runtime.command_client();

    // Execute orders command
    let mut orders_cmd = create_test_command("orders", Uuid::new_v4(), b"order", 0);
    orders_cmd.pages[0].payload = Some(command_page::Payload::Command(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order".to_vec(),
    }));
    client
        .execute(orders_cmd)
        .await
        .expect("Orders command failed");

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
    let mut products_cmd = create_test_command("products", Uuid::new_v4(), b"product", 0);
    products_cmd.pages[0].payload = Some(command_page::Payload::Command(Any {
        type_url: "products.ProductCreated".to_string(),
        value: b"product".to_vec(),
    }));
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
        .register_command_handler("orders", EchoAggregate::new())
        .register_command_handler("inventory", EchoAggregate::new())
        .register_saga(
            "fulfillment",
            SagaWrapper(saga_clone),
            SagaConfig::new("orders", "inventory"),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    // Start event distribution
    runtime.start().await.expect("Failed to start runtime");

    let event_bus = runtime.event_bus();
    let handler_state = RecordingHandlerState::new();
    let subscriber = event_bus.create_subscriber("test-sub", None).await.unwrap();
    subscriber
        .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
        .await
        .unwrap();
    subscriber.start_consuming().await.unwrap();

    let client = runtime.command_client();

    let correlation_id = "saga-correlation-test-123";
    let mut command = create_test_command("orders", Uuid::new_v4(), b"order", 0);
    if let Some(ref mut cover) = command.cover {
        cover.correlation_id = correlation_id.to_string();
    }
    command.pages[0].payload = Some(command_page::Payload::Command(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order".to_vec(),
    }));

    client.execute(command).await.expect("Command failed");
    tokio::time::sleep(Duration::from_millis(200)).await;

    let events = handler_state.get_events().await;

    // All events (from both orders and inventory) should have same correlation ID
    for event in &events {
        let event_correlation_id = event
            .cover
            .as_ref()
            .map(|c| c.correlation_id.as_str())
            .unwrap_or("");
        assert_eq!(
            event_correlation_id, correlation_id,
            "All events should preserve correlation ID"
        );
    }
}

#[tokio::test]
async fn test_saga_rejects_command_to_wrong_output_domain() {
    // Saga configured to output to "inventory" but tries to send to "shipping"
    let saga = Arc::new(FulfillmentSaga::new("shipping")); // produces commands to "shipping"
    let saga_clone = saga.clone();

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_command_handler("orders", EchoAggregate::new())
        .register_command_handler("shipping", EchoAggregate::new())
        .register_saga(
            "fulfillment",
            SagaWrapper(saga_clone),
            SagaConfig::new("orders", "inventory"), // but config says output to "inventory"
        )
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");

    // Record events to verify shipping doesn't receive commands
    let event_bus = runtime.event_bus();
    let handler_state = RecordingHandlerState::new();
    let subscriber = event_bus.create_subscriber("test-sub", None).await.unwrap();
    subscriber
        .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
        .await
        .unwrap();
    subscriber.start_consuming().await.unwrap();

    let client = runtime.command_client();

    // Execute command that triggers saga
    let mut cmd = create_test_command("orders", Uuid::new_v4(), b"order", 0);
    cmd.pages[0].payload = Some(command_page::Payload::Command(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order".to_vec(),
    }));
    client.execute(cmd).await.expect("Command failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify that shipping did NOT receive any events
    // (the saga command should have been rejected due to domain mismatch)
    let events = handler_state.get_events().await;
    let shipping_events: Vec<_> = events
        .iter()
        .filter(|e| {
            e.cover
                .as_ref()
                .map(|c| c.domain == "shipping")
                .unwrap_or(false)
        })
        .collect();

    assert!(
        shipping_events.is_empty(),
        "Shipping should NOT receive events when saga targets wrong output domain"
    );
}

#[tokio::test]
async fn test_saga_only_receives_events_from_input_domain() {
    let orders_saga = Arc::new(FulfillmentSaga::new("inventory"));
    let orders_saga_clone = orders_saga.clone();

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_command_handler("orders", EchoAggregate::new())
        .register_command_handler("products", EchoAggregate::new())
        .register_command_handler("inventory", EchoAggregate::new())
        .register_saga(
            "orders-saga",
            SagaWrapper(orders_saga_clone),
            SagaConfig::new("orders", "inventory"), // only listens to "orders"
        )
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");

    let client = runtime.command_client();

    // Execute command on "products" domain (saga should NOT be triggered)
    let mut products_cmd = create_test_command("products", Uuid::new_v4(), b"product", 0);
    products_cmd.pages[0].payload = Some(command_page::Payload::Command(Any {
        type_url: "products.OrderPlaced".to_string(), // Even with matching event type
        value: b"product".to_vec(),
    }));
    client
        .execute(products_cmd)
        .await
        .expect("Products command failed");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Saga should NOT have been triggered (wrong input domain)
    assert!(
        !orders_saga.was_triggered(),
        "Saga should NOT be triggered for products domain (input_domain is 'orders')"
    );

    // Now execute on "orders" domain
    let mut orders_cmd = create_test_command("orders", Uuid::new_v4(), b"order", 0);
    orders_cmd.pages[0].payload = Some(command_page::Payload::Command(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order".to_vec(),
    }));
    client
        .execute(orders_cmd)
        .await
        .expect("Orders command failed");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Now saga should have been triggered
    assert!(
        orders_saga.was_triggered(),
        "Saga SHOULD be triggered for orders domain"
    );
}

// ========================================================================
// Two-Phase Protocol Tests: Demonstrating Three Saga Outcomes
// ========================================================================

/// Saga translates source events to commands.
///
/// In the delivery-retry model:
/// - Saga receives only source events (no destination state)
/// - Saga produces commands with deferred sequences
/// - Framework handles delivery retry on sequence conflicts
#[tokio::test]
async fn test_saga_translates_source_to_commands() {
    let handle_called = Arc::new(AtomicBool::new(false));
    let handle_called_clone = handle_called.clone();

    /// Saga that produces commands from source events
    struct TranslatorSaga {
        handle_called: Arc<AtomicBool>,
    }

    #[async_trait]
    impl SagaHandler for TranslatorSaga {
        async fn handle(&self, source: &EventBook) -> Result<SagaResponse, Status> {
            self.handle_called.store(true, Ordering::SeqCst);

            let has_order_placed = source.pages.iter().any(|p| {
                matches!(&p.payload, Some(event_page::Payload::Event(e)) if e.type_url.contains("OrderPlaced"))
            });

            if has_order_placed {
                let source_correlation_id = source
                    .cover
                    .as_ref()
                    .map(|c| c.correlation_id.clone())
                    .unwrap_or_default();
                Ok(SagaResponse {
                    commands: vec![CommandBook {
                        cover: Some(Cover {
                            domain: "inventory".to_string(),
                            root: source.cover.as_ref().and_then(|c| c.root.clone()),
                            correlation_id: source_correlation_id,
                            edition: None,
                        }),
                        pages: vec![CommandPage {
                            header: Some(PageHeader {
                                sequence_type: Some(page_header::SequenceType::Sequence(0)),
                            }),
                            payload: Some(command_page::Payload::Command(Any {
                                type_url: "inventory.Reserve".to_string(),
                                value: b"reserve".to_vec(),
                            })),
                            merge_strategy: MergeStrategy::MergeCommutative as i32,
                        }],
                    }],
                    ..Default::default()
                })
            } else {
                Ok(SagaResponse::default())
            }
        }
    }

    let saga = TranslatorSaga {
        handle_called: handle_called_clone,
    };

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_command_handler("orders", EchoAggregate::new())
        .register_command_handler("inventory", EchoAggregate::new())
        .register_saga(
            "order-to-inventory",
            saga,
            SagaConfig::new("orders", "inventory"),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");

    let client = runtime.command_client();
    let mut order_cmd = create_test_command("orders", Uuid::new_v4(), b"order-data", 0);
    order_cmd.pages[0].payload = Some(command_page::Payload::Command(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order-data".to_vec(),
    }));
    client.execute(order_cmd).await.expect("Order failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(
        handle_called.load(Ordering::SeqCst),
        "handle() should have been called"
    );
}

/// Saga produces commands from source events only.
///
/// The framework handles all delivery concerns - saga is a pure translator.
#[tokio::test]
async fn test_saga_produces_commands_from_source() {
    let handle_called = Arc::new(AtomicBool::new(false));
    let handle_called_clone = handle_called.clone();

    /// Simple saga that produces commands based on source events
    struct SimpleFulfillmentSaga {
        handle_called: Arc<AtomicBool>,
    }

    #[async_trait]
    impl SagaHandler for SimpleFulfillmentSaga {
        async fn handle(&self, source: &EventBook) -> Result<SagaResponse, Status> {
            self.handle_called.store(true, Ordering::SeqCst);

            let has_order = source.pages.iter().any(|p| {
                matches!(&p.payload, Some(event_page::Payload::Event(e)) if e.type_url.contains("OrderPlaced"))
            });

            if has_order {
                let source_correlation_id = source
                    .cover
                    .as_ref()
                    .map(|c| c.correlation_id.clone())
                    .unwrap_or_default();
                Ok(SagaResponse {
                    commands: vec![CommandBook {
                        cover: Some(Cover {
                            domain: "shipping".to_string(),
                            root: source.cover.as_ref().and_then(|c| c.root.clone()),
                            correlation_id: source_correlation_id,
                            edition: None,
                        }),
                        pages: vec![CommandPage {
                            header: Some(PageHeader {
                                sequence_type: Some(page_header::SequenceType::Sequence(0)),
                            }),
                            payload: Some(command_page::Payload::Command(Any {
                                type_url: "shipping.CreateShipment".to_string(),
                                value: b"ship".to_vec(),
                            })),
                            merge_strategy: MergeStrategy::MergeCommutative as i32,
                        }],
                    }],
                    ..Default::default()
                })
            } else {
                Ok(SagaResponse::default())
            }
        }
    }

    let saga = SimpleFulfillmentSaga {
        handle_called: handle_called_clone,
    };

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_command_handler("orders", EchoAggregate::new())
        .register_command_handler("shipping", EchoAggregate::new())
        .register_saga(
            "simple-fulfillment",
            saga,
            SagaConfig::new("orders", "shipping"),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");

    let client = runtime.command_client();
    let mut cmd = create_test_command("orders", Uuid::new_v4(), b"order", 0);
    cmd.pages[0].payload = Some(command_page::Payload::Command(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order".to_vec(),
    }));
    client.execute(cmd).await.expect("Command failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(
        handle_called.load(Ordering::SeqCst),
        "handle() should have been called"
    );
}

/// Saga no-op: returns empty commands for irrelevant events.
///
/// Sagas may legitimately produce zero commands when source events
/// don't require translation to the target domain.
#[tokio::test]
async fn test_saga_noop_returns_empty_commands() {
    let handle_called = Arc::new(AtomicBool::new(false));
    let handle_called_clone = handle_called.clone();

    /// Saga that only acts on specific events - returns empty for others
    struct SelectiveSaga {
        handle_called: Arc<AtomicBool>,
    }

    #[async_trait]
    impl SagaHandler for SelectiveSaga {
        async fn handle(&self, source: &EventBook) -> Result<SagaResponse, Status> {
            self.handle_called.store(true, Ordering::SeqCst);

            // Only act on "SpecialEvent", ignore everything else
            let has_special = source.pages.iter().any(|p| {
                matches!(&p.payload, Some(event_page::Payload::Event(e)) if e.type_url.contains("SpecialEvent"))
            });

            if has_special {
                let source_correlation_id = source
                    .cover
                    .as_ref()
                    .map(|c| c.correlation_id.clone())
                    .unwrap_or_default();
                Ok(SagaResponse {
                    commands: vec![CommandBook {
                        cover: Some(Cover {
                            domain: "target".to_string(),
                            root: source.cover.as_ref().and_then(|c| c.root.clone()),
                            correlation_id: source_correlation_id,
                            edition: None,
                        }),
                        pages: vec![CommandPage {
                            header: Some(PageHeader {
                                sequence_type: Some(page_header::SequenceType::Sequence(0)),
                            }),
                            payload: Some(command_page::Payload::Command(Any {
                                type_url: "target.DoSomething".to_string(),
                                value: b"data".to_vec(),
                            })),
                            merge_strategy: MergeStrategy::MergeCommutative as i32,
                        }],
                    }],
                    ..Default::default()
                })
            } else {
                // No-op: return empty commands
                Ok(SagaResponse::default())
            }
        }
    }

    let saga = SelectiveSaga {
        handle_called: handle_called_clone,
    };

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_command_handler("orders", EchoAggregate::new())
        .register_command_handler("target", EchoAggregate::new())
        .register_saga("selective-saga", saga, SagaConfig::new("orders", "target"))
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");

    // Record events to verify no target commands were executed
    let event_bus = runtime.event_bus();
    let handler_state = RecordingHandlerState::new();
    let subscriber = event_bus.create_subscriber("test-sub", None).await.unwrap();
    subscriber
        .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
        .await
        .unwrap();
    subscriber.start_consuming().await.unwrap();

    let client = runtime.command_client();

    // Execute a non-special event
    let mut cmd = create_test_command("orders", Uuid::new_v4(), b"regular", 0);
    cmd.pages[0].payload = Some(command_page::Payload::Command(Any {
        type_url: "orders.RegularEvent".to_string(), // NOT "SpecialEvent"
        value: b"regular".to_vec(),
    }));
    client.execute(cmd).await.expect("Command failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(
        handle_called.load(Ordering::SeqCst),
        "handle() should still be called even for no-op"
    );

    // Verify no commands went to target domain
    let events = handler_state.get_events().await;
    let target_events: Vec<_> = events
        .iter()
        .filter(|e| {
            e.cover
                .as_ref()
                .map(|c| c.domain == "target")
                .unwrap_or(false)
        })
        .collect();

    assert!(
        target_events.is_empty(),
        "No events should go to target domain when saga returns empty commands (no-op)"
    );
}

// ============================================================================
// Tests: e2e saga workflow
// ============================================================================

/// When event_bus is an external transport (IPC, AMQP), sagas must still
/// receive events via dual-publish to the internal channel bus.
#[tokio::test]
async fn test_saga_cascade_with_external_event_bus() {
    let step_count = Arc::new(AtomicU32::new(0));
    let orders_saga = Arc::new(OrdersToInventorySaga::new(step_count.clone()));
    let inventory_saga = Arc::new(InventoryToShippingSaga::new(step_count.clone()));

    // Simulate an external transport (IPC/AMQP) — a separate bus that
    // does NOT deliver to the in-process channel bus.
    let external_bus: Arc<dyn EventBus> = Arc::new(angzarr::bus::ChannelEventBus::new(
        angzarr::bus::ChannelConfig::publisher(),
    ));

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .with_event_bus(external_bus)
        .register_command_handler("orders", EchoAggregate::new())
        .register_command_handler("inventory", EchoAggregate::new())
        .register_command_handler("shipping", EchoAggregate::new())
        .register_saga(
            "orders-to-inventory",
            OrdersToInventoryWrapper(orders_saga),
            SagaConfig::new("orders", "inventory"),
        )
        .register_saga(
            "inventory-to-shipping",
            InventoryToShippingWrapper(inventory_saga),
            SagaConfig::new("inventory", "shipping"),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");
    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = runtime.command_client();

    let root = Uuid::new_v4();
    let mut cmd = create_test_command("orders", root, b"order-data", 0);
    cmd.pages[0].payload = Some(command_page::Payload::Command(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order-123".to_vec(),
    }));

    client.execute(cmd).await.expect("Initial command failed");
    tokio::time::sleep(Duration::from_millis(800)).await;

    // Verify saga cascade completed across all three domains
    let steps = step_count.load(Ordering::SeqCst);
    assert!(
        steps >= 2,
        "Both sagas should trigger (got {} steps)",
        steps
    );

    let orders_events = runtime
        .event_store("orders")
        .unwrap()
        .get("orders", DEFAULT_EDITION, root)
        .await
        .unwrap();
    let inventory_events = runtime
        .event_store("inventory")
        .unwrap()
        .get("inventory", DEFAULT_EDITION, root)
        .await
        .unwrap();
    let shipping_events = runtime
        .event_store("shipping")
        .unwrap()
        .get("shipping", DEFAULT_EDITION, root)
        .await
        .unwrap();

    assert!(!orders_events.is_empty(), "Orders should have events");
    assert!(
        !inventory_events.is_empty(),
        "Inventory should have events (saga cascade from orders)"
    );
    assert!(
        !shipping_events.is_empty(),
        "Shipping should have events (saga cascade from inventory)"
    );
}

#[tokio::test]
async fn test_saga_chains_across_three_domains() {
    let step_count = Arc::new(AtomicU32::new(0));
    let orders_saga = Arc::new(OrdersToInventorySaga::new(step_count.clone()));
    let inventory_saga = Arc::new(InventoryToShippingSaga::new(step_count.clone()));

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_command_handler("orders", EchoAggregate::new())
        .register_command_handler("inventory", EchoAggregate::new())
        .register_command_handler("shipping", EchoAggregate::new())
        .register_saga(
            "orders-to-inventory",
            OrdersToInventoryWrapper(orders_saga),
            SagaConfig::new("orders", "inventory"),
        )
        .register_saga(
            "inventory-to-shipping",
            InventoryToShippingWrapper(inventory_saga),
            SagaConfig::new("inventory", "shipping"),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");

    // Wait for saga consumers to initialize
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Record all events
    let event_bus = runtime.event_bus();
    let handler_state = RecordingHandlerState::new();
    let subscriber = event_bus.create_subscriber("test-sub", None).await.unwrap();
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
    let mut cmd = create_test_command("orders", root, b"order-data", 0);
    cmd.pages[0].payload = Some(command_page::Payload::Command(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order-123".to_vec(),
    }));
    if let Some(ref mut cover) = cmd.cover {
        cover.correlation_id = "e2e-test-correlation".to_string();
    }

    client.execute(cmd).await.expect("Initial command failed");

    // Wait for full saga chain to complete (order -> inventory -> shipping)
    tokio::time::sleep(Duration::from_millis(800)).await;

    // Verify sagas were triggered
    let steps = step_count.load(Ordering::SeqCst);
    assert!(
        steps >= 1,
        "Sagas should be triggered at least once (got {} steps)",
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
        let event_correlation_id = event
            .cover
            .as_ref()
            .map(|c| c.correlation_id.as_str())
            .unwrap_or("");
        assert_eq!(
            event_correlation_id, "e2e-test-correlation",
            "All events should preserve correlation ID through saga chain"
        );
    }

    // Verify storage has events for each domain with same root
    let orders_events = runtime
        .event_store("orders")
        .unwrap()
        .get("orders", DEFAULT_EDITION, root)
        .await
        .unwrap();
    let inventory_events = runtime
        .event_store("inventory")
        .unwrap()
        .get("inventory", DEFAULT_EDITION, root)
        .await
        .unwrap();
    let shipping_events = runtime
        .event_store("shipping")
        .unwrap()
        .get("shipping", DEFAULT_EDITION, root)
        .await
        .unwrap();

    assert!(!orders_events.is_empty(), "Orders should have events");
    assert!(!inventory_events.is_empty(), "Inventory should have events");
    assert!(!shipping_events.is_empty(), "Shipping should have events");
}

#[tokio::test]
async fn test_multiple_saga_chains_sequential() {
    // Test multiple saga chains execute correctly when run sequentially
    let step_count = Arc::new(AtomicU32::new(0));
    let orders_saga = Arc::new(OrdersToInventorySaga::new(step_count.clone()));
    let inventory_saga = Arc::new(InventoryToShippingSaga::new(step_count.clone()));

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_command_handler("orders", EchoAggregate::new())
        .register_command_handler("inventory", EchoAggregate::new())
        .register_command_handler("shipping", EchoAggregate::new())
        .register_saga(
            "orders-to-inventory",
            OrdersToInventoryWrapper(orders_saga),
            SagaConfig::new("orders", "inventory"),
        )
        .register_saga(
            "inventory-to-shipping",
            InventoryToShippingWrapper(inventory_saga),
            SagaConfig::new("inventory", "shipping"),
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
        let mut cmd = create_test_command("orders", root, format!("order-{}", i).as_bytes(), 0);
        cmd.pages[0].payload = Some(command_page::Payload::Command(Any {
            type_url: "orders.OrderPlaced".to_string(),
            value: format!("order-{}", i).into_bytes(),
        }));
        if let Some(ref mut cover) = cmd.cover {
            cover.correlation_id = format!("sequential-{}", i);
        }
        client.execute(cmd).await.expect("Command failed");
        roots.push(root);

        // Wait longer for saga chain to complete before next
        // (Orders -> Inventory -> Shipping takes multiple hops)
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Verify each root has events in all three domains
    for (i, root) in roots.iter().enumerate() {
        let orders_events = runtime
            .event_store("orders")
            .unwrap()
            .get("orders", DEFAULT_EDITION, *root)
            .await
            .unwrap();
        let inventory_events = runtime
            .event_store("inventory")
            .unwrap()
            .get("inventory", DEFAULT_EDITION, *root)
            .await
            .unwrap();
        let shipping_events = runtime
            .event_store("shipping")
            .unwrap()
            .get("shipping", DEFAULT_EDITION, *root)
            .await
            .unwrap();

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
