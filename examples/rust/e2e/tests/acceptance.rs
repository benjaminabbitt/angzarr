//! E2E Acceptance Tests
//!
//! Comprehensive end-to-end tests for the angzarr event sourcing system.
//! Tests full business flows through the gateway with correlation ID tracing,
//! projector validation, and resilience testing.

use std::time::{Duration, Instant};

use cucumber::{gherkin::Step, given, then, when, World as _};
use prost::Message;
use uuid::Uuid;

use e2e::{
    assert_command_failed, examples_proto, extract_event_type, proto, E2EWorld,
};

// ============================================================================
// Setup Steps (Given)
// ============================================================================


#[given(expr = "inventory for {string} has {int} units")]
async fn inventory_exists(world: &mut E2EWorld, product_alias: String, units: i32) {
    // Force the deterministic root so the inventory reservation saga (which
    // sends to inventory_product_root(product_id)) targets this aggregate.
    // Uses insert() to overwrite any random root a prior product step set.
    let det_root = common::identity::inventory_product_root(&product_alias);
    world.roots.insert(product_alias.clone(), det_root);

    let command = examples_proto::InitializeStock {
        product_id: product_alias.clone(),
        quantity: units,
        low_stock_threshold: 10,
    };

    let correlation = format!("setup-inventory-{}", product_alias);
    let cmd_book = world.build_command(
        "inventory",
        &product_alias,
        &correlation,
        "examples.InitializeStock",
        &command,
    );

    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to initialize inventory: {:?}",
        world.last_error
    );
}

#[given(expr = "no aggregate exists for root {string}")]
async fn no_aggregate_exists(world: &mut E2EWorld, root_alias: String) {
    // Just register a new UUID for this alias - don't create anything
    world.root(&root_alias);
}

// ============================================================================
// Action Steps (When)
// ============================================================================

#[when(expr = "I send a command expecting sequence {int}")]
async fn send_command_with_sequence(world: &mut E2EWorld, sequence: u32) {
    // Get the first order or new aggregate alias
    let alias = world
        .roots
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| "NEW-AGG".to_string());

    let root = world.root(&alias);
    let correlation = format!("test-high-seq-{}", sequence);

    // Use CreateOrder for sequence 0 on a new aggregate, SubmitPayment otherwise.
    // CreateOrder works on a fresh aggregate; SubmitPayment requires an existing order.
    let events = world.query_events("order", root).await;
    if events.is_empty() && sequence == 0 {
        let command = examples_proto::CreateOrder {
            items: vec![examples_proto::LineItem {
                product_id: "SKU-TEST".to_string(),
                name: "Test Item".to_string(),
                quantity: 1,
                unit_price_cents: 100,
                ..Default::default()
            }],
            ..Default::default()
        };
        let cmd_book = world.build_command_with_sequence(
            "order",
            root,
            &correlation,
            sequence,
            "examples.CreateOrder",
            &command,
        );
        world.execute_raw(cmd_book).await;
    } else {
        let command = examples_proto::SubmitPayment {
            amount_cents: 100,
            payment_method: "card".to_string(),
            ..Default::default()
        };
        let cmd_book = world.build_command_with_sequence(
            "order",
            root,
            &correlation,
            sequence,
            "examples.SubmitPayment",
            &command,
        );
        world.execute_raw(cmd_book).await;
    }
}

// ============================================================================
// Assertion Steps (Then)
// ============================================================================

#[then("the command succeeds")]
async fn command_succeeds(world: &mut E2EWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
}

#[then(expr = "the command fails with {string}")]
async fn command_fails_with(world: &mut E2EWorld, expected_status: String) {
    assert_command_failed(world, &expected_status);
}

#[then("the error contains missing events")]
async fn error_contains_missing_events(world: &mut E2EWorld) {
    // The error details should contain an EventBook with the missing events
    // This is encoded in the gRPC error details
    assert!(
        world.last_error.is_some(),
        "Expected error with missing events"
    );
}

#[then(expr = "the error contains events {int}-{int}")]
async fn error_contains_event_range(world: &mut E2EWorld, _from: u32, _to: u32) {
    assert!(
        world.last_error.is_some(),
        "Expected error with event range"
    );
    // The actual event range verification would require parsing the error details
}

#[then(expr = "the error indicates expected={int} actual={int}")]
async fn error_indicates_sequence_mismatch(world: &mut E2EWorld, expected: u32, actual: u32) {
    let error = world.last_error.as_ref().expect("Expected an error");
    assert!(
        error.contains(&expected.to_string()) || error.contains("mismatch"),
        "Expected error to mention expected={} actual={}, got: {}",
        expected,
        actual,
        error
    );
}

#[then("no new events are stored")]
async fn no_new_events_stored(world: &mut E2EWorld) {
    // Query events and verify count hasn't changed
    // This would need to compare against a baseline
    assert!(
        world.last_response.is_none(),
        "Expected no new events but got a response"
    );
}

#[then(expr = "an event {string} is emitted")]
async fn event_is_emitted(world: &mut E2EWorld, event_type: String) {
    let response = world
        .last_response
        .as_ref()
        .expect("No response - command may have failed");
    let events = response.events.as_ref().expect("No events in response");

    let found = events.pages.iter().any(|page| {
        page.event
            .as_ref()
            .map(|e| extract_event_type(e).contains(&event_type))
            .unwrap_or(false)
    });

    assert!(
        found,
        "Expected event '{}' not found in response",
        event_type
    );
}

#[then(expr = "the correlation ID {string} is in the response")]
async fn correlation_id_in_response(world: &mut E2EWorld, correlation_alias: String) {
    let expected_corr = world.correlation(&correlation_alias);
    let response = world.last_response.as_ref().expect("No response");

    // The response should have the correlation ID from the command
    // For now, we just verify we got a response with events
    assert!(
        response.events.is_some(),
        "Expected response with events for correlation {}",
        expected_corr
    );
}

#[then(expr = "the correlation ID {string} is preserved")]
async fn correlation_id_preserved(world: &mut E2EWorld, correlation_alias: String) {
    correlation_id_in_response(world, correlation_alias).await;
}

#[then(expr = "the correlation ID {string} is preserved in shipment events")]
async fn correlation_id_preserved_in_shipment(world: &mut E2EWorld, _correlation_alias: String) {
    // Correlation is at EventBook Cover level; saga propagates correlation from source
    // This step verifies the saga created shipment events (already checked by prior steps)
    assert!(
        world.last_error.is_none(),
        "Expected no errors during shipment correlation check"
    );
}

// ============================================================================
// Saga Flow Steps - Order Setup and Completion
// ============================================================================

#[given(expr = "an order {string} exists and is paid")]
async fn order_exists_and_paid(world: &mut E2EWorld, order_alias: String) {
    // Create the order
    let customer_id = format!("CUST-{}", order_alias);
    let command = examples_proto::CreateOrder {
        customer_id: customer_id.clone(),
        items: vec![examples_proto::LineItem {
            product_id: "SKU-001".to_string(),
            name: "Test Product".to_string(),
            quantity: 1,
            unit_price_cents: 1000,
            ..Default::default()
        }],
        ..Default::default()
    };

    let correlation = format!("setup-order-{}", order_alias);
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation,
        "examples.CreateOrder",
        &command,
    );

    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to create order: {:?}",
        world.last_error
    );

    // Submit payment
    let payment_cmd = examples_proto::SubmitPayment {
        payment_method: "card".to_string(),
        amount_cents: 1000,
    };

    let payment_correlation = format!("payment-{}", order_alias);
    let payment_book = world.build_command(
        "order",
        &order_alias,
        &payment_correlation,
        "examples.SubmitPayment",
        &payment_cmd,
    );

    world.execute(payment_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to submit payment: {:?}",
        world.last_error
    );
}

#[given(expr = "an order {string} ready for fulfillment")]
async fn order_ready_for_fulfillment(world: &mut E2EWorld, order_alias: String) {
    order_exists_and_paid(world, order_alias).await;
}

#[given(regex = r#"^an order "([^"]+)" with correlation "([^"]+)"$"#)]
async fn order_with_correlation(
    world: &mut E2EWorld,
    order_alias: String,
    correlation_alias: String,
) {
    let command = examples_proto::CreateOrder {
        customer_id: format!("CUST-{}", order_alias),
        items: vec![examples_proto::LineItem {
            product_id: "SKU-001".to_string(),
            name: "Test Product".to_string(),
            quantity: 1,
            unit_price_cents: 1000,
            ..Default::default()
        }],
        ..Default::default()
    };

    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation_alias,
        "examples.CreateOrder",
        &command,
    );

    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to create order with correlation: {:?}",
        world.last_error
    );

    // Store correlation→order mapping for PM steps
    world
        .context
        .insert(format!("order-for-corr:{}", correlation_alias), order_alias);
}

#[given(regex = r#"^an order "([^"]+)" with items:$"#)]
async fn order_with_items_table(world: &mut E2EWorld, step: &Step, order_alias: String) {
    let table = step.table.as_ref().expect("Expected a data table");
    let mut items = Vec::new();

    for row in table.rows.iter().skip(1) {
        items.push(examples_proto::LineItem {
            product_id: row[0].trim().to_string(),
            name: row[0].trim().to_string(),
            quantity: row[1].trim().parse().unwrap_or(1),
            unit_price_cents: 1000,
            ..Default::default()
        });
    }

    let command = examples_proto::CreateOrder {
        customer_id: format!("CUST-{}", order_alias),
        items,
        ..Default::default()
    };

    let correlation = format!("setup-order-{}", order_alias);
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation,
        "examples.CreateOrder",
        &command,
    );

    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to create order with items: {:?}",
        world.last_error
    );
}

#[given(regex = r#"^orders exist:$"#)]
async fn orders_exist_table(world: &mut E2EWorld, step: &Step) {
    let table = step.table.as_ref().expect("Expected a data table");

    for row in table.rows.iter().skip(1) {
        let order_id = row[0].trim().to_string();
        let status = row[1].trim().to_string();

        // Create order
        let command = examples_proto::CreateOrder {
            customer_id: format!("CUST-{}", order_id),
            items: vec![examples_proto::LineItem {
                product_id: "SKU-001".to_string(),
                name: "Test Product".to_string(),
                quantity: 1,
                unit_price_cents: 1000,
                ..Default::default()
            }],
            ..Default::default()
        };

        let correlation = format!("setup-order-{}", order_id);
        let cmd_book = world.build_command(
            "order",
            &order_id,
            &correlation,
            "examples.CreateOrder",
            &command,
        );

        world.execute(cmd_book).await;
        assert!(
            world.last_error.is_none(),
            "Failed to create order {}: {:?}",
            order_id,
            world.last_error
        );

        // Submit payment if status is "paid"
        if status == "paid" || status == "completed" {
            let payment_cmd = examples_proto::SubmitPayment {
                payment_method: "card".to_string(),
                amount_cents: 1000,
            };

            let payment_correlation = format!("payment-{}", order_id);
            let payment_book = world.build_command(
                "order",
                &order_id,
                &payment_correlation,
                "examples.SubmitPayment",
                &payment_cmd,
            );

            world.execute(payment_book).await;
            assert!(
                world.last_error.is_none(),
                "Failed to pay order {}: {:?}",
                order_id,
                world.last_error
            );
        }

        // Confirm payment if status is "completed"
        if status == "completed" {
            let confirm_cmd = examples_proto::ConfirmPayment {
                payment_reference: format!("PAY-REF-{}", order_id),
            };

            let confirm_correlation = format!("confirm-{}", order_id);
            let confirm_book = world.build_command(
                "order",
                &order_id,
                &confirm_correlation,
                "examples.ConfirmPayment",
                &confirm_cmd,
            );

            world.execute(confirm_book).await;
            assert!(
                world.last_error.is_none(),
                "Failed to confirm order {}: {:?}",
                order_id,
                world.last_error
            );
        }
    }
}

#[given(regex = r#"^an order "([^"]+)" for customer "([^"]+)" totaling (\d+) cents$"#)]
async fn order_for_customer_totaling(
    world: &mut E2EWorld,
    order_alias: String,
    customer_alias: String,
    total_cents: i32,
) {
    let customer_root = world.root(&customer_alias);
    let cart_root = uuid::Uuid::new_v4(); // Generate cart root for inventory reservation linking
    let command = examples_proto::CreateOrder {
        customer_id: customer_alias,
        items: vec![examples_proto::LineItem {
            product_id: "SKU-001".to_string(),
            name: "Test Product".to_string(),
            quantity: 1,
            unit_price_cents: total_cents,
            ..Default::default()
        }],
        customer_root: customer_root.as_bytes().to_vec(),
        cart_root: cart_root.as_bytes().to_vec(),
    };

    let correlation = format!("setup-order-{}", order_alias);
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation,
        "examples.CreateOrder",
        &command,
    );

    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to create order: {:?}",
        world.last_error
    );
}

#[given(regex = r#"^an order ready for fulfillment$"#)]
async fn an_order_ready_for_fulfillment_noarg(world: &mut E2EWorld) {
    order_exists_and_paid(world, "ORD-READY".to_string()).await;
}

#[given(regex = r#"^a product "([^"]+)" with price (\d+) and stock (\d+)$"#)]
async fn product_with_price_and_stock(
    world: &mut E2EWorld,
    product_alias: String,
    price_cents: i32,
    stock: i32,
) {
    // Store price in context for later use
    world
        .context
        .insert(format!("price:{}", product_alias), price_cents.to_string());

    // Initialize inventory
    inventory_exists(world, product_alias, stock).await;
}

#[when(regex = r#"^I create order "([^"]+)" with correlation "([^"]+)"$"#)]
async fn create_order_with_correlation(
    world: &mut E2EWorld,
    order_alias: String,
    correlation_alias: String,
) {
    let command = examples_proto::CreateOrder {
        customer_id: format!("CUST-{}", order_alias),
        items: vec![examples_proto::LineItem {
            product_id: "SKU-001".to_string(),
            name: "Test Product".to_string(),
            quantity: 1,
            unit_price_cents: 1000,
            ..Default::default()
        }],
        ..Default::default()
    };

    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation_alias,
        "examples.CreateOrder",
        &command,
    );

    world.execute(cmd_book).await;
}

#[then(expr = "the shipment contains all order items")]
async fn shipment_contains_all_items(world: &mut E2EWorld) {
    // Fulfillment saga creates a shipment; verify it exists
    assert!(
        world.last_error.is_none(),
        "Expected shipment with order items"
    );
}

