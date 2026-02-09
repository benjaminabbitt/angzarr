//! Saga integration tests — activation, filtering, cascading, and e2e workflows.

use crate::common::*;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

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
    /// This saga doesn't need destination state - returns empty covers
    async fn prepare(&self, _source: &EventBook) -> Result<Vec<Cover>, Status> {
        Ok(vec![])
    }

    async fn execute(
        &self,
        source: &EventBook,
        _destinations: &[EventBook],
    ) -> Result<SagaResponse, Status> {
        self.triggered.store(true, Ordering::SeqCst);

        let mut commands = Vec::new();

        // For each event, produce a command to another domain
        let source_correlation_id = source
            .cover
            .as_ref()
            .map(|c| c.correlation_id.clone())
            .unwrap_or_default();

        for page in &source.pages {
            if let Some(event) = &page.event {
                if event.type_url.contains("OrderPlaced") {
                    let cmd = CommandBook {
                        cover: Some(Cover {
                            domain: self.command_domain.clone(),
                            root: source.cover.as_ref().and_then(|c| c.root.clone()),
                            correlation_id: source_correlation_id.clone(),
                            edition: None,
                        }),
                        pages: vec![CommandPage {
                            sequence: 0,
                            command: Some(Any {
                                type_url: "inventory.ReserveStock".to_string(),
                                value: event.value.clone(),
                            }),
                        }],
                        ..Default::default()
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
    async fn prepare(&self, _source: &EventBook) -> Result<Vec<Cover>, Status> {
        Ok(vec![]) // No destination state needed
    }

    async fn execute(
        &self,
        source: &EventBook,
        _destinations: &[EventBook],
    ) -> Result<SagaResponse, Status> {
        self.step_count.fetch_add(1, Ordering::SeqCst);

        let source_correlation_id = source
            .cover
            .as_ref()
            .map(|c| c.correlation_id.clone())
            .unwrap_or_default();

        let mut commands = Vec::new();

        for page in &source.pages {
            if let Some(event) = &page.event {
                if event.type_url.contains("OrderPlaced") {
                    commands.push(CommandBook {
                        cover: Some(Cover {
                            domain: "inventory".to_string(),
                            root: source.cover.as_ref().and_then(|c| c.root.clone()),
                            correlation_id: source_correlation_id.clone(),
                            edition: None,
                        }),
                        pages: vec![CommandPage {
                            sequence: 0,
                            command: Some(Any {
                                type_url: "inventory.ReserveStock".to_string(),
                                value: event.value.clone(),
                            }),
                        }],
                        ..Default::default()
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
    async fn prepare(&self, _source: &EventBook) -> Result<Vec<Cover>, Status> {
        Ok(vec![]) // No destination state needed
    }

    async fn execute(
        &self,
        source: &EventBook,
        _destinations: &[EventBook],
    ) -> Result<SagaResponse, Status> {
        self.step_count.fetch_add(1, Ordering::SeqCst);

        let source_correlation_id = source
            .cover
            .as_ref()
            .map(|c| c.correlation_id.clone())
            .unwrap_or_default();

        let mut commands = Vec::new();

        for page in &source.pages {
            if let Some(event) = &page.event {
                if event.type_url.contains("ReserveStock") {
                    commands.push(CommandBook {
                        cover: Some(Cover {
                            domain: "shipping".to_string(),
                            root: source.cover.as_ref().and_then(|c| c.root.clone()),
                            correlation_id: source_correlation_id.clone(),
                            edition: None,
                        }),
                        pages: vec![CommandPage {
                            sequence: 0,
                            command: Some(Any {
                                type_url: "shipping.CreateShipment".to_string(),
                                value: event.value.clone(),
                            }),
                        }],
                        ..Default::default()
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

struct InventoryToShippingWrapper(Arc<InventoryToShippingSaga>);

#[async_trait]
impl SagaHandler for InventoryToShippingWrapper {
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

// ============================================================================
// Tests: saga activation
// ============================================================================

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
    orders_cmd.pages[0].command = Some(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order".to_vec(),
    });
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
    command.pages[0].command = Some(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order".to_vec(),
    });

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
        .register_aggregate("orders", EchoAggregate::new())
        .register_aggregate("shipping", EchoAggregate::new())
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
    cmd.pages[0].command = Some(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order".to_vec(),
    });
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
        .register_aggregate("orders", EchoAggregate::new())
        .register_aggregate("products", EchoAggregate::new())
        .register_aggregate("inventory", EchoAggregate::new())
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
    products_cmd.pages[0].command = Some(Any {
        type_url: "products.OrderPlaced".to_string(), // Even with matching event type
        value: b"product".to_vec(),
    });
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
    orders_cmd.pages[0].command = Some(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order".to_vec(),
    });
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

/// Outcome 1: Saga needs destination state
/// - prepare() returns covers of aggregates to fetch
/// - framework fetches those EventBooks from event store
/// - execute() receives source + destinations
#[tokio::test]
async fn test_two_phase_saga_fetches_destinations() {
    // Track whether prepare and execute were called with correct data
    let prepare_called = Arc::new(AtomicBool::new(false));
    let execute_called = Arc::new(AtomicBool::new(false));
    let destinations_received = Arc::new(AtomicBool::new(false));

    let prepare_called_clone = prepare_called.clone();
    let execute_called_clone = execute_called.clone();
    let destinations_received_clone = destinations_received.clone();

    /// Saga that needs to check inventory state before reserving
    struct InventoryCheckingSaga {
        prepare_called: Arc<AtomicBool>,
        execute_called: Arc<AtomicBool>,
        destinations_received: Arc<AtomicBool>,
        inventory_root: Uuid,
    }

    #[async_trait]
    impl SagaHandler for InventoryCheckingSaga {
        async fn prepare(&self, source: &EventBook) -> Result<Vec<Cover>, Status> {
            self.prepare_called.store(true, Ordering::SeqCst);

            // Check if this is an OrderPlaced event
            let has_order_placed = source.pages.iter().any(|p| {
                p.event
                    .as_ref()
                    .map(|e| e.type_url.contains("OrderPlaced"))
                    .unwrap_or(false)
            });

            if has_order_placed {
                // Request the inventory aggregate's current state
                Ok(vec![Cover {
                    domain: "inventory".to_string(),
                    root: Some(ProtoUuid {
                        value: self.inventory_root.as_bytes().to_vec(),
                    }),
                    correlation_id: String::new(),
                    edition: None,
                }])
            } else {
                Ok(vec![])
            }
        }

        async fn execute(
            &self,
            source: &EventBook,
            destinations: &[EventBook],
        ) -> Result<SagaResponse, Status> {
            self.execute_called.store(true, Ordering::SeqCst);

            // Verify we received the correct destination state
            if !destinations.is_empty() {
                self.destinations_received.store(true, Ordering::SeqCst);

                // Validate the destination is the inventory aggregate we requested
                let dest = &destinations[0];
                let cover = dest.cover.as_ref().expect("Destination should have cover");
                assert_eq!(cover.domain, "inventory", "Should fetch inventory domain");

                let root = cover.root.as_ref().expect("Destination should have root");
                let fetched_root = Uuid::from_slice(&root.value).unwrap();
                assert_eq!(
                    fetched_root, self.inventory_root,
                    "Should fetch the exact aggregate we requested in prepare()"
                );

                // Verify it has the event we created earlier
                assert!(
                    !dest.pages.is_empty(),
                    "Destination should have events from the inventory aggregate we created"
                );
                let first_event = dest.pages[0].event.as_ref().expect("Should have event");
                assert!(
                    first_event.type_url.contains("CreateProduct"),
                    "Fetched inventory should contain the CreateProduct event we stored"
                );
            }

            // Generate reservation command based on inventory state
            let has_order_placed = source.pages.iter().any(|p| {
                p.event
                    .as_ref()
                    .map(|e| e.type_url.contains("OrderPlaced"))
                    .unwrap_or(false)
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
                            sequence: 0,
                            command: Some(Any {
                                type_url: "inventory.Reserve".to_string(),
                                value: b"reserve".to_vec(),
                            }),
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                })
            } else {
                Ok(SagaResponse::default())
            }
        }
    }

    let inventory_root = Uuid::new_v4();
    let saga = InventoryCheckingSaga {
        prepare_called: prepare_called_clone,
        execute_called: execute_called_clone,
        destinations_received: destinations_received_clone,
        inventory_root,
    };

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .register_aggregate("inventory", EchoAggregate::new())
        .register_saga(
            "inventory-checker",
            saga,
            SagaConfig::new("orders", "inventory"),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");

    // First, create some inventory state so there's something to fetch
    let client = runtime.command_client();
    let inv_cmd = CommandBook {
        cover: Some(Cover {
            domain: "inventory".to_string(),
            root: Some(ProtoUuid {
                value: inventory_root.as_bytes().to_vec(),
            }),
            correlation_id: "setup".to_string(),
            edition: None,
        }),
        pages: vec![CommandPage {
            sequence: 0,
            command: Some(Any {
                type_url: "inventory.CreateProduct".to_string(),
                value: b"product-data".to_vec(),
            }),
        }],
        ..Default::default()
    };
    client
        .execute(inv_cmd)
        .await
        .expect("Inventory setup failed");
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Now execute an order (triggers saga)
    let mut order_cmd = create_test_command("orders", Uuid::new_v4(), b"order-data", 0);
    order_cmd.pages[0].command = Some(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order-data".to_vec(),
    });
    client.execute(order_cmd).await.expect("Order failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify two-phase protocol was followed
    assert!(
        prepare_called.load(Ordering::SeqCst),
        "prepare() should have been called"
    );
    assert!(
        execute_called.load(Ordering::SeqCst),
        "execute() should have been called"
    );
    assert!(
        destinations_received.load(Ordering::SeqCst),
        "execute() should have received destination EventBooks from prepare()"
    );
}

/// Outcome 2: Saga doesn't need destination state
/// - prepare() returns empty vec
/// - framework calls execute() immediately (no fetch)
/// - execute() produces commands from source events only
#[tokio::test]
async fn test_two_phase_saga_no_destinations_needed() {
    let prepare_called = Arc::new(AtomicBool::new(false));
    let execute_called = Arc::new(AtomicBool::new(false));

    let prepare_called_clone = prepare_called.clone();
    let execute_called_clone = execute_called.clone();

    /// Simple saga that doesn't need any destination state
    struct SimpleFulfillmentSaga {
        prepare_called: Arc<AtomicBool>,
        execute_called: Arc<AtomicBool>,
    }

    #[async_trait]
    impl SagaHandler for SimpleFulfillmentSaga {
        async fn prepare(&self, _source: &EventBook) -> Result<Vec<Cover>, Status> {
            self.prepare_called.store(true, Ordering::SeqCst);
            // No destinations needed - we just react to events
            Ok(vec![])
        }

        async fn execute(
            &self,
            source: &EventBook,
            destinations: &[EventBook],
        ) -> Result<SagaResponse, Status> {
            self.execute_called.store(true, Ordering::SeqCst);

            // Verify no destinations were passed (since we didn't request any)
            assert!(
                destinations.is_empty(),
                "Should receive no destinations when prepare() returns empty"
            );

            // Produce command based only on source events
            let has_order = source.pages.iter().any(|p| {
                p.event
                    .as_ref()
                    .map(|e| e.type_url.contains("OrderPlaced"))
                    .unwrap_or(false)
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
                            sequence: 0,
                            command: Some(Any {
                                type_url: "shipping.CreateShipment".to_string(),
                                value: b"ship".to_vec(),
                            }),
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                })
            } else {
                Ok(SagaResponse::default())
            }
        }
    }

    let saga = SimpleFulfillmentSaga {
        prepare_called: prepare_called_clone,
        execute_called: execute_called_clone,
    };

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .register_aggregate("shipping", EchoAggregate::new())
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
    cmd.pages[0].command = Some(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order".to_vec(),
    });
    client.execute(cmd).await.expect("Command failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(
        prepare_called.load(Ordering::SeqCst),
        "prepare() should have been called"
    );
    assert!(
        execute_called.load(Ordering::SeqCst),
        "execute() should have been called"
    );
}

/// Outcome 3: Saga doesn't act on this event (no-op)
/// - prepare() may or may not be called
/// - execute() returns empty commands vec
/// - No commands are executed
#[tokio::test]
async fn test_two_phase_saga_noop_returns_empty_commands() {
    let execute_called = Arc::new(AtomicBool::new(false));
    let execute_called_clone = execute_called.clone();

    /// Saga that only acts on specific events - returns empty for others
    struct SelectiveSaga {
        execute_called: Arc<AtomicBool>,
    }

    #[async_trait]
    impl SagaHandler for SelectiveSaga {
        async fn prepare(&self, _source: &EventBook) -> Result<Vec<Cover>, Status> {
            Ok(vec![])
        }

        async fn execute(
            &self,
            source: &EventBook,
            _destinations: &[EventBook],
        ) -> Result<SagaResponse, Status> {
            self.execute_called.store(true, Ordering::SeqCst);

            // Only act on "SpecialEvent", ignore everything else
            let has_special = source.pages.iter().any(|p| {
                p.event
                    .as_ref()
                    .map(|e| e.type_url.contains("SpecialEvent"))
                    .unwrap_or(false)
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
                            sequence: 0,
                            command: Some(Any {
                                type_url: "target.DoSomething".to_string(),
                                value: b"data".to_vec(),
                            }),
                        }],
                        ..Default::default()
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
        execute_called: execute_called_clone,
    };

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .register_aggregate("target", EchoAggregate::new())
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
    cmd.pages[0].command = Some(Any {
        type_url: "orders.RegularEvent".to_string(), // NOT "SpecialEvent"
        value: b"regular".to_vec(),
    });
    client.execute(cmd).await.expect("Command failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(
        execute_called.load(Ordering::SeqCst),
        "execute() should still be called even for no-op"
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
        .register_aggregate("orders", EchoAggregate::new())
        .register_aggregate("inventory", EchoAggregate::new())
        .register_aggregate("shipping", EchoAggregate::new())
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
    cmd.pages[0].command = Some(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order-123".to_vec(),
    });

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
        .register_aggregate("orders", EchoAggregate::new())
        .register_aggregate("inventory", EchoAggregate::new())
        .register_aggregate("shipping", EchoAggregate::new())
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
    cmd.pages[0].command = Some(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order-123".to_vec(),
    });
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
        .register_aggregate("orders", EchoAggregate::new())
        .register_aggregate("inventory", EchoAggregate::new())
        .register_aggregate("shipping", EchoAggregate::new())
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
        cmd.pages[0].command = Some(Any {
            type_url: "orders.OrderPlaced".to_string(),
            value: format!("order-{}", i).into_bytes(),
        });
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