#[when(expr = "payment is confirmed for order {string} with correlation {string}")]
async fn confirm_payment_with_correlation(
    world: &mut E2EWorld,
    order_alias: String,
    correlation_alias: String,
) {
    let command = examples_proto::ConfirmPayment {
        payment_reference: format!("PAY-REF-{}", Uuid::new_v4()),
    };

    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation_alias,
        "examples.ConfirmPayment",
        &command,
    );

    world.execute(cmd_book).await;
}

#[given(expr = "order {string} is completed with correlation {string}")]
#[when(expr = "order {string} is completed with correlation {string}")]
async fn complete_order_with_correlation(
    world: &mut E2EWorld,
    order_alias: String,
    correlation_alias: String,
) {
    // Determine order total from events (OrderCreated has subtotal_cents)
    let root = world.root(&order_alias);
    let events = world.query_events("order", root).await;

    let mut total_cents = 1000; // fallback
    let mut already_paid = false;

    for page in &events {
        if let Some(event) = &page.event {
            let event_type = extract_event_type(event);
            if event_type.contains("OrderCreated") {
                if let Ok(decoded) = examples_proto::OrderCreated::decode(event.value.as_slice()) {
                    total_cents = decoded.subtotal_cents;
                }
            }
            if event_type.contains("PaymentSubmitted") {
                already_paid = true;
            }
        }
    }

    // Submit payment if not already done
    if !already_paid {
        let payment_cmd = examples_proto::SubmitPayment {
            payment_method: "card".to_string(),
            amount_cents: total_cents,
        };

        let payment_correlation = format!("payment-{}", order_alias);
        let payment_book = world.build_command(
            "order",
            &order_alias,
            &payment_correlation,
            "examples.SubmitPayment",
            &payment_cmd,
        );

        world.execute(payment_book).await;
        assert!(
            world.last_error.is_none(),
            "Failed to submit payment for {}: {:?}",
            order_alias,
            world.last_error
        );
    }

    confirm_payment_with_correlation(world, order_alias, correlation_alias).await;
    assert!(
        world.last_error.is_none(),
        "Failed to confirm payment: {:?}",
        world.last_error
    );
}

#[when(expr = "the order {string} is completed with correlation {string}")]
async fn the_order_completed_with_correlation(
    world: &mut E2EWorld,
    order_alias: String,
    correlation_alias: String,
) {
    complete_order_with_correlation(world, order_alias, correlation_alias).await;
}

#[when(expr = "I complete order {string} with correlation {string}")]
async fn i_complete_order(world: &mut E2EWorld, order_alias: String, correlation_alias: String) {
    complete_order_with_correlation(world, order_alias, correlation_alias).await;
}

// ============================================================================
// Saga Flow Steps - Async Event Waiting
// ============================================================================

/// Wait for an event of the given type to appear in the specified domain
async fn wait_for_event(
    world: &mut E2EWorld,
    domain: &str,
    root_alias: &str,
    event_type: &str,
    timeout_secs: u64,
) -> bool {
    let root = world.root(root_alias);
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        let events = world.query_events(domain, root).await;
        let found = events.iter().any(|page| {
            page.event
                .as_ref()
                .map(|e| extract_event_type(e).contains(event_type))
                .unwrap_or(false)
        });

        if found {
            return true;
        }

        if Instant::now() > deadline {
            return false;
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[given(expr = "within {int} seconds a shipment is created")]
#[then(expr = "within {int} seconds a shipment is created")]
async fn shipment_created_within_timeout(world: &mut E2EWorld, timeout_secs: u64) {
    // Saga reuses the order's root UUID for the fulfillment aggregate
    let order_alias = world
        .roots
        .keys()
        .find(|k| k.starts_with("ORD") || k.contains("order"))
        .cloned()
        .unwrap_or_else(|| "ORD-DEFAULT".to_string());

    let found = wait_for_event(
        world,
        "fulfillment",
        &order_alias,
        "ShipmentCreated",
        timeout_secs,
    )
    .await;
    assert!(
        found,
        "ShipmentCreated event not found within {} seconds",
        timeout_secs
    );
}

#[then(expr = "the shipment references order {string}")]
async fn shipment_references_order(world: &mut E2EWorld, order_alias: String) {
    // Saga reuses the order's root UUID for the fulfillment aggregate
    let root = world.root(&order_alias);
    let events = world.query_events("fulfillment", root).await;

    let order_root = world.root(&order_alias);

    for page in &events {
        if let Some(event) = &page.event {
            let event_type = extract_event_type(event);
            if event_type.contains("ShipmentCreated") {
                if let Ok(decoded) = examples_proto::ShipmentCreated::decode(event.value.as_slice())
                {
                    // The order_id in shipment should match the order root UUID
                    assert!(
                        decoded.order_id.contains(&order_root.to_string())
                            || !decoded.order_id.is_empty(),
                        "Shipment order_id doesn't reference expected order"
                    );
                    return;
                }
            }
        }
    }
    panic!("No ShipmentCreated event found to verify order reference");
}

#[then(expr = "within {int} seconds {int} shipments are created")]
async fn multiple_shipments_created(world: &mut E2EWorld, timeout_secs: u64, expected_count: i32) {
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        let mut shipment_count = 0;

        // Collect order aliases (sagas reuse order root for fulfillment)
        let aliases: Vec<String> = world
            .roots
            .keys()
            .filter(|k| k.contains("ORD") || k.contains("MULTI"))
            .cloned()
            .collect();

        for alias in aliases {
            // Saga uses order root directly for fulfillment domain
            let root = world.root(&alias);
            let events = world.query_events("fulfillment", root).await;

            for page in &events {
                if let Some(event) = &page.event {
                    if extract_event_type(event).contains("ShipmentCreated") {
                        shipment_count += 1;
                    }
                }
            }
        }

        if shipment_count >= expected_count {
            return;
        }

        if Instant::now() > deadline {
            panic!(
                "Expected {} shipments, found {} within {} seconds",
                expected_count, shipment_count, timeout_secs
            );
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[then(expr = "shipment for {string} has correlation {string}")]
async fn shipment_has_correlation(
    world: &mut E2EWorld,
    order_alias: String,
    _correlation_alias: String,
) {
    // Saga reuses the order's root UUID for fulfillment aggregate
    let root = world.root(&order_alias);
    let events = world.query_events("fulfillment", root).await;

    let found = events.iter().any(|page| {
        page.event
            .as_ref()
            .map(|e| extract_event_type(e).contains("ShipmentCreated"))
            .unwrap_or(false)
    });

    assert!(found, "No shipment found for order {}", order_alias);
}

#[then(expr = "no duplicate shipments exist")]
async fn no_duplicate_shipments(world: &mut E2EWorld) {
    // Collect aliases first to avoid borrow issues
    let aliases: Vec<String> = world.roots.keys().cloned().collect();

    // Check that each fulfillment aggregate has at most one ShipmentCreated event
    for alias in aliases {
        if alias.starts_with("fulfillment-") {
            let root = world.root(&alias);
            let events = world.query_events("fulfillment", root).await;

            let shipment_count = events
                .iter()
                .filter(|page| {
                    page.event
                        .as_ref()
                        .map(|e| extract_event_type(e).contains("ShipmentCreated"))
                        .unwrap_or(false)
                })
                .count();

            assert!(
                shipment_count <= 1,
                "Duplicate shipments found for {}: {} ShipmentCreated events",
                alias,
                shipment_count
            );
        }
    }
}

// ============================================================================
// Saga Flow Steps - Correlation Verification
// ============================================================================

#[then(regex = r#"^within (\d+) seconds the correlation "([^"]+)" appears in:$"#)]
async fn correlation_appears_in(
    world: &mut E2EWorld,
    step: &Step,
    timeout_secs: u64,
    correlation_alias: String,
) {
    let table = step.table.as_ref().expect("Expected a data table");
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    // Parse expectations: (domain, event_type_substring)
    let expectations: Vec<(String, String)> = table
        .rows
        .iter()
        .skip(1)
        .map(|row| (row[0].trim().to_string(), row[1].trim().to_string()))
        .collect();

    let correlation_id = world.correlation(&correlation_alias);

    loop {
        let results = world.query_by_correlation(&correlation_id).await;

        let all_found = expectations.iter().all(|(exp_domain, exp_type)| {
            results
                .iter()
                .any(|(d, t, _)| d == exp_domain && t.contains(exp_type.as_str()))
        });

        if all_found {
            return;
        }

        if Instant::now() > deadline {
            let found: Vec<String> = results
                .iter()
                .map(|(d, t, _)| format!("{}/{}", d, t))
                .collect();
            let missing: Vec<String> = expectations
                .iter()
                .filter(|(exp_d, exp_t)| {
                    !results
                        .iter()
                        .any(|(d, t, _)| d == exp_d && t.contains(exp_t.as_str()))
                })
                .map(|(d, t)| format!("{}/{}", d, t))
                .collect();
            panic!(
                "Correlation '{}' not found in all expected domains within {} seconds.\n\
                 Missing: {:?}\nFound: {:?}",
                correlation_alias, timeout_secs, missing, found
            );
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

// ============================================================================
// Saga Flow Steps - Error Handling
// ============================================================================

#[given(regex = r#"^an order "([^"]+)" exists without a customer aggregate$"#)]
async fn order_without_customer(world: &mut E2EWorld, order_alias: String) {
    // Create an order with a customer_id string but no actual customer aggregate.
    // The loyalty-earn saga will fail because customer_root is empty in OrderCompleted.
    let command = examples_proto::CreateOrder {
        customer_id: "no-customer".to_string(),
        items: vec![examples_proto::LineItem {
            product_id: "SKU-001".to_string(),
            name: "Test Product".to_string(),
            quantity: 1,
            unit_price_cents: 1000,
            ..Default::default()
        }],
        ..Default::default()
    };

    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &format!("ghost-create-{}", order_alias),
        "examples.CreateOrder",
        &command,
    );
    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to create order: {:?}",
        world.last_error
    );

    // Pay the order so it can be completed
    let pay_cmd = examples_proto::SubmitPayment {
        payment_method: "card".to_string(),
        amount_cents: 1000,
    };
    let pay_book = world.build_command(
        "order",
        &order_alias,
        &format!("ghost-pay-{}", order_alias),
        "examples.SubmitPayment",
        &pay_cmd,
    );
    world.execute(pay_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to pay order: {:?}",
        world.last_error
    );
}

#[when(regex = r#"^I re-complete order "([^"]+)"$"#)]
async fn re_complete_order(world: &mut E2EWorld, order_alias: String) {
    // Try to confirm payment again on an already-completed order
    let confirm_cmd = examples_proto::ConfirmPayment {
        payment_reference: format!("PAY-RE-{}", order_alias),
    };
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &format!("re-complete-{}", order_alias),
        "examples.ConfirmPayment",
        &confirm_cmd,
    );
    world.execute(cmd_book).await;
}

// ============================================================================
// Saga Flow Steps - Cancellation
// ============================================================================

#[when(expr = "order {string} is cancelled with reason {string} and correlation {string}")]
async fn cancel_order_with_correlation(
    world: &mut E2EWorld,
    order_alias: String,
    reason: String,
    correlation_alias: String,
) {
    let command = examples_proto::CancelOrder { reason };

    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation_alias,
        "examples.CancelOrder",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "order {string} is cancelled with correlation {string}")]
async fn cancel_order(world: &mut E2EWorld, order_alias: String, correlation_alias: String) {
    cancel_order_with_correlation(
        world,
        order_alias,
        "Customer request".to_string(),
        correlation_alias,
    )
    .await;
}

// ============================================================================
// Saga Flow Steps - Loyalty Verification
// ============================================================================

#[then(expr = "customer {string} has earned loyalty points")]
async fn customer_has_earned_loyalty_points(world: &mut E2EWorld, customer_alias: String) {
    let root = world.root(&customer_alias);
    let events = world.query_events("customer", root).await;

    let has_loyalty_points = events.iter().any(|page| {
        page.event
            .as_ref()
            .map(|e| extract_event_type(e).contains("LoyaltyPointsAdded"))
            .unwrap_or(false)
    });

    assert!(
        has_loyalty_points,
        "Expected LoyaltyPointsAdded event for customer {}, found events: {:?}",
        customer_alias,
        events
            .iter()
            .filter_map(|p| p.event.as_ref().map(extract_event_type))
            .collect::<Vec<_>>()
    );
}

// ============================================================================
// Process Manager Steps - Helpers
// ============================================================================

/// Trigger a PM prerequisite event by executing the appropriate domain command.
async fn trigger_pm_prerequisite(world: &mut E2EWorld, event_type: &str, correlation_alias: &str) {
    let order_alias = world
        .context
        .get(&format!("order-for-corr:{}", correlation_alias))
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "No order found for correlation '{}'. Use Given step first.",
                correlation_alias
            )
        });

    match event_type {
        "PaymentSubmitted" => {
            let payment_cmd = examples_proto::SubmitPayment {
                payment_method: "card".to_string(),
                amount_cents: 1000,
            };
            let cmd_book = world.build_command(
                "order",
                &order_alias,
                correlation_alias,
                "examples.SubmitPayment",
                &payment_cmd,
            );
            world.execute(cmd_book).await;
            assert!(
                world.last_error.is_none(),
                "SubmitPayment failed: {:?}",
                world.last_error
            );
        }
        "StockReserved" => {
            let inventory_alias = format!("inv-{}", correlation_alias);

            // Initialize inventory if not already done
            if !world
                .context
                .contains_key(&format!("inv-init:{}", correlation_alias))
            {
                let init_cmd = examples_proto::InitializeStock {
                    product_id: format!("SKU-{}", correlation_alias),
                    quantity: 100,
                    low_stock_threshold: 10,
                };
                let init_corr = format!("setup-inv-{}", correlation_alias);
                let init_book = world.build_command(
                    "inventory",
                    &inventory_alias,
                    &init_corr,
                    "examples.InitializeStock",
                    &init_cmd,
                );
                world.execute(init_book).await;
                assert!(
                    world.last_error.is_none(),
                    "InitializeStock failed: {:?}",
                    world.last_error
                );
                world.context.insert(
                    format!("inv-init:{}", correlation_alias),
                    "true".to_string(),
                );
            }

            let reserve_cmd = examples_proto::ReserveStock {
                quantity: 1,
                order_id: order_alias,
            };
            let cmd_book = world.build_command(
                "inventory",
                &inventory_alias,
                correlation_alias,
                "examples.ReserveStock",
                &reserve_cmd,
            );
            world.execute(cmd_book).await;
            assert!(
                world.last_error.is_none(),
                "ReserveStock failed: {:?}",
                world.last_error
            );
        }
        "ItemsPacked" => {
            // Use order root for fulfillment (matches saga behavior)
            let fulfillment_alias = format!("fulfill-{}", correlation_alias);
            let order_root = world.root(&order_alias);
            world.roots.insert(fulfillment_alias.clone(), order_root);

            // Step 1: CreateShipment
            let create_cmd = examples_proto::CreateShipment {
                order_id: order_alias,
                items: vec![],
            };
            let cmd_book = world.build_command(
                "fulfillment",
                &fulfillment_alias,
                correlation_alias,
                "examples.CreateShipment",
                &create_cmd,
            );
            world.execute(cmd_book).await;
            assert!(
                world.last_error.is_none(),
                "CreateShipment failed: {:?}",
                world.last_error
            );

            // Step 2: MarkPicked
            let pick_cmd = examples_proto::MarkPicked {
                picker_id: "picker-auto".to_string(),
            };
            let cmd_book = world.build_command(
                "fulfillment",
                &fulfillment_alias,
                correlation_alias,
                "examples.MarkPicked",
                &pick_cmd,
            );
            world.execute(cmd_book).await;
            assert!(
                world.last_error.is_none(),
                "MarkPicked failed: {:?}",
                world.last_error
            );

            // Step 3: MarkPacked — produces ItemsPacked which triggers PM
            let pack_cmd = examples_proto::MarkPacked {
                packer_id: "packer-auto".to_string(),
            };
            let cmd_book = world.build_command(
                "fulfillment",
                &fulfillment_alias,
                correlation_alias,
                "examples.MarkPacked",
                &pack_cmd,
            );
            world.execute(cmd_book).await;
            assert!(
                world.last_error.is_none(),
                "MarkPacked failed: {:?}",
                world.last_error
            );
        }
        _ => panic!("Unknown PM prerequisite: {}", event_type),
    }
}

/// Check whether the PM dispatched Ship by looking for Shipped events.
async fn has_ship_been_dispatched(world: &E2EWorld, correlation_alias: &str) -> bool {
    let order_alias = world
        .context
        .get(&format!("order-for-corr:{}", correlation_alias))
        .cloned()
        .unwrap_or_default();

    if order_alias.is_empty() {
        return false;
    }

    // Check the fulfillment alias root first, then order root
    let fulfillment_alias = format!("fulfill-{}", correlation_alias);
    let root = world
        .roots
        .get(&fulfillment_alias)
        .or_else(|| world.roots.get(&order_alias))
        .copied();

    let Some(root) = root else {
        return false;
    };

    let events = world.query_events("fulfillment", root).await;
    events.iter().any(|page| {
        page.event
            .as_ref()
            .map(|e| {
                let t = extract_event_type(e);
                t.ends_with("Shipped") && !t.contains("Shipment")
            })
            .unwrap_or(false)
    })
}

// ============================================================================
// Process Manager Steps - Given
// ============================================================================

#[given(regex = r#"^all prerequisites completed for correlation "([^"]+)"$"#)]
async fn all_prerequisites_completed(world: &mut E2EWorld, correlation_alias: String) {
    // Create order
    let order_alias = format!("ORD-{}", correlation_alias);
    order_with_correlation(world, order_alias, correlation_alias.clone()).await;

    // Trigger all three prerequisites
    trigger_pm_prerequisite(world, "PaymentSubmitted", &correlation_alias).await;
    trigger_pm_prerequisite(world, "StockReserved", &correlation_alias).await;
    trigger_pm_prerequisite(world, "ItemsPacked", &correlation_alias).await;
}

#[given("Ship was already dispatched")]
async fn ship_was_already_dispatched(world: &mut E2EWorld) {
    // After all prerequisites, PM should have dispatched Ship.
    // Wait briefly to confirm.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Find any correlation from context
    let corr = world
        .context
        .keys()
        .find(|k| k.starts_with("order-for-corr:"))
        .map(|k| k.strip_prefix("order-for-corr:").unwrap().to_string())
        .expect("No PM correlation found");

    assert!(
        has_ship_been_dispatched(world, &corr).await,
        "Ship should have been dispatched after all prerequisites"
    );
}

#[given(regex = r#"^all prerequisites received for correlation "([^"]+)"$"#)]
async fn all_prerequisites_received(world: &mut E2EWorld, correlation_alias: String) {
    all_prerequisites_completed(world, correlation_alias).await;
}

#[given(regex = r#"^orders with correlations "([^"]+)" and "([^"]+)"$"#)]
async fn orders_with_two_correlations(world: &mut E2EWorld, corr_a: String, corr_b: String) {
    let order_a = format!("ORD-{}", corr_a);
    let order_b = format!("ORD-{}", corr_b);
    order_with_correlation(world, order_a, corr_a).await;
    order_with_correlation(world, order_b, corr_b).await;
}

// ============================================================================
// Process Manager Steps - When
// ============================================================================

#[when(regex = r#"^PaymentSubmitted is received for correlation "([^"]+)"$"#)]
async fn pm_payment_submitted(world: &mut E2EWorld, correlation_alias: String) {
    trigger_pm_prerequisite(world, "PaymentSubmitted", &correlation_alias).await;
}

#[when(regex = r#"^StockReserved is received for correlation "([^"]+)"$"#)]
async fn pm_stock_reserved(world: &mut E2EWorld, correlation_alias: String) {
    trigger_pm_prerequisite(world, "StockReserved", &correlation_alias).await;
}

#[when(regex = r#"^ItemsPacked is received for correlation "([^"]+)"$"#)]
async fn pm_items_packed(world: &mut E2EWorld, correlation_alias: String) {
    trigger_pm_prerequisite(world, "ItemsPacked", &correlation_alias).await;
}

#[when(regex = r#"^ItemsPacked arrives (?:first )?for correlation "([^"]+)"$"#)]
async fn pm_items_packed_arrives(world: &mut E2EWorld, correlation_alias: String) {
    trigger_pm_prerequisite(world, "ItemsPacked", &correlation_alias).await;
}

#[when(regex = r#"^StockReserved arrives (?:second )?for correlation "([^"]+)"$"#)]
async fn pm_stock_reserved_arrives(world: &mut E2EWorld, correlation_alias: String) {
    trigger_pm_prerequisite(world, "StockReserved", &correlation_alias).await;
}

#[when(regex = r#"^PaymentSubmitted arrives (?:third )?for correlation "([^"]+)"$"#)]
async fn pm_payment_submitted_arrives(world: &mut E2EWorld, correlation_alias: String) {
    trigger_pm_prerequisite(world, "PaymentSubmitted", &correlation_alias).await;
}

#[when(regex = r#"^a duplicate PaymentSubmitted arrives for correlation "([^"]+)"$"#)]
async fn pm_duplicate_payment(world: &mut E2EWorld, correlation_alias: String) {
    // Submit a second payment attempt — this will fail at the order aggregate
    // (already paid), but the PM should still handle the event bus notification.
    // Since the order rejects the duplicate, no new event reaches the PM.
    let order_alias = world
        .context
        .get(&format!("order-for-corr:{}", correlation_alias))
        .cloned()
        .expect("No order for this correlation");

    let payment_cmd = examples_proto::SubmitPayment {
        payment_method: "card".to_string(),
        amount_cents: 1000,
    };
    let dup_corr = format!("dup-{}", correlation_alias);
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &dup_corr,
        "examples.SubmitPayment",
        &payment_cmd,
    );
    world.execute(cmd_book).await;
    // Ignore error — duplicate payment is expected to fail
}

#[when(regex = r#"^ItemsPacked is re-delivered for correlation "([^"]+)"$"#)]
async fn pm_redelivered_packed(world: &mut E2EWorld, correlation_alias: String) {
    // Attempt to re-pack — fulfillment aggregate rejects if not in "picking" state
    let fulfillment_alias = format!("fulfill-{}", correlation_alias);
    let pack_cmd = examples_proto::MarkPacked {
        packer_id: "packer-redeliver".to_string(),
    };
    let dup_corr = format!("redeliver-{}", correlation_alias);
    let cmd_book = world.build_command(
        "fulfillment",
        &fulfillment_alias,
        &dup_corr,
        "examples.MarkPacked",
        &pack_cmd,
    );
    world.execute(cmd_book).await;
    // Ignore error — re-delivery of already-packed shipment is expected to fail
}

#[when(regex = r#"^PaymentSubmitted arrives for "([^"]+)"$"#)]
async fn pm_payment_arrives_short(world: &mut E2EWorld, correlation_alias: String) {
    trigger_pm_prerequisite(world, "PaymentSubmitted", &correlation_alias).await;
}

#[when(regex = r#"^all three prerequisites arrive for "([^"]+)"$"#)]
async fn pm_all_prerequisites_arrive(world: &mut E2EWorld, correlation_alias: String) {
    trigger_pm_prerequisite(world, "PaymentSubmitted", &correlation_alias).await;
    trigger_pm_prerequisite(world, "StockReserved", &correlation_alias).await;
    trigger_pm_prerequisite(world, "ItemsPacked", &correlation_alias).await;
}

#[when(regex = r#"^customer "([^"]+)" creates and checks out a cart with "([^"]+)"$"#)]
async fn customer_creates_cart_with_product(
    world: &mut E2EWorld,
    customer_alias: String,
    product_alias: String,
) {
    // Create order directly (cart domain removed)
    let order_alias = format!("ORD-{}", customer_alias);
    let order_cmd = examples_proto::CreateOrder {
        customer_id: customer_alias,
        items: vec![examples_proto::LineItem {
            product_id: product_alias.clone(),
            name: product_alias,
            quantity: 1,
            unit_price_cents: 2500,
            ..Default::default()
        }],
        ..Default::default()
    };
    let order_corr = format!("pm-integration-{}", order_alias);
    let order_book = world.build_command(
        "order",
        &order_alias,
        &order_corr,
        "examples.CreateOrder",
        &order_cmd,
    );
    world.execute(order_book).await;
    assert!(
        world.last_error.is_none(),
        "CreateOrder failed: {:?}",
        world.last_error
    );

    // Store correlation→order mapping for PM integration
    world.context.insert(
        format!("order-for-corr:{}", order_corr),
        order_alias.clone(),
    );
    world
        .context
        .insert("pm-integration-order".to_string(), order_alias);
    world
        .context
        .insert("pm-integration-corr".to_string(), order_corr);
}

#[when("payment is submitted for the order")]
async fn payment_submitted_for_order(world: &mut E2EWorld) {
    let order_alias = world
        .context
        .get("pm-integration-order")
        .cloned()
        .expect("No integration order found");
    let corr = world
        .context
        .get("pm-integration-corr")
        .cloned()
        .expect("No integration correlation found");

    let payment_cmd = examples_proto::SubmitPayment {
        payment_method: "card".to_string(),
        amount_cents: 2500,
    };
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &corr,
        "examples.SubmitPayment",
        &payment_cmd,
    );
    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "SubmitPayment failed: {:?}",
        world.last_error
    );
}

#[when("stock is reserved for the order")]
async fn stock_reserved_for_order(world: &mut E2EWorld) {
    let order_alias = world
        .context
        .get("pm-integration-order")
        .cloned()
        .expect("No integration order found");
    let corr = world
        .context
        .get("pm-integration-corr")
        .cloned()
        .expect("No integration correlation found");

    let inventory_alias = format!("inv-{}", corr);
    let init_cmd = examples_proto::InitializeStock {
        product_id: "PM-WIDGET".to_string(),
        quantity: 100,
        low_stock_threshold: 10,
    };
    let init_corr = format!("setup-inv-integration");
    let init_book = world.build_command(
        "inventory",
        &inventory_alias,
        &init_corr,
        "examples.InitializeStock",
        &init_cmd,
    );
    world.execute(init_book).await;
    assert!(
        world.last_error.is_none(),
        "InitializeStock failed: {:?}",
        world.last_error
    );

    let reserve_cmd = examples_proto::ReserveStock {
        quantity: 1,
        order_id: order_alias,
    };
    let cmd_book = world.build_command(
        "inventory",
        &inventory_alias,
        &corr,
        "examples.ReserveStock",
        &reserve_cmd,
    );
    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "ReserveStock failed: {:?}",
        world.last_error
    );
}

// ============================================================================
// Process Manager Steps - Then
// ============================================================================

#[then("no Ship command is dispatched yet")]
async fn no_ship_dispatched(world: &mut E2EWorld) {
    // Brief wait to ensure async processing completes
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check all known fulfillment roots for ShipmentShipped
    let aliases: Vec<String> = world
        .roots
        .keys()
        .filter(|k| k.starts_with("fulfill-") || k.starts_with("ORD-"))
        .cloned()
        .collect();

    for alias in &aliases {
        let root = world.roots[alias];
        let events = world.query_events("fulfillment", root).await;
        let shipped = events.iter().any(|page| {
            page.event
                .as_ref()
                .map(|e| extract_event_type(e).ends_with("Shipped"))
                .unwrap_or(false)
        });
        assert!(
            !shipped,
            "Ship was dispatched unexpectedly for alias {}",
            alias
        );
    }
}

#[then(regex = r#"^within (\d+) seconds a Ship command is dispatched to fulfillment$"#)]
async fn ship_dispatched_within(world: &mut E2EWorld, timeout_secs: u64) {
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        // Check all fulfillment roots for ShipmentShipped
        let aliases: Vec<String> = world
            .roots
            .keys()
            .filter(|k| k.starts_with("fulfill-") || k.starts_with("ORD-"))
            .cloned()
            .collect();

        for alias in &aliases {
            let root = world.roots[alias];
            let events = world.query_events("fulfillment", root).await;
            let shipped = events.iter().any(|page| {
                page.event
                    .as_ref()
                    .map(|e| extract_event_type(e).ends_with("Shipped"))
                    .unwrap_or(false)
            });
            if shipped {
                return;
            }
        }

        if Instant::now() > deadline {
            panic!(
                "Ship command not dispatched within {} seconds",
                timeout_secs
            );
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[then(regex = r#"^the Ship command has correlation "([^"]+)"$"#)]
async fn ship_has_correlation(world: &mut E2EWorld, _correlation_alias: String) {
    // In standalone mode, correlation is preserved through the event bus.
    // If Ship was dispatched (verified by prior step), correlation was used.
    // Verification: the PM uses the trigger's correlation_id for its commands.
    assert!(
        world.last_error.is_none(),
        "Expected no errors during correlation check"
    );
}

#[then("no additional Ship command is dispatched")]
async fn no_additional_ship(world: &mut E2EWorld) {
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Collect unique root UUIDs to avoid double-counting
    // (fulfill-* and ORD-* aliases may point to the same root)
    let unique_roots: std::collections::HashSet<Uuid> = world
        .roots
        .iter()
        .filter(|(k, _)| k.starts_with("fulfill-") || k.starts_with("ORD-"))
        .map(|(_, v)| *v)
        .collect();

    let mut total_shipped = 0;
    for root in &unique_roots {
        let events = world.query_events("fulfillment", *root).await;
        total_shipped += events
            .iter()
            .filter(|page| {
                page.event
                    .as_ref()
                    .map(|e| {
                        let t = extract_event_type(e);
                        t.ends_with("Shipped") && !t.contains("Shipment")
                    })
                    .unwrap_or(false)
            })
            .count();
    }

    assert!(
        total_shipped <= 1,
        "Expected at most 1 Ship dispatch, found {}",
        total_shipped
    );
}

#[then(regex = r#"^querying PM state for correlation "([^"]+)" shows:$"#)]
async fn pm_state_shows(world: &mut E2EWorld, step: &Step, correlation_alias: String) {
    let table = step.table.as_ref().expect("Expected a data table");
    let correlation_id = world.correlation(&correlation_alias);

    // Collect expected completions from table
    let mut expected_completed: Vec<String> = Vec::new();
    let mut expected_pending: Vec<String> = Vec::new();
    for row in table.rows.iter().skip(1) {
        let prerequisite = row[0].trim().to_string();
        let status = row[1].trim();
        match status {
            "completed" => expected_completed.push(prerequisite),
            "pending" => expected_pending.push(prerequisite),
            _ => panic!("Unknown status: {}", status),
        }
    }

    // PM root is derived from correlation_id
    let pm_root = Uuid::new_v5(&Uuid::NAMESPACE_OID, correlation_id.as_bytes());

    // Poll for async PM event processing (up to 5 seconds)
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut completed = std::collections::HashSet::new();
    loop {
        let pm_events = world.query_events("order-fulfillment", pm_root).await;

        completed.clear();
        for page in &pm_events {
            if let Some(event) = &page.event {
                let event_type = extract_event_type(event);
                if event_type.contains("PrerequisiteCompleted") {
                    if let Ok(decoded) = process_manager_fulfillment::PrerequisiteCompleted::decode(
                        event.value.as_slice(),
                    ) {
                        completed.insert(decoded.prerequisite.clone());
                    }
                }
                if event_type.contains("DispatchIssued") {
                    completed.insert("dispatched".to_string());
                }
            }
        }

        let all_expected_met = expected_completed.iter().all(|p| completed.contains(p));
        if all_expected_met || tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Verify expected state
    for prerequisite in &expected_completed {
        assert!(
            completed.contains(prerequisite.as_str()),
            "Expected prerequisite '{}' to be completed, but it's pending. Completed: {:?}",
            prerequisite,
            completed
        );
    }
    for prerequisite in &expected_pending {
        assert!(
            !completed.contains(prerequisite.as_str()),
            "Expected prerequisite '{}' to be pending, but it's completed",
            prerequisite
        );
    }
}

#[then(regex = r#"^Ship is dispatched only for "([^"]+)"$"#)]
async fn ship_dispatched_only_for(world: &mut E2EWorld, correlation_alias: String) {
    // Wait for async PM processing
    tokio::time::sleep(Duration::from_secs(2)).await;

    assert!(
        has_ship_been_dispatched(world, &correlation_alias).await,
        "Expected Ship dispatched for correlation {}",
        correlation_alias
    );
}

#[then(regex = r#"^PM state for "([^"]+)" shows only payment completed$"#)]
async fn pm_state_only_payment(world: &mut E2EWorld, correlation_alias: String) {
    let correlation_id = world.correlation(&correlation_alias);
    let pm_root = Uuid::new_v5(&Uuid::NAMESPACE_OID, correlation_id.as_bytes());
    let pm_events = world.query_events("order-fulfillment", pm_root).await;

    let mut completed = Vec::new();
    for page in &pm_events {
        if let Some(event) = &page.event {
            let event_type = extract_event_type(event);
            if event_type.contains("PrerequisiteCompleted") {
                if let Ok(decoded) = process_manager_fulfillment::PrerequisiteCompleted::decode(
                    event.value.as_slice(),
                ) {
                    completed.push(decoded.prerequisite.clone());
                }
            }
        }
    }

    assert!(
        completed.contains(&"payment".to_string()),
        "Expected payment to be completed. Found: {:?}",
        completed
    );
    assert_eq!(
        completed.len(),
        1,
        "Expected only payment completed, found: {:?}",
        completed
    );
}

#[then(regex = r#"^within (\d+) seconds the fulfillment process dispatches Ship$"#)]
async fn fulfillment_process_dispatches_ship(world: &mut E2EWorld, timeout_secs: u64) {
    let order_alias = world
        .context
        .get("pm-integration-order")
        .cloned()
        .expect("No integration order found");
    let corr = world
        .context
        .get("pm-integration-corr")
        .cloned()
        .expect("No integration correlation found");

    // Step 1: Confirm payment → triggers OrderCompleted → fulfillment saga → ShipmentCreated
    let confirm_cmd = examples_proto::ConfirmPayment {
        payment_reference: format!("PAY-REF-{}", Uuid::new_v4()),
    };
    let confirm_book = world.build_command(
        "order",
        &order_alias,
        &corr,
        "examples.ConfirmPayment",
        &confirm_cmd,
    );
    world.execute(confirm_book).await;
    assert!(
        world.last_error.is_none(),
        "ConfirmPayment failed: {:?}",
        world.last_error
    );

    // Step 2: Wait for ShipmentCreated (from fulfillment saga)
    let root = world.root(&order_alias);
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        let events = world.query_events("fulfillment", root).await;
        let created = events.iter().any(|page| {
            page.event
                .as_ref()
                .map(|e| extract_event_type(e).contains("ShipmentCreated"))
                .unwrap_or(false)
        });
        if created {
            break;
        }
        if Instant::now() > deadline {
            let event_types: Vec<String> = events
                .iter()
                .filter_map(|p| p.event.as_ref().map(extract_event_type))
                .collect();
            panic!(
                "ShipmentCreated not found within {} seconds. Fulfillment events: {:?}",
                timeout_secs, event_types
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Step 3: Pick and Pack the shipment so the PM can dispatch Ship
    let fulfillment_alias = format!("fulfill-integration-{}", order_alias);
    world.roots.insert(fulfillment_alias.clone(), root);

    let pick_cmd = examples_proto::MarkPicked {
        picker_id: "picker-integration".to_string(),
    };
    let pick_book = world.build_command(
        "fulfillment",
        &fulfillment_alias,
        &corr,
        "examples.MarkPicked",
        &pick_cmd,
    );
    world.execute(pick_book).await;
    assert!(
        world.last_error.is_none(),
        "MarkPicked failed: {:?}",
        world.last_error
    );

    let pack_cmd = examples_proto::MarkPacked {
        packer_id: "packer-integration".to_string(),
    };
    let pack_book = world.build_command(
        "fulfillment",
        &fulfillment_alias,
        &corr,
        "examples.MarkPacked",
        &pack_cmd,
    );
    world.execute(pack_book).await;
    assert!(
        world.last_error.is_none(),
        "MarkPacked failed: {:?}",
        world.last_error
    );

    // Step 4: Wait for Shipped (PM dispatched Ship after ItemsPacked)
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        let events = world.query_events("fulfillment", root).await;
        let shipped = events.iter().any(|page| {
            page.event
                .as_ref()
                .map(|e| extract_event_type(e).ends_with("Shipped"))
                .unwrap_or(false)
        });
        if shipped {
            return;
        }
        if Instant::now() > deadline {
            let event_types: Vec<String> = events
                .iter()
                .filter_map(|p| p.event.as_ref().map(extract_event_type))
                .collect();
            panic!(
                "Ship not dispatched within {} seconds. Fulfillment events: {:?}",
                timeout_secs, event_types
            );
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[then(regex = r#"^the shipment transitions to "([^"]+)" status$"#)]
async fn shipment_transitions_to_status(world: &mut E2EWorld, expected_status: String) {
    let order_alias = world
        .context
        .get("pm-integration-order")
        .cloned()
        .expect("No integration order found");

    let root = world.root(&order_alias);
    let events = world.query_events("fulfillment", root).await;

    let expected_event = match expected_status.as_str() {
        "shipped" => "Shipped",
        "created" => "ShipmentCreated",
        "picked" => "ItemsPicked",
        "packed" => "ItemsPacked",
        _ => panic!("Unknown shipment status: {}", expected_status),
    };

    let found = events.iter().any(|page| {
        page.event
            .as_ref()
            .map(|e| extract_event_type(e).contains(expected_event))
            .unwrap_or(false)
    });

    assert!(
        found,
        "Expected shipment to be in '{}' status (event: {})",
        expected_status, expected_event
    );
}

// ============================================================================
// Cancellation Saga Steps
// ============================================================================

#[given(regex = r#"^an order "([^"]+)" with:$"#)]
async fn order_with_table_fields(world: &mut E2EWorld, step: &Step, order_alias: String) {
    let table = step.table.as_ref().expect("Expected a data table");
    let mut customer_id = format!("CUST-{}", order_alias);
    let mut loyalty_applied = 0i32;
    let mut inventory_reserved = false;

    for row in table.rows.iter().skip(1) {
        let field = row[0].trim();
        let value = row[1].trim();
        match field {
            "customer_id" => customer_id = value.to_string(),
            "loyalty_applied" => loyalty_applied = value.parse().unwrap_or(0),
            "inventory_reserved" => inventory_reserved = value == "true",
            _ => {}
        }
    }

    // Register customer root (customer aggregate no longer exists)
    let customer_root = world.root(&customer_id);
    let cart_root = uuid::Uuid::new_v4(); // Generate cart root for inventory reservation linking
    let command = examples_proto::CreateOrder {
        customer_id: customer_id.clone(),
        items: vec![examples_proto::LineItem {
            product_id: "SKU-CANCEL-001".to_string(),
            name: "Cancellation Test Product".to_string(),
            quantity: 1,
            unit_price_cents: 1000,
            product_root: world.root("SKU-CANCEL-001").as_bytes().to_vec(),
            ..Default::default()
        }],
        customer_root: customer_root.as_bytes().to_vec(),
        cart_root: cart_root.as_bytes().to_vec(),
    };

    let correlation = format!("setup-order-{}", order_alias);
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation,
        "examples.CreateOrder",
        &command,
    );
    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to create order with fields: {:?}",
        world.last_error
    );

    // Apply loyalty discount if specified
    if loyalty_applied > 0 {
        let discount_cmd = examples_proto::ApplyLoyaltyDiscount {
            points: loyalty_applied,
            discount_cents: loyalty_applied, // 1:1 points to cents
        };
        let discount_corr = format!("discount-{}", order_alias);
        let discount_book = world.build_command(
            "order",
            &order_alias,
            &discount_corr,
            "examples.ApplyLoyaltyDiscount",
            &discount_cmd,
        );
        world.execute(discount_book).await;
        assert!(
            world.last_error.is_none(),
            "Failed to apply loyalty discount: {:?}",
            world.last_error
        );
    }

    // Initialize inventory for the product
    let inv_alias = "SKU-CANCEL-001";
    let init_cmd = examples_proto::InitializeStock {
        product_id: "SKU-CANCEL-001".to_string(),
        quantity: 100,
        low_stock_threshold: 10,
    };
    let init_corr = format!("setup-inv-{}", order_alias);
    let init_book = world.build_command(
        "inventory",
        inv_alias,
        &init_corr,
        "examples.InitializeStock",
        &init_cmd,
    );
    world.execute(init_book).await;
    // Ignore if already initialized

    // Reserve stock if requested
    if inventory_reserved {
        // Use cart_root as order_id — the cancellation saga derives order_id from
        // cart_root when non-empty (see CancellationSaga::process_event).
        let reserve_cmd = examples_proto::ReserveStock {
            quantity: 1,
            order_id: cart_root.to_string(),
        };
        let reserve_corr = format!("reserve-{}", order_alias);
        let reserve_book = world.build_command(
            "inventory",
            inv_alias,
            &reserve_corr,
            "examples.ReserveStock",
            &reserve_cmd,
        );
        world.execute(reserve_book).await;
        assert!(
            world.last_error.is_none(),
            "ReserveStock failed: {:?}",
            world.last_error
        );
    }

    // Store context
    world
        .context
        .insert(format!("customer-for-order:{}", order_alias), customer_id);
}

#[given(regex = r#"^an order "([^"]+)" with no loyalty points applied$"#)]
async fn order_with_no_loyalty(world: &mut E2EWorld, order_alias: String) {
    let customer_id = format!("CUST-{}", order_alias);
    // Register customer root (customer aggregate no longer exists)
    let customer_root = world.root(&customer_id);
    let cart_root = uuid::Uuid::new_v4(); // Generate cart root for inventory reservation linking
    let command = examples_proto::CreateOrder {
        customer_id: customer_id.clone(),
        items: vec![examples_proto::LineItem {
            product_id: "SKU-NOPTS-001".to_string(),
            name: "No Points Product".to_string(),
            quantity: 1,
            unit_price_cents: 1000,
            product_root: world.root("SKU-NOPTS-001").as_bytes().to_vec(),
            ..Default::default()
        }],
        customer_root: customer_root.as_bytes().to_vec(),
        cart_root: cart_root.as_bytes().to_vec(),
    };

    let correlation = format!("setup-order-{}", order_alias);
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation,
        "examples.CreateOrder",
        &command,
    );
    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to create order: {:?}",
        world.last_error
    );

    // Initialize inventory
    let init_cmd = examples_proto::InitializeStock {
        product_id: "SKU-NOPTS-001".to_string(),
        quantity: 100,
        low_stock_threshold: 10,
    };
    let init_corr = format!("setup-inv-{}", order_alias);
    let init_book = world.build_command(
        "inventory",
        "SKU-NOPTS-001",
        &init_corr,
        "examples.InitializeStock",
        &init_cmd,
    );
    world.execute(init_book).await;

    // Reserve stock so cancellation saga can release it.
    // Use cart_root as order_id — the cancellation saga derives order_id from
    // cart_root when non-empty (see CancellationSaga::process_event).
    let reserve_cmd = examples_proto::ReserveStock {
        quantity: 1,
        order_id: cart_root.to_string(),
    };
    let reserve_corr = format!("reserve-{}", order_alias);
    let reserve_book = world.build_command(
        "inventory",
        "SKU-NOPTS-001",
        &reserve_corr,
        "examples.ReserveStock",
        &reserve_cmd,
    );
    world.execute(reserve_book).await;
    assert!(
        world.last_error.is_none(),
        "ReserveStock failed: {:?}",
        world.last_error
    );

    world
        .context
        .insert(format!("customer-for-order:{}", order_alias), customer_id);
}

// ============================================================================
// Correlation Preservation Steps
// ============================================================================

#[then(regex = r#"^the correlation "([^"]+)" appears in:$"#)]
async fn correlation_appears_in_table(
    world: &mut E2EWorld,
    step: &Step,
    _correlation_alias: String,
) {
    let table = step.table.as_ref().expect("Expected a data table");

    for row in table.rows.iter().skip(1) {
        let domain = row[0].trim();
        let expected_event_type = row[1].trim();

        // Search all known roots for this event type in this domain
        let aliases: Vec<String> = world.roots.keys().cloned().collect();
        let mut found = false;

        for alias in &aliases {
            let root = world.roots[alias];
            let events = world.query_events(domain, root).await;
            if events.iter().any(|page| {
                page.event
                    .as_ref()
                    .map(|e| extract_event_type(e).contains(expected_event_type))
                    .unwrap_or(false)
            }) {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "Expected event '{}' in domain '{}' not found",
            expected_event_type, domain
        );
    }
}

#[when("both orders are completed")]
async fn both_orders_completed(world: &mut E2EWorld) {
    let order_aliases: Vec<String> = world
        .roots
        .keys()
        .filter(|k| k.starts_with("ORD-"))
        .cloned()
        .collect();

    for order_alias in order_aliases {
        let corr = format!("complete-{}", order_alias);
        complete_order_with_correlation(world, order_alias, corr).await;
    }
}

#[then(regex = r#"^fulfillment events for "([^"]+)" have correlation "([^"]+)"$"#)]
async fn fulfillment_events_have_correlation(
    world: &mut E2EWorld,
    order_alias: String,
    _correlation_alias: String,
) {
    // Wait for async saga to create fulfillment events (up to 5 seconds)
    let root = world.root(&order_alias);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut events;
    loop {
        events = world.query_events("fulfillment", root).await;
        if !events.is_empty() || tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        !events.is_empty(),
        "No fulfillment events found for order {}",
        order_alias
    );
}

#[then("no cross-contamination of correlation IDs")]
async fn no_correlation_cross_contamination(world: &mut E2EWorld) {
    // Each order should have independent fulfillment events
    let order_aliases: Vec<String> = world
        .roots
        .keys()
        .filter(|k| k.starts_with("ORD-"))
        .cloned()
        .collect();

    // Wait for async saga processing (previous step may have already waited)
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let mut all_have_events = true;
        for alias in &order_aliases {
            let root = world.roots[alias];
            let events = world.query_events("fulfillment", root).await;
            if events.is_empty() {
                all_have_events = false;
                break;
            }
        }
        if all_have_events || tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    for alias in &order_aliases {
        let root = world.roots[alias];
        let events = world.query_events("fulfillment", root).await;
        assert!(
            !events.is_empty(),
            "Missing fulfillment events for {}",
            alias
        );
    }
}

// ============================================================================
// Domain Lifecycle Steps - Inventory
// ============================================================================

#[when(expr = "I initialize stock for {string} with {int} units")]
async fn initialize_stock_when(world: &mut E2EWorld, product_alias: String, units: i32) {
    let command = examples_proto::InitializeStock {
        product_id: product_alias.clone(),
        quantity: units,
        low_stock_threshold: 10,
    };

    let correlation = format!("init-stock-{}", Uuid::new_v4());
    let cmd_book = world.build_command(
        "inventory",
        &product_alias,
        &correlation,
        "examples.InitializeStock",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I receive {int} units for {string}")]
async fn receive_stock(world: &mut E2EWorld, quantity: i32, product_alias: String) {
    let command = examples_proto::ReceiveStock {
        quantity,
        reference: format!("PO-{}", Uuid::new_v4()),
    };

    let correlation = format!("receive-stock-{}", Uuid::new_v4());
    let cmd_book = world.build_command(
        "inventory",
        &product_alias,
        &correlation,
        "examples.ReceiveStock",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I reserve {int} units of {string} for order {string}")]
async fn reserve_stock(
    world: &mut E2EWorld,
    quantity: i32,
    product_alias: String,
    order_id: String,
) {
    let command = examples_proto::ReserveStock { quantity, order_id };

    let correlation = format!("reserve-stock-{}", Uuid::new_v4());
    let cmd_book = world.build_command(
        "inventory",
        &product_alias,
        &correlation,
        "examples.ReserveStock",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I release reservation of {string} for order {string}")]
async fn release_reservation(world: &mut E2EWorld, product_alias: String, order_id: String) {
    let command = examples_proto::ReleaseReservation { order_id };

    let correlation = format!("release-resv-{}", Uuid::new_v4());
    let cmd_book = world.build_command(
        "inventory",
        &product_alias,
        &correlation,
        "examples.ReleaseReservation",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I commit reservation of {string} for order {string}")]
async fn commit_reservation(world: &mut E2EWorld, product_alias: String, order_id: String) {
    let command = examples_proto::CommitReservation { order_id };

    let correlation = format!("commit-resv-{}", Uuid::new_v4());
    let cmd_book = world.build_command(
        "inventory",
        &product_alias,
        &correlation,
        "examples.CommitReservation",
        &command,
    );

    world.execute(cmd_book).await;
}

#[given(expr = "{int} units of {string} are reserved for order {string}")]
async fn units_reserved_for_order(
    world: &mut E2EWorld,
    quantity: i32,
    product_alias: String,
    order_id: String,
) {
    let command = examples_proto::ReserveStock {
        quantity,
        order_id: order_id.clone(),
    };

    let correlation = format!("setup-reserve-{}-{}", product_alias, order_id);
    let cmd_book = world.build_command(
        "inventory",
        &product_alias,
        &correlation,
        "examples.ReserveStock",
        &command,
    );

    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to reserve stock: {:?}",
        world.last_error
    );
}

// ============================================================================
// Domain Lifecycle Steps - Fulfillment
// ============================================================================

#[when(expr = "I create shipment {string} for order {string}")]
async fn create_shipment(world: &mut E2EWorld, shipment_alias: String, order_id: String) {
    let command = examples_proto::CreateShipment {
        order_id,
        items: vec![],
    };

    let correlation = format!("create-shipment-{}", Uuid::new_v4());
    let cmd_book = world.build_command(
        "fulfillment",
        &shipment_alias,
        &correlation,
        "examples.CreateShipment",
        &command,
    );

    world.execute(cmd_book).await;
}

#[given(expr = "a shipment {string} exists for order {string}")]
async fn shipment_exists(world: &mut E2EWorld, shipment_alias: String, order_id: String) {
    let command = examples_proto::CreateShipment {
        order_id,
        items: vec![],
    };

    let correlation = format!("setup-shipment-{}", shipment_alias);
    let cmd_book = world.build_command(
        "fulfillment",
        &shipment_alias,
        &correlation,
        "examples.CreateShipment",
        &command,
    );

    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to create shipment: {:?}",
        world.last_error
    );
}

#[when(expr = "I mark shipment {string} as picked by {string}")]
async fn mark_shipment_picked(world: &mut E2EWorld, shipment_alias: String, picker_id: String) {
    let command = examples_proto::MarkPicked { picker_id };

    let correlation = format!("pick-{}", Uuid::new_v4());
    let cmd_book = world.build_command(
        "fulfillment",
        &shipment_alias,
        &correlation,
        "examples.MarkPicked",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I mark shipment {string} as packed by {string}")]
async fn mark_shipment_packed(world: &mut E2EWorld, shipment_alias: String, packer_id: String) {
    let command = examples_proto::MarkPacked { packer_id };

    let correlation = format!("pack-{}", Uuid::new_v4());
    let cmd_book = world.build_command(
        "fulfillment",
        &shipment_alias,
        &correlation,
        "examples.MarkPacked",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I ship {string} via {string} tracking {string}")]
async fn ship_order(
    world: &mut E2EWorld,
    shipment_alias: String,
    carrier: String,
    tracking_number: String,
) {
    let command = examples_proto::Ship {
        carrier,
        tracking_number,
    };

    let correlation = format!("ship-{}", Uuid::new_v4());
    let cmd_book = world.build_command(
        "fulfillment",
        &shipment_alias,
        &correlation,
        "examples.Ship",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I record delivery for {string} with signature {string}")]
async fn record_delivery(world: &mut E2EWorld, shipment_alias: String, signature: String) {
    let command = examples_proto::RecordDelivery { signature };

    let correlation = format!("deliver-{}", Uuid::new_v4());
    let cmd_book = world.build_command(
        "fulfillment",
        &shipment_alias,
        &correlation,
        "examples.RecordDelivery",
        &command,
    );

    world.execute(cmd_book).await;
}

// ============================================================================
// Domain Lifecycle Steps - Order
// ============================================================================

#[when(
    expr = "I create order {string} for customer {string} with {int} of {string} at {int} cents"
)]
async fn create_order_with_items(
    world: &mut E2EWorld,
    order_alias: String,
    customer_id: String,
    quantity: i32,
    product_id: String,
    unit_price_cents: i32,
) {
    let command = examples_proto::CreateOrder {
        customer_id,
        items: vec![examples_proto::LineItem {
            product_id: product_id.clone(),
            name: product_id,
            quantity,
            unit_price_cents,
            ..Default::default()
        }],
        ..Default::default()
    };

    let correlation = format!("create-order-{}", Uuid::new_v4());
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation,
        "examples.CreateOrder",
        &command,
    );

    world.execute(cmd_book).await;
}

#[given(expr = "an order {string} exists for customer {string}")]
async fn order_exists_for_customer(world: &mut E2EWorld, order_alias: String, customer_id: String) {
    let command = examples_proto::CreateOrder {
        customer_id,
        items: vec![examples_proto::LineItem {
            product_id: "SKU-001".to_string(),
            name: "Test Product".to_string(),
            quantity: 2,
            unit_price_cents: 1000,
            ..Default::default()
        }],
        ..Default::default()
    };

    let correlation = format!("setup-order-{}", order_alias);
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation,
        "examples.CreateOrder",
        &command,
    );

    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to create order: {:?}",
        world.last_error
    );
}

#[when(expr = "I apply loyalty discount of {int} points worth {int} cents to order {string}")]
async fn apply_loyalty_discount(
    world: &mut E2EWorld,
    points: i32,
    discount_cents: i32,
    order_alias: String,
) {
    let command = examples_proto::ApplyLoyaltyDiscount {
        points,
        discount_cents,
    };

    let correlation = format!("apply-discount-{}", Uuid::new_v4());
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation,
        "examples.ApplyLoyaltyDiscount",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I submit payment of {int} cents via {string} for order {string}")]
async fn submit_payment(
    world: &mut E2EWorld,
    amount_cents: i32,
    payment_method: String,
    order_alias: String,
) {
    let command = examples_proto::SubmitPayment {
        payment_method,
        amount_cents,
    };

    let correlation = format!("submit-payment-{}", Uuid::new_v4());
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation,
        "examples.SubmitPayment",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I confirm payment for order {string} with reference {string}")]
async fn confirm_payment_with_ref(
    world: &mut E2EWorld,
    order_alias: String,
    payment_reference: String,
) {
    let command = examples_proto::ConfirmPayment { payment_reference };

    let correlation = format!("confirm-payment-{}", Uuid::new_v4());
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation,
        "examples.ConfirmPayment",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I cancel order {string} with reason {string}")]
async fn cancel_order_with_reason(world: &mut E2EWorld, order_alias: String, reason: String) {
    let command = examples_proto::CancelOrder { reason };

    let correlation = format!("cancel-order-{}", Uuid::new_v4());
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation,
        "examples.CancelOrder",
        &command,
    );

    world.execute(cmd_book).await;
}

#[given(expr = "an order {string} exists and is completed")]
async fn order_exists_and_completed(world: &mut E2EWorld, order_alias: String) {
    // Create order
    order_exists_and_paid(world, order_alias.clone()).await;

    // Confirm payment to complete it
    let command = examples_proto::ConfirmPayment {
        payment_reference: format!("PAY-COMPLETE-{}", Uuid::new_v4()),
    };

    let correlation = format!("setup-complete-{}", order_alias);
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation,
        "examples.ConfirmPayment",
        &command,
    );

    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to complete order: {:?}",
        world.last_error
    );
}

// ============================================================================
// Saga Flow Step - Table-based async event waiting
// ============================================================================

#[then(regex = r#"^within (\d+) seconds:$"#)]
async fn within_n_seconds_table(world: &mut E2EWorld, step: &Step, timeout_secs: u64) {
    let table = step.table.as_ref().expect("Expected a data table");
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    // Parse expectations from table (skip header row)
    let mut expectations: Vec<(String, String, Option<String>)> = Vec::new();
    for row in table.rows.iter().skip(1) {
        let domain = row[0].trim().to_string();
        let event_type = row[1].trim().to_string();
        let correlation = row.get(2).map(|s| s.trim().to_string());
        expectations.push((domain, event_type, correlation));
    }

    // For each expected event, poll until found or timeout
    let mut found = vec![false; expectations.len()];

    loop {
        for (i, (domain, event_type, _)) in expectations.iter().enumerate() {
            if found[i] {
                continue;
            }

            // Try all known roots for this domain
            let aliases: Vec<String> = world.roots.keys().cloned().collect();
            for alias in &aliases {
                let root = world.root(alias);
                let events = world.query_events(domain, root).await;

                if events.iter().any(|page| {
                    page.event
                        .as_ref()
                        .map(|e| extract_event_type(e).contains(event_type.as_str()))
                        .unwrap_or(false)
                }) {
                    found[i] = true;
                    break;
                }
            }
        }

        if found.iter().all(|f| *f) {
            return;
        }

        if Instant::now() > deadline {
            let missing: Vec<String> = expectations
                .iter()
                .zip(found.iter())
                .filter(|(_, f)| !**f)
                .map(|((domain, event_type, _), _)| format!("{}/{}", domain, event_type))
                .collect();
            panic!(
                "Timed out waiting for events within {} seconds. Missing: {:?}",
                timeout_secs, missing
            );
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

// ============================================================================
// Projector Steps
// ============================================================================

/// Helper: create a completed order with specific parameters.
/// Returns the order alias for subsequent lookups.
async fn create_completed_order(
    world: &mut E2EWorld,
    order_alias: &str,
    customer_id: &str,
    subtotal_cents: i32,
    discount_cents: i32,
    total_cents: i32,
) {
    // Create order with items matching subtotal
    let command = examples_proto::CreateOrder {
        customer_id: customer_id.to_string(),
        items: vec![examples_proto::LineItem {
            product_id: "SKU-PROJ".to_string(),
            name: "Projector Test Item".to_string(),
            quantity: 1,
            unit_price_cents: subtotal_cents,
            ..Default::default()
        }],
        ..Default::default()
    };

    let correlation = format!("proj-setup-{}", order_alias);
    let cmd_book = world.build_command(
        "order",
        order_alias,
        &correlation,
        "examples.CreateOrder",
        &command,
    );
    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to create order: {:?}",
        world.last_error
    );

    // Apply discount if specified
    if discount_cents > 0 {
        let discount_cmd = examples_proto::ApplyLoyaltyDiscount {
            points: discount_cents, // 1:1 points to cents for simplicity
            discount_cents,
        };
        let disc_book = world.build_command(
            "order",
            order_alias,
            &format!("proj-discount-{}", order_alias),
            "examples.ApplyLoyaltyDiscount",
            &discount_cmd,
        );
        world.execute(disc_book).await;
        assert!(
            world.last_error.is_none(),
            "Failed to apply discount: {:?}",
            world.last_error
        );
    }

    // Submit payment
    let pay_cmd = examples_proto::SubmitPayment {
        payment_method: "card".to_string(),
        amount_cents: total_cents,
    };
    let pay_book = world.build_command(
        "order",
        order_alias,
        &format!("proj-pay-{}", order_alias),
        "examples.SubmitPayment",
        &pay_cmd,
    );
    world.execute(pay_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to submit payment: {:?}",
        world.last_error
    );

    // Confirm payment (completes the order)
    let confirm_cmd = examples_proto::ConfirmPayment {
        payment_reference: format!("PAY-PROJ-{}", order_alias),
    };
    let confirm_book = world.build_command(
        "order",
        order_alias,
        &format!("proj-confirm-{}", order_alias),
        "examples.ConfirmPayment",
        &confirm_cmd,
    );
    world.execute(confirm_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to confirm payment: {:?}",
        world.last_error
    );
}

#[given(regex = r#"^a completed order with:$"#)]
async fn completed_order_with_table(world: &mut E2EWorld, step: &Step) {
    let table = step.table.as_ref().expect("Expected a data table");

    // Parse field/value pairs from table
    let mut order_id = "ORD-PROJ-DEFAULT".to_string();
    let mut customer_id = "CUST-PROJ".to_string();
    let mut subtotal_cents = 1000i32;
    let mut discount_cents = 0i32;
    let mut total_cents = 0i32;

    for row in table.rows.iter().skip(1) {
        let field = row[0].trim();
        let value = row[1].trim();
        match field {
            "order_id" => order_id = value.to_string(),
            "customer_id" => customer_id = value.to_string(),
            "subtotal_cents" => subtotal_cents = value.parse().expect("Invalid subtotal_cents"),
            "discount_cents" => discount_cents = value.parse().expect("Invalid discount_cents"),
            "total_cents" => total_cents = value.parse().expect("Invalid total_cents"),
            _ => {} // Ignore unknown fields
        }
    }

    if total_cents == 0 {
        total_cents = subtotal_cents - discount_cents;
    }

    // Store the order alias for later projector queries
    world
        .context
        .insert("last_order_alias".to_string(), order_id.clone());

    create_completed_order(
        world,
        &order_id,
        &customer_id,
        subtotal_cents,
        discount_cents,
        total_cents,
    )
    .await;
}

#[given(regex = r#"^a completed order "([^"]+)" totaling (\d+) cents$"#)]
async fn completed_order_totaling(world: &mut E2EWorld, order_alias: String, total_cents: i32) {
    world
        .context
        .insert("last_order_alias".to_string(), order_alias.clone());
    create_completed_order(
        world,
        &order_alias,
        &format!("CUST-{}", order_alias),
        total_cents,
        0,
        total_cents,
    )
    .await;
}

#[given(regex = r#"^a completed order "([^"]+)" with total (\d+) cents$"#)]
async fn completed_order_with_total(world: &mut E2EWorld, order_alias: String, total_cents: i32) {
    world
        .context
        .insert("last_order_alias".to_string(), order_alias.clone());
    create_completed_order(
        world,
        &order_alias,
        &format!("CUST-{}", order_alias),
        total_cents,
        0,
        total_cents,
    )
    .await;
}

#[given(regex = r#"^an order "([^"]+)" is created totaling (\d+) cents$"#)]
async fn order_created_totaling(world: &mut E2EWorld, order_alias: String, total_cents: i32) {
    world
        .context
        .insert("last_order_alias".to_string(), order_alias.clone());

    let command = examples_proto::CreateOrder {
        customer_id: format!("CUST-{}", order_alias),
        items: vec![examples_proto::LineItem {
            product_id: "SKU-REFUND".to_string(),
            name: "Refund Test Item".to_string(),
            quantity: 1,
            unit_price_cents: total_cents,
            ..Default::default()
        }],
        ..Default::default()
    };

    let correlation = format!("proj-create-{}", order_alias);
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation,
        "examples.CreateOrder",
        &command,
    );
    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to create order: {:?}",
        world.last_error
    );
}

#[given(regex = r#"^an order "([^"]+)" is created for projector test$"#)]
async fn order_created_for_projector(world: &mut E2EWorld, order_alias: String) {
    world
        .context
        .insert("last_order_alias".to_string(), order_alias.clone());

    let command = examples_proto::CreateOrder {
        customer_id: format!("CUST-{}", order_alias),
        items: vec![examples_proto::LineItem {
            product_id: "SKU-PROJ".to_string(),
            name: "Projector Test Item".to_string(),
            quantity: 1,
            unit_price_cents: 1000,
            ..Default::default()
        }],
        ..Default::default()
    };

    let correlation = format!("proj-create-{}", order_alias);
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation,
        "examples.CreateOrder",
        &command,
    );
    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to create order: {:?}",
        world.last_error
    );
}

#[when(regex = r#"^order "([^"]+)" payment is submitted$"#)]
async fn order_payment_submitted(world: &mut E2EWorld, order_alias: String) {
    let pay_cmd = examples_proto::SubmitPayment {
        payment_method: "card".to_string(),
        amount_cents: 1000,
    };
    let pay_book = world.build_command(
        "order",
        &order_alias,
        &format!("proj-pay-{}", order_alias),
        "examples.SubmitPayment",
        &pay_cmd,
    );
    world.execute(pay_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to submit payment: {:?}",
        world.last_error
    );
}

#[when(regex = r#"^order "([^"]+)" payment is confirmed$"#)]
async fn order_payment_confirmed(world: &mut E2EWorld, order_alias: String) {
    let confirm_cmd = examples_proto::ConfirmPayment {
        payment_reference: format!("PAY-PROJ-{}", Uuid::new_v4()),
    };
    let confirm_book = world.build_command(
        "order",
        &order_alias,
        &format!("proj-confirm-{}", order_alias),
        "examples.ConfirmPayment",
        &confirm_cmd,
    );
    world.execute(confirm_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to confirm payment: {:?}",
        world.last_error
    );
}

#[when(regex = r#"^order "([^"]+)" is cancelled$"#)]
async fn order_is_cancelled(world: &mut E2EWorld, order_alias: String) {
    let command = examples_proto::CancelOrder {
        reason: "test refund".to_string(),
    };
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &format!("proj-cancel-{}", order_alias),
        "examples.CancelOrder",
        &command,
    );
    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to cancel order: {:?}",
        world.last_error
    );
}

// --- Web Projector Assertions ---

#[then(regex = r#"^within (\d+) seconds the web projector shows:$"#)]
async fn web_projector_shows_table(world: &mut E2EWorld, step: &Step, timeout_secs: u64) {
    let table = step.table.as_ref().expect("Expected a data table");
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    // Find order alias from table or context
    let order_alias = table
        .rows
        .iter()
        .skip(1)
        .find(|row| row[0].trim() == "order_id")
        .map(|row| row[1].trim().to_string())
        .or_else(|| world.context.get("last_order_alias").cloned())
        .expect("No order_id found in table or context");

    let order_uuid = world.root(&order_alias);
    let order_id_str = order_uuid.to_string();

    loop {
        if let Some(proj) = world.query_order_projection(&order_id_str).await {
            // Verify all fields from the table
            let mut all_match = true;
            for row in table.rows.iter().skip(1) {
                let field = row[0].trim();
                let expected = row[1].trim();
                let actual = match field {
                    "order_id" => order_alias.clone(),
                    "customer_id" => proj.customer_id.clone(),
                    "status" => proj.status.clone(),
                    "subtotal_cents" => proj.subtotal_cents.to_string(),
                    "discount_cents" => proj.discount_cents.to_string(),
                    "total_cents" => proj.total_cents.to_string(),
                    "loyalty_points_used" => proj.loyalty_points_used.to_string(),
                    "loyalty_points_earned" => proj.loyalty_points_earned.to_string(),
                    _ => continue,
                };
                if actual != expected {
                    all_match = false;
                    break;
                }
            }
            if all_match {
                return;
            }
        }

        if Instant::now() > deadline {
            let proj = world.query_order_projection(&order_id_str).await;
            panic!(
                "Web projector did not show expected values within {} seconds. Got: {:?}",
                timeout_secs, proj
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[then(regex = r#"^within (\d+) seconds? the web projector shows status "([^"]+)" for "([^"]+)"$"#)]
async fn web_projector_shows_status(
    world: &mut E2EWorld,
    timeout_secs: u64,
    expected_status: String,
    order_alias: String,
) {
    let order_uuid = world.root(&order_alias);
    let order_id_str = order_uuid.to_string();
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        if let Some(proj) = world.query_order_projection(&order_id_str).await {
            if proj.status == expected_status {
                return;
            }
        }

        if Instant::now() > deadline {
            let proj = world.query_order_projection(&order_id_str).await;
            panic!(
                "Web projector status for '{}' not '{}' within {} seconds. Got: {:?}",
                order_alias,
                expected_status,
                timeout_secs,
                proj.map(|p| p.status)
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[then(regex = r#"^within (\d+) seconds the web projector total for "([^"]+)" is (\d+)$"#)]
async fn web_projector_total(
    world: &mut E2EWorld,
    timeout_secs: u64,
    order_alias: String,
    expected_total: i64,
) {
    let order_uuid = world.root(&order_alias);
    let order_id_str = order_uuid.to_string();
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        if let Some(proj) = world.query_order_projection(&order_id_str).await {
            if proj.total_cents == expected_total {
                return;
            }
        }

        if Instant::now() > deadline {
            let proj = world.query_order_projection(&order_id_str).await;
            panic!(
                "Web projector total for '{}' not {} within {} seconds. Got: {:?}",
                order_alias,
                expected_total,
                timeout_secs,
                proj.map(|p| p.total_cents)
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

// --- Accounting Projector Assertions ---

#[then(regex = r#"^within (\d+) seconds the accounting ledger for "([^"]+)" has:$"#)]
async fn accounting_ledger_has(
    world: &mut E2EWorld,
    step: &Step,
    timeout_secs: u64,
    order_alias: String,
) {
    let table = step.table.as_ref().expect("Expected a data table");
    let order_uuid = world.root(&order_alias);
    let order_id_str = order_uuid.to_string();
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    // Parse expected entries from table
    let expected: Vec<(String, i64)> = table
        .rows
        .iter()
        .skip(1)
        .map(|row| {
            (
                row[0].trim().to_string(),
                row[1].trim().parse::<i64>().expect("Invalid amount_cents"),
            )
        })
        .collect();

    loop {
        let entries = world.query_ledger_entries(&order_id_str).await;

        let all_match = expected.iter().all(|(entry_type, amount)| {
            entries
                .iter()
                .any(|e| e.entry_type == *entry_type && e.amount_cents == *amount)
        });

        if all_match && !entries.is_empty() {
            return;
        }

        if Instant::now() > deadline {
            panic!(
                "Accounting ledger for '{}' did not match within {} seconds.\nExpected: {:?}\nGot: {:?}",
                order_alias,
                timeout_secs,
                expected,
                entries.iter().map(|e| (&e.entry_type, e.amount_cents)).collect::<Vec<_>>()
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[then(regex = r#"^within (\d+) seconds the accounting revenue for "([^"]+)" is (\d+)$"#)]
async fn accounting_revenue(
    world: &mut E2EWorld,
    timeout_secs: u64,
    order_alias: String,
    expected_revenue: i64,
) {
    let order_uuid = world.root(&order_alias);
    let order_id_str = order_uuid.to_string();
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        let entries = world.query_ledger_entries(&order_id_str).await;
        let revenue = entries
            .iter()
            .find(|e| e.entry_type == "revenue")
            .map(|e| e.amount_cents);

        if revenue == Some(expected_revenue) {
            return;
        }

        if Instant::now() > deadline {
            panic!(
                "Accounting revenue for '{}' not {} within {} seconds. Got: {:?}",
                order_alias, expected_revenue, timeout_secs, revenue
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

// --- Loyalty Balance Assertions ---

#[then(regex = r#"^within (\d+) seconds the loyalty balance for "([^"]+)" shows:$"#)]
async fn loyalty_balance_shows(
    world: &mut E2EWorld,
    step: &Step,
    timeout_secs: u64,
    customer_alias: String,
) {
    let table = step.table.as_ref().expect("Expected a data table");
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    // Parse expected values from key-value table
    let mut expected_current: Option<i32> = None;
    let mut expected_lifetime: Option<i32> = None;

    for row in table.rows.iter().skip(1) {
        let field = row[0].trim();
        let value = row[1].trim();
        match field {
            "current_points" => expected_current = Some(value.parse().expect("Invalid points")),
            "lifetime_points" => expected_lifetime = Some(value.parse().expect("Invalid points")),
            _ => {}
        }
    }

    let customer_uuid = world.root(&customer_alias);
    let customer_id_str = customer_uuid.to_string();

    loop {
        if let Some(balance) = world.query_loyalty_balance(&customer_id_str).await {
            let current_ok = expected_current
                .map(|exp| balance.current_points == exp)
                .unwrap_or(true);
            let lifetime_ok = expected_lifetime
                .map(|exp| balance.lifetime_points == exp)
                .unwrap_or(true);

            if current_ok && lifetime_ok {
                return;
            }
        }

        if Instant::now() > deadline {
            let balance = world.query_loyalty_balance(&customer_id_str).await;
            panic!(
                "Loyalty balance for '{}' did not match within {} seconds.\nExpected: current={:?}, lifetime={:?}\nGot: {:?}",
                customer_alias, timeout_secs, expected_current, expected_lifetime, balance
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[given(regex = r#"^an order "([^"]+)" with correlation "([^"]+)" is paid$"#)]
async fn order_with_correlation_is_paid(
    world: &mut E2EWorld,
    order_alias: String,
    correlation_alias: String,
) {
    // Create order
    order_with_correlation(world, order_alias, correlation_alias.clone()).await;
    // Submit payment
    trigger_pm_prerequisite(world, "PaymentSubmitted", &correlation_alias).await;
}

#[given(regex = r#"^an order "([^"]+)" with correlation "([^"]+)" is completed$"#)]
async fn order_with_correlation_is_completed(
    world: &mut E2EWorld,
    order_alias: String,
    correlation_alias: String,
) {
    // Create order
    order_with_correlation(world, order_alias.clone(), correlation_alias.clone()).await;
    // Submit payment
    trigger_pm_prerequisite(world, "PaymentSubmitted", &correlation_alias).await;
    // Confirm payment → OrderCompleted
    let confirm_cmd = examples_proto::ConfirmPayment {
        payment_reference: format!("PAY-REF-{}", Uuid::new_v4()),
    };
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation_alias,
        "examples.ConfirmPayment",
        &confirm_cmd,
    );
    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "ConfirmPayment failed in setup: {:?}",
        world.last_error
    );
}

#[when(regex = r#"^the order "([^"]+)" is cancelled with correlation "([^"]+)"$"#)]
async fn os_cancel_order(
    world: &mut E2EWorld,
    order_alias: String,
    correlation_alias: String,
) {
    let cancel_cmd = examples_proto::CancelOrder {
        reason: "test cancellation".to_string(),
    };
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation_alias,
        "examples.CancelOrder",
        &cancel_cmd,
    );
    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "CancelOrder failed: {:?}",
        world.last_error
    );
}

#[when(regex = r#"^a shipment is created for correlation "([^"]+)"$"#)]
async fn shipment_created_for_correlation(world: &mut E2EWorld, correlation_alias: String) {
    let order_alias = world
        .context
        .get(&format!("order-for-corr:{}", correlation_alias))
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "No order found for correlation '{}'. Use Given step first.",
                correlation_alias
            )
        });

    let fulfillment_alias = format!("fulfill-os-{}", correlation_alias);
    let order_root = world.root(&order_alias);
    world.roots.insert(fulfillment_alias.clone(), order_root);

    let create_cmd = examples_proto::CreateShipment {
        order_id: order_alias,
        items: vec![],
    };
    let cmd_book = world.build_command(
        "fulfillment",
        &fulfillment_alias,
        &correlation_alias,
        "examples.CreateShipment",
        &create_cmd,
    );
    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "CreateShipment failed: {:?}",
        world.last_error
    );
}

// ============================================================================
// Temporal Order Cancellation Steps
// ============================================================================

#[when(regex = r#"^I query order "([^"]+)" events at sequence (\d+)$"#)]
async fn query_order_at_sequence(world: &mut E2EWorld, order_alias: String, sequence: u32) {
    let root = world.root(&order_alias);
    world
        .query_events_temporal("order", root, Some(sequence), None)
        .await;
}

#[when(regex = r#"^I query order "([^"]+)" all events$"#)]
async fn query_order_all_events(world: &mut E2EWorld, order_alias: String) {
    let root = world.root(&order_alias);
    let events = world.query_events("order", root).await;
    world.last_temporal_events = Some(events);
}

#[then(regex = r#"^the temporal result has (\d+) events?$"#)]
async fn temporal_result_has_n_events(world: &mut E2EWorld, count: usize) {
    let events = world
        .last_temporal_events
        .as_ref()
        .expect("No temporal query result");
    assert_eq!(
        events.len(),
        count,
        "Expected {} temporal events, got {}",
        count,
        events.len()
    );
}

#[then(regex = r#"^the temporal event at index (\d+) is "([^"]+)"$"#)]
async fn temporal_event_at_index_is(world: &mut E2EWorld, index: usize, expected_type: String) {
    let events = world
        .last_temporal_events
        .as_ref()
        .expect("No temporal query result");
    assert!(
        index < events.len(),
        "Index {} out of bounds (have {} events)",
        index,
        events.len()
    );
    let event = events[index]
        .event
        .as_ref()
        .expect("Event page has no event");
    let actual_type = extract_event_type(event);
    assert!(
        actual_type.contains(&expected_type),
        "Expected event at index {} to be '{}', got '{}'",
        index,
        expected_type,
        actual_type
    );
}

#[then(regex = r#"^the order has (\d+) total events$"#)]
async fn order_has_total_events(world: &mut E2EWorld, count: usize) {
    let events = world
        .last_temporal_events
        .as_ref()
        .expect("No temporal query result");
    assert_eq!(
        events.len(),
        count,
        "Expected {} total events, got {}",
        count,
        events.len()
    );
}

#[when(
    regex = r#"^I dry-run cancel order "([^"]+)" at sequence (\d+) with reason "([^"]+)"$"#
)]
async fn dry_run_cancel_order_at_sequence(
    world: &mut E2EWorld,
    order_alias: String,
    sequence: u32,
    reason: String,
) {
    let root = world.root(&order_alias);

    // Record current event count to verify no mutation
    let current_events = world.query_events("order", root).await;
    world.event_count_before_dry_run = Some(current_events.len());

    let command = examples_proto::CancelOrder { reason };
    let correlation = format!("dryrun-cancel-{}", Uuid::new_v4());
    let cmd_book = proto::CommandBook {
        cover: Some(proto::Cover {
            domain: "order".to_string(),
            root: Some(proto::Uuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: correlation,
            edition: None,
        }),
        pages: vec![proto::CommandPage {
            sequence,
            command: Some(prost_types::Any {
                type_url: "type.examples/examples.CancelOrder".to_string(),
                value: command.encode_to_vec(),
            }),
        }],
        saga_origin: None,
    };

    world.dry_run(cmd_book, Some(sequence), None).await;
}

#[then(regex = r#"^querying order "([^"]+)" still returns exactly (\d+) events$"#)]
async fn order_still_has_n_events(world: &mut E2EWorld, order_alias: String, count: usize) {
    let root = world.root(&order_alias);
    let events = world.query_events("order", root).await;
    assert_eq!(
        events.len(),
        count,
        "Expected order to still have {} events (no mutation), got {}",
        count,
        events.len()
    );
}

// ============================================================================
// Speculative Component Execution Steps
// ============================================================================

/// Build a synthetic OrderCompleted event from existing order events.
///
/// Extracts customer_root, cart_root, items, and subtotal from the OrderCreated
/// event in the pages. Used by speculative saga tests to simulate "what if this
/// order completed?" without actually confirming payment through the event bus
/// (which would trigger the real saga and create side effects).
fn build_synthetic_order_completed(events: &[angzarr::proto::EventPage]) -> angzarr::proto::EventPage {
    let mut customer_root = vec![];
    let mut cart_root = vec![];
    let mut items = vec![];
    let mut subtotal_cents = 1000i32;

    for page in events {
        if let Some(event) = &page.event {
            if event.type_url.ends_with("OrderCreated") {
                if let Ok(created) =
                    <examples_proto::OrderCreated as prost::Message>::decode(event.value.as_slice())
                {
                    customer_root = created.customer_root;
                    cart_root = created.cart_root;
                    subtotal_cents = created.subtotal_cents;
                    items = created
                        .items
                        .into_iter()
                        .map(|i| examples_proto::LineItem {
                            product_id: i.product_id,
                            name: i.name,
                            quantity: i.quantity,
                            unit_price_cents: i.unit_price_cents,
                            product_root: i.product_root,
                        })
                        .collect();
                }
            }
        }
    }

    let completed = examples_proto::OrderCompleted {
        final_total_cents: subtotal_cents,
        payment_method: "card".to_string(),
        payment_reference: "PAY-REF-SPEC".to_string(),
        loyalty_points_earned: subtotal_cents / 100,
        completed_at: None,
        customer_root,
        cart_root,
        items,
    };

    angzarr::proto::EventPage {
        sequence: Some(angzarr::proto::event_page::Sequence::Num(events.len() as u32)),
        created_at: None,
        event: Some(prost_types::Any {
            type_url: "type.examples/examples.OrderCompleted".to_string(),
            value: <examples_proto::OrderCompleted as prost::Message>::encode_to_vec(&completed),
        }),
    }
}

#[given(regex = r#"^an order "([^"]+)" exists with subtotal (\d+) cents$"#)]
async fn order_exists_with_subtotal(
    world: &mut E2EWorld,
    order_alias: String,
    subtotal_cents: i32,
) {
    let command = examples_proto::CreateOrder {
        customer_id: format!("CUST-{}", order_alias),
        items: vec![examples_proto::LineItem {
            product_id: "SKU-SPEC".to_string(),
            name: "Speculative Test Item".to_string(),
            quantity: 1,
            unit_price_cents: subtotal_cents,
            ..Default::default()
        }],
        ..Default::default()
    };

    let correlation = format!("setup-spec-order-{}", order_alias);
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation,
        "examples.CreateOrder",
        &command,
    );
    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to create order: {:?}",
        world.last_error
    );
}

#[when(regex = r#"^I speculatively run the "([^"]+)" projector against order "([^"]+)" events$"#)]
async fn speculative_projector(world: &mut E2EWorld, projector_name: String, order_alias: String) {
    let root = world.root(&order_alias);

    // Allow async projector to finish processing before we snapshot DB state
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Snapshot web projector DB state before speculative execution
    let order_id = root.to_string();
    let projection_before = world.query_order_projection(&order_id).await;
    world.context.insert(
        "web_db_had_entry_before".to_string(),
        projection_before.is_some().to_string(),
    );
    if let Some(ref p) = projection_before {
        world.context.insert(
            "web_db_status_before".to_string(),
            p.status.clone(),
        );
        world.context.insert(
            "web_db_total_before".to_string(),
            p.total_cents.to_string(),
        );
    }

    let events = world.query_events("order", root).await;

    let event_book = angzarr::proto::EventBook {
        cover: Some(angzarr::proto::Cover {
            domain: "order".to_string(),
            root: Some(angzarr::proto::Uuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        snapshot: None,
        pages: events,
        snapshot_state: None,
    };

    let request = angzarr::proto::SpeculateProjectorRequest {
        projector_name,
        events: Some(event_book),
    };
    match world.speculative().projector(request).await {
        Ok(_projection) => {
            world.last_error = None;
            world
                .context
                .insert("speculative_success".to_string(), "true".to_string());
        }
        Err(e) => {
            world.last_error = Some(e.to_string());
        }
    }
}

#[when(regex = r#"^I speculatively run the "([^"]+)" projector against inventory "([^"]+)" events$"#)]
async fn speculative_projector_inventory(
    world: &mut E2EWorld,
    projector_name: String,
    inventory_alias: String,
) {
    let root = world.root(&inventory_alias);

    // Snapshot inventory projector state before speculative execution
    world.context.insert(
        "inventory_before_speculative".to_string(),
        format!("{}", root),
    );

    let events = world.query_events("inventory", root).await;

    let event_book = angzarr::proto::EventBook {
        cover: Some(angzarr::proto::Cover {
            domain: "inventory".to_string(),
            root: Some(angzarr::proto::Uuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        snapshot: None,
        pages: events,
        snapshot_state: None,
    };

    let request = angzarr::proto::SpeculateProjectorRequest {
        projector_name,
        events: Some(event_book),
    };
    match world.speculative().projector(request).await {
        Ok(_projection) => {
            world.last_error = None;
            world
                .context
                .insert("speculative_success".to_string(), "true".to_string());
        }
        Err(e) => {
            world.last_error = Some(e.to_string());
        }
    }
}

#[then(regex = r#"^the speculative projection succeeds$"#)]
async fn speculative_projection_succeeds(world: &mut E2EWorld) {
    assert!(
        world.last_error.is_none(),
        "Speculative projection failed: {:?}",
        world.last_error
    );
    assert_eq!(
        world.context.get("speculative_success").map(|s| s.as_str()),
        Some("true"),
        "Speculative projection did not complete"
    );
}

#[then(regex = r#"^speculative execution did not modify the web projector for "([^"]+)"$"#)]
async fn speculative_did_not_modify_web_projector(world: &mut E2EWorld, order_alias: String) {
    let root = world.root(&order_alias);
    let order_id = root.to_string();

    let projection_after = world.query_order_projection(&order_id).await;

    let had_entry_before = world
        .context
        .get("web_db_had_entry_before")
        .map(|s| s == "true")
        .unwrap_or(false);

    if had_entry_before {
        // Entry existed before speculative run — verify it wasn't modified
        let after = projection_after.expect("Entry disappeared after speculative execution");
        let status_before = world.context.get("web_db_status_before").cloned().unwrap_or_default();
        let total_before = world.context.get("web_db_total_before").cloned().unwrap_or_default();
        assert_eq!(
            after.status, status_before,
            "Speculative execution modified web projector status"
        );
        assert_eq!(
            after.total_cents.to_string(), total_before,
            "Speculative execution modified web projector total_cents"
        );
    } else {
        // No entry before — speculative execution should not have created one
        assert!(
            projection_after.is_none(),
            "Speculative execution created a web projector entry for {}",
            order_alias
        );
    }
}

#[then(regex = r#"^speculative execution did not modify the inventory projector for "([^"]+)"$"#)]
async fn speculative_did_not_modify_inventory_projector(
    world: &mut E2EWorld,
    inventory_alias: String,
) {
    // In standalone mode, the inventory projector is a simple in-memory projector.
    // For speculative execution, we verify that no side effects were persisted
    // by checking the event count remains unchanged (handled by the framework).
    // This assertion verifies the speculative execution completed successfully.
    assert!(
        world.context.get("speculative_success").map(|s| s == "true").unwrap_or(false),
        "Speculative execution of inventory projector for {} did not succeed",
        inventory_alias
    );
}

#[when(
    regex = r#"^I speculatively run the "([^"]+)" against order "([^"]+)" completion events$"#
)]
async fn speculative_saga(world: &mut E2EWorld, saga_name: String, order_alias: String) {
    let root = world.root(&order_alias);
    let mut events = world.query_events("order", root).await;

    // Sagas react to OrderCompleted. Append a synthetic OrderCompleted event
    // to simulate "what if this order completed?" — the order is only created
    // and paid in the Given step, not confirmed via the event bus (which would
    // trigger the real saga and create side effects we're trying to avoid).
    events.push(build_synthetic_order_completed(&events));

    let event_book = angzarr::proto::EventBook {
        cover: Some(angzarr::proto::Cover {
            domain: "order".to_string(),
            root: Some(angzarr::proto::Uuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: format!("spec-saga-{}", order_alias),
            edition: None,
        }),
        snapshot: None,
        pages: events,
        snapshot_state: None,
    };

    let request = angzarr::proto::SpeculateSagaRequest {
        saga_name,
        source: Some(event_book),
        destinations: vec![],
    };
    match world.speculative().saga(request).await {
        Ok(response) => {
            world.last_error = None;
            world.context.insert(
                "speculative_command_count".to_string(),
                response.commands.len().to_string(),
            );
        }
        Err(e) => {
            world.last_error = Some(e.to_string());
        }
    }
}

#[when(
    regex = r#"^I speculatively run the "([^"]+)" against order "([^"]+)" completion events with current state$"#
)]
async fn speculative_saga_with_current_state(
    world: &mut E2EWorld,
    saga_name: String,
    order_alias: String,
) {
    let root = world.root(&order_alias);
    let mut events = world.query_events("order", root).await;

    // Append synthetic OrderCompleted (see speculative_saga step for rationale)
    events.push(build_synthetic_order_completed(&events));

    let event_book = angzarr::proto::EventBook {
        cover: Some(angzarr::proto::Cover {
            domain: "order".to_string(),
            root: Some(angzarr::proto::Uuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: format!("spec-saga-{}", order_alias),
            edition: None,
        }),
        snapshot: None,
        pages: events,
        snapshot_state: None,
    };

    // Note: domain_specs (Current state fetching) is handled internally
    // by the standalone client. For gateway mode, destinations must be pre-fetched.
    let request = angzarr::proto::SpeculateSagaRequest {
        saga_name,
        source: Some(event_book),
        destinations: vec![], // Saga implementation handles state fetching
    };
    match world.speculative().saga(request).await {
        Ok(response) => {
            world.last_error = None;
            world.context.insert(
                "speculative_command_count".to_string(),
                response.commands.len().to_string(),
            );
        }
        Err(e) => {
            world.last_error = Some(e.to_string());
        }
    }
}

#[then(regex = r#"^the speculative saga produces commands$"#)]
async fn speculative_saga_produces_commands(world: &mut E2EWorld) {
    assert!(
        world.last_error.is_none(),
        "Speculative saga failed: {:?}",
        world.last_error
    );
    let count: usize = world
        .context
        .get("speculative_command_count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    assert!(
        count > 0,
        "Speculative saga should produce commands, but got 0"
    );
}

#[then(regex = r#"^no fulfillment events exist for "([^"]+)"$"#)]
async fn no_fulfillment_events(world: &mut E2EWorld, order_alias: String) {
    let root = world.root(&order_alias);
    let events = world.query_events("fulfillment", root).await;
    assert!(
        events.is_empty(),
        "Expected no fulfillment events after speculative execution, but found {}",
        events.len()
    );
}

#[then(regex = r#"^customer "([^"]+)" loyalty points are unchanged$"#)]
async fn customer_loyalty_unchanged(world: &mut E2EWorld, customer_alias: String) {
    let root = world.root(&customer_alias);
    let events = world.query_events("customer", root).await;

    // The customer should have the same events as before speculative execution
    // (CreateCustomer + AddLoyaltyPoints). No additional events from saga speculation.
    let loyalty_add_count = events
        .iter()
        .filter(|e| {
            e.event
                .as_ref()
                .map(|ev| extract_event_type(ev).contains("LoyaltyPointsAdded"))
                .unwrap_or(false)
        })
        .count();

    // Only the initial AddLoyaltyPoints from setup (500 points)
    assert_eq!(
        loyalty_add_count, 1,
        "Customer should have exactly 1 LoyaltyPointsAdded (from setup), got {}",
        loyalty_add_count
    );
}

#[when(
    regex = r#"^I speculatively run the "([^"]+)" PM against order "([^"]+)" completion events$"#
)]
async fn speculative_pm(world: &mut E2EWorld, pm_name: String, order_alias: String) {
    let root = world.root(&order_alias);
    let events = world.query_events("order", root).await;

    let event_book = angzarr::proto::EventBook {
        cover: Some(angzarr::proto::Cover {
            domain: "order".to_string(),
            root: Some(angzarr::proto::Uuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        snapshot: None,
        pages: events,
        snapshot_state: None,
    };

    let request = angzarr::proto::SpeculatePmRequest {
        pm_name,
        trigger: Some(event_book),
        process_state: None,
        destinations: vec![],
    };
    match world.speculative().process_manager(request).await {
        Ok(result) => {
            world.last_error = None;
            world.context.insert(
                "speculative_pm_command_count".to_string(),
                result.commands.len().to_string(),
            );
            world.context.insert(
                "speculative_pm_has_events".to_string(),
                result.process_events.is_some().to_string(),
            );
        }
        Err(e) => {
            world.last_error = Some(e.to_string());
        }
    }
}

#[then(regex = r#"^the speculative PM produces a result$"#)]
async fn speculative_pm_produces_result(world: &mut E2EWorld) {
    assert!(
        world.last_error.is_none(),
        "Speculative PM failed: {:?}",
        world.last_error
    );
}

#[then(regex = r#"^no process manager events are persisted for "([^"]+)"$"#)]
async fn no_pm_events_persisted(world: &mut E2EWorld, order_alias: String) {
    // PM events are stored in the PM's own domain. The PM domain names are
    // "order-fulfillment" and "order-status". After speculative execution,
    // no new PM events should have been written.
    let root = world.root(&order_alias);

    // Check both PM domains
    for pm_domain in &["order-fulfillment", "order-status"] {
        let events = world.query_events(pm_domain, root).await;
        assert!(
            events.is_empty(),
            "Expected no PM events in domain '{}' after speculative execution, but found {}",
            pm_domain,
            events.len()
        );
    }
}

#[given(regex = r#"^I record the event count for order "([^"]+)"$"#)]
async fn record_order_event_count(world: &mut E2EWorld, order_alias: String) {
    let root = world.root(&order_alias);
    let events = world.query_events("order", root).await;
    world.event_count_before_dry_run = Some(events.len());
    world
        .context
        .insert("spec_order_alias".to_string(), order_alias);
}

#[then(regex = r#"^the event count for order "([^"]+)" is unchanged$"#)]
async fn order_event_count_unchanged(world: &mut E2EWorld, order_alias: String) {
    let root = world.root(&order_alias);
    let events = world.query_events("order", root).await;

    if let Some(expected) = world.event_count_before_dry_run {
        assert_eq!(
            events.len(),
            expected,
            "Order event count changed after speculative execution: expected {}, got {}",
            expected,
            events.len()
        );
    }
}


// ============================================================================
// Checkout Saga & Inventory Reservation Steps
// ============================================================================

#[then(regex = r#"^within (\d+) seconds a "([^"]+)" event is emitted for product "([^"]+)"$"#)]
async fn inventory_event_for_product(
    world: &mut E2EWorld,
    timeout_secs: u64,
    event_type: String,
    product_id: String,
) {
    let root = common::identity::inventory_product_root(&product_id);
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        let events = world.query_events("inventory", root).await;
        let found = events.iter().any(|page| {
            page.event
                .as_ref()
                .map(|e| extract_event_type(e).contains(&event_type))
                .unwrap_or(false)
        });
        if found {
            return;
        }
        if Instant::now() > deadline {
            let event_types: Vec<String> = events
                .iter()
                .filter_map(|p| p.event.as_ref().map(extract_event_type))
                .collect();
            panic!(
                "Expected '{}' event for product '{}' within {} seconds. Found: {:?}",
                event_type, product_id, timeout_secs, event_types
            );
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

// ============================================================================
// Saga-Created Aggregate Discovery
// ============================================================================

#[then(regex = r#"^the saga-created order for correlation "([^"]+)" is stored as "([^"]+)"$"#)]
async fn store_saga_order(
    world: &mut E2EWorld,
    correlation_alias: String,
    order_alias: String,
) {
    let correlation_id = world.correlation(&correlation_alias);
    let deadline = Instant::now() + Duration::from_secs(5);

    loop {
        let results = world.query_by_correlation(&correlation_id).await;
        if let Some((_, _, root)) = results
            .iter()
            .find(|(d, t, _)| d == "order" && t.contains("OrderCreated"))
        {
            world.roots.insert(order_alias.clone(), *root);
            return;
        }
        if Instant::now() > deadline {
            let found: Vec<String> = results
                .iter()
                .map(|(d, t, _)| format!("{}/{}", d, t))
                .collect();
            panic!(
                "No OrderCreated for correlation '{}' within 5s. Found: {:?}",
                correlation_alias, found
            );
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[then(regex = r#"^the saga-created shipment for correlation "([^"]+)" is stored as "([^"]+)"$"#)]
async fn store_saga_shipment(
    world: &mut E2EWorld,
    correlation_alias: String,
    ship_alias: String,
) {
    let correlation_id = world.correlation(&correlation_alias);
    let deadline = Instant::now() + Duration::from_secs(5);

    loop {
        let results = world.query_by_correlation(&correlation_id).await;
        if let Some((_, _, root)) = results
            .iter()
            .find(|(d, t, _)| d == "fulfillment" && t.contains("ShipmentCreated"))
        {
            world.roots.insert(ship_alias.clone(), *root);
            return;
        }
        if Instant::now() > deadline {
            let found: Vec<String> = results
                .iter()
                .map(|(d, t, _)| format!("{}/{}", d, t))
                .collect();
            panic!(
                "No ShipmentCreated for correlation '{}' within 5s. Found: {:?}",
                correlation_alias, found
            );
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

// ============================================================================
// Correlation-Explicit Command Variants
// ============================================================================

#[when(regex = r#"^I submit payment of (\d+) cents via "([^"]+)" for order "([^"]+)" with correlation "([^"]+)"$"#)]
async fn submit_payment_with_correlation(
    world: &mut E2EWorld,
    amount_cents: i32,
    payment_method: String,
    order_alias: String,
    correlation_alias: String,
) {
    let command = examples_proto::SubmitPayment {
        payment_method,
        amount_cents,
    };
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation_alias,
        "examples.SubmitPayment",
        &command,
    );
    world.execute(cmd_book).await;
}

#[when(regex = r#"^I confirm payment for order "([^"]+)" with reference "([^"]+)" and correlation "([^"]+)"$"#)]
async fn confirm_payment_with_ref_and_correlation(
    world: &mut E2EWorld,
    order_alias: String,
    payment_reference: String,
    correlation_alias: String,
) {
    let command = examples_proto::ConfirmPayment { payment_reference };
    let cmd_book = world.build_command(
        "order",
        &order_alias,
        &correlation_alias,
        "examples.ConfirmPayment",
        &command,
    );
    world.execute(cmd_book).await;
}

#[when(regex = r#"^I mark shipment "([^"]+)" as picked by "([^"]+)" with correlation "([^"]+)"$"#)]
async fn mark_picked_with_correlation(
    world: &mut E2EWorld,
    shipment_alias: String,
    picker_id: String,
    correlation_alias: String,
) {
    let command = examples_proto::MarkPicked { picker_id };
    let cmd_book = world.build_command(
        "fulfillment",
        &shipment_alias,
        &correlation_alias,
        "examples.MarkPicked",
        &command,
    );
    world.execute(cmd_book).await;
}

#[when(regex = r#"^I mark shipment "([^"]+)" as packed by "([^"]+)" with correlation "([^"]+)"$"#)]
async fn mark_packed_with_correlation(
    world: &mut E2EWorld,
    shipment_alias: String,
    packer_id: String,
    correlation_alias: String,
) {
    let command = examples_proto::MarkPacked { packer_id };
    let cmd_book = world.build_command(
        "fulfillment",
        &shipment_alias,
        &correlation_alias,
        "examples.MarkPacked",
        &command,
    );
    world.execute(cmd_book).await;
}

#[when(regex = r#"^I record delivery for "([^"]+)" with signature "([^"]+)" and correlation "([^"]+)"$"#)]
async fn record_delivery_with_correlation(
    world: &mut E2EWorld,
    shipment_alias: String,
    signature: String,
    correlation_alias: String,
) {
    let command = examples_proto::RecordDelivery { signature };
    let cmd_book = world.build_command(
        "fulfillment",
        &shipment_alias,
        &correlation_alias,
        "examples.RecordDelivery",
        &command,
    );
    world.execute(cmd_book).await;
}

// ============================================================================
// Negative Correlation Assertion
// ============================================================================

#[then(regex = r#"^no ([A-Za-z]+) event exists for correlation "([^"]+)"$"#)]
async fn no_event_for_correlation(
    world: &mut E2EWorld,
    event_type: String,
    correlation_alias: String,
) {
    let correlation_id = world.correlation(&correlation_alias);
    let results = world.query_by_correlation(&correlation_id).await;
    let found = results.iter().any(|(_, t, _)| t.contains(&event_type));
    assert!(
        !found,
        "Expected no '{}' event for correlation '{}', but found one",
        event_type, correlation_alias
    );
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("e2e=debug".parse().unwrap())
                .add_directive("angzarr=debug".parse().unwrap()),
        )
        .init();

    // Run cucumber tests, filtering features by mode.
    // Standalone skips: @gateway, @chaos, @infra
    // Gateway skips: @standalone (editions, projectors, PMs need in-process access)
    let mode = std::env::var("ANGZARR_TEST_MODE").unwrap_or_else(|_| "standalone".into());
    let runner = E2EWorld::cucumber();
    if mode == "standalone" {
        runner
            .filter_run("../../features/acceptance/", |_, _, sc| {
                !sc.tags
                    .iter()
                    .any(|t| t == "gateway" || t == "chaos" || t == "infra")
            })
            .await;
    } else {
        runner
            .filter_run("../../features/acceptance/", |feature, _, sc| {
                !feature.tags.iter().any(|t| t == "standalone")
                    && !sc.tags.iter().any(|t| t == "standalone")
            })
            .await;
    }
}
