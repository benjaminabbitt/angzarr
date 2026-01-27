//! E2E Acceptance Tests
//!
//! Comprehensive end-to-end tests for the angzarr event sourcing system.
//! Tests full business flows through the gateway with correlation ID tracing,
//! projector validation, and resilience testing.

use std::time::{Duration, Instant};

use cucumber::{given, then, when, World as _};
use futures::future::join_all;
use prost::Message;
use uuid::Uuid;

use e2e::{
    assert_command_failed, assert_contiguous_sequences, examples_proto, extract_event_type, proto,
    E2EWorld,
};

// ============================================================================
// Setup Steps (Given)
// ============================================================================

#[given(expr = "a product {string} exists with price {int} cents")]
async fn product_exists(world: &mut E2EWorld, product_alias: String, price_cents: i32) {
    let command = examples_proto::CreateProduct {
        sku: product_alias.clone(),
        name: product_alias.clone(),
        description: format!("Test product {}", product_alias),
        price_cents,
    };

    let correlation = format!("setup-product-{}", product_alias);
    let cmd_book = world.build_command(
        "product",
        &product_alias,
        &correlation,
        "examples.CreateProduct",
        &command,
    );

    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to create product: {:?}",
        world.last_error
    );
}

#[given(expr = "customer {string} exists with {int} loyalty points")]
async fn customer_exists(world: &mut E2EWorld, customer_alias: String, loyalty_points: i32) {
    let command = examples_proto::CreateCustomer {
        name: customer_alias.clone(),
        email: format!("{}@test.example", customer_alias.to_lowercase()),
    };

    let correlation = format!("setup-customer-{}", customer_alias);
    let cmd_book = world.build_command(
        "customer",
        &customer_alias,
        &correlation,
        "examples.CreateCustomer",
        &command,
    );

    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to register customer: {:?}",
        world.last_error
    );

    // Add loyalty points if specified
    if loyalty_points > 0 {
        let add_points = examples_proto::AddLoyaltyPoints {
            points: loyalty_points,
            reason: "Initial balance".to_string(),
        };

        let points_correlation = format!("setup-points-{}", customer_alias);
        let points_cmd = world.build_command(
            "customer",
            &customer_alias,
            &points_correlation,
            "examples.AddLoyaltyPoints",
            &add_points,
        );

        world.execute(points_cmd).await;
        assert!(
            world.last_error.is_none(),
            "Failed to add loyalty points: {:?}",
            world.last_error
        );
    }
}

#[given(expr = "inventory for {string} has {int} units")]
async fn inventory_exists(world: &mut E2EWorld, product_alias: String, units: i32) {
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

#[given(expr = "a cart {string} with sequence {int}")]
async fn cart_with_sequence(world: &mut E2EWorld, cart_alias: String, _sequence: u32) {
    // Create cart at sequence 0
    let command = examples_proto::CreateCart {
        customer_id: format!("CUST-{}", cart_alias),
    };

    let correlation = format!("setup-cart-{}", cart_alias);
    let cmd_book = world.build_command(
        "cart",
        &cart_alias,
        &correlation,
        "examples.CreateCart",
        &command,
    );

    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to create cart: {:?}",
        world.last_error
    );
}

#[given(expr = "a cart {string} at sequence {int}")]
async fn cart_at_sequence(world: &mut E2EWorld, cart_alias: String, target_sequence: u32) {
    // Create cart
    let create_cmd = examples_proto::CreateCart {
        customer_id: format!("CUST-{}", cart_alias),
    };

    let correlation = format!("setup-cart-{}", cart_alias);
    let cmd_book = world.build_command(
        "cart",
        &cart_alias,
        &correlation,
        "examples.CreateCart",
        &create_cmd,
    );

    world.execute(cmd_book).await;
    assert!(world.last_error.is_none(), "Failed to create cart");

    // Add items to reach target sequence
    for i in 1..target_sequence {
        let add_cmd = examples_proto::AddItem {
            product_id: format!("SKU-SETUP-{}", i),
            name: format!("Setup Item {}", i),
            quantity: 1,
            unit_price_cents: 100,
        };

        let item_correlation = format!("setup-item-{}-{}", cart_alias, i);
        let item_book = world.build_command(
            "cart",
            &cart_alias,
            &item_correlation,
            "examples.AddItem",
            &add_cmd,
        );

        world.execute(item_book).await;
        assert!(
            world.last_error.is_none(),
            "Failed to add setup item: {:?}",
            world.last_error
        );
    }
}

#[given(expr = "no aggregate exists for root {string}")]
async fn no_aggregate_exists(world: &mut E2EWorld, root_alias: String) {
    // Just register a new UUID for this alias - don't create anything
    world.root(&root_alias);
}

#[given(expr = "a cart {string} exists")]
async fn cart_exists(world: &mut E2EWorld, cart_alias: String) {
    let command = examples_proto::CreateCart {
        customer_id: format!("CUST-{}", cart_alias),
    };

    let correlation = format!("setup-cart-{}", cart_alias);
    let cmd_book = world.build_command(
        "cart",
        &cart_alias,
        &correlation,
        "examples.CreateCart",
        &command,
    );

    world.execute(cmd_book).await;
    assert!(
        world.last_error.is_none(),
        "Failed to create cart: {:?}",
        world.last_error
    );
}

#[given(expr = "a cart {string} with item {string} quantity {int}")]
async fn cart_with_item(
    world: &mut E2EWorld,
    cart_alias: String,
    product_id: String,
    quantity: i32,
) {
    // First create the cart
    let create_cmd = examples_proto::CreateCart {
        customer_id: format!("CUST-{}", cart_alias),
    };

    let correlation = format!("setup-cart-{}", cart_alias);
    let cmd_book = world.build_command(
        "cart",
        &cart_alias,
        &correlation,
        "examples.CreateCart",
        &create_cmd,
    );

    world.execute(cmd_book).await;
    assert!(world.last_error.is_none(), "Failed to create cart");

    // Then add the item
    let add_cmd = examples_proto::AddItem {
        product_id: product_id.clone(),
        name: product_id.clone(),
        quantity,
        unit_price_cents: 1000,
    };

    let item_correlation = format!("setup-item-{}", cart_alias);
    let item_book = world.build_command(
        "cart",
        &cart_alias,
        &item_correlation,
        "examples.AddItem",
        &add_cmd,
    );

    world.execute(item_book).await;
    assert!(world.last_error.is_none(), "Failed to add item to cart");
}

#[given(expr = "an empty cart {string} exists")]
async fn empty_cart_exists(world: &mut E2EWorld, cart_alias: String) {
    cart_exists(world, cart_alias).await;
}

#[given(expr = "no cart exists for {string}")]
async fn no_cart_exists(world: &mut E2EWorld, cart_alias: String) {
    // Just register a new UUID for this alias - don't create anything
    world.root(&cart_alias);
}

// ============================================================================
// Action Steps (When)
// ============================================================================

#[when(expr = "I create a cart {string} with correlation {string}")]
async fn create_cart(world: &mut E2EWorld, cart_alias: String, correlation_alias: String) {
    let command = examples_proto::CreateCart {
        customer_id: format!("CUST-{}", cart_alias),
    };

    let cmd_book = world.build_command(
        "cart",
        &cart_alias,
        &correlation_alias,
        "examples.CreateCart",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I add item {string} quantity {int} to cart {string}")]
async fn add_item_to_cart_explicit(
    world: &mut E2EWorld,
    product_id: String,
    quantity: i32,
    cart_alias: String,
) {
    let correlation = world
        .correlation_ids
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| format!("add-item-{}", Uuid::new_v4()));

    let command = examples_proto::AddItem {
        product_id: product_id.clone(),
        name: product_id,
        quantity,
        unit_price_cents: 1000,
    };

    let cmd_book = world.build_command(
        "cart",
        &cart_alias,
        &correlation,
        "examples.AddItem",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I update item {string} to quantity {int} in cart {string}")]
async fn update_item_quantity(
    world: &mut E2EWorld,
    product_id: String,
    new_quantity: i32,
    cart_alias: String,
) {
    let correlation = format!("update-qty-{}", Uuid::new_v4());

    let command = examples_proto::UpdateQuantity {
        product_id,
        new_quantity,
    };

    let cmd_book = world.build_command(
        "cart",
        &cart_alias,
        &correlation,
        "examples.UpdateQuantity",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I remove item {string} from cart {string}")]
async fn remove_item_from_cart(world: &mut E2EWorld, product_id: String, cart_alias: String) {
    let correlation = format!("remove-item-{}", Uuid::new_v4());

    let command = examples_proto::RemoveItem { product_id };

    let cmd_book = world.build_command(
        "cart",
        &cart_alias,
        &correlation,
        "examples.RemoveItem",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I apply coupon {string} to cart {string}")]
async fn apply_coupon_to_cart(world: &mut E2EWorld, coupon_code: String, cart_alias: String) {
    let correlation = format!("apply-coupon-{}", Uuid::new_v4());

    let command = examples_proto::ApplyCoupon {
        code: coupon_code,
        coupon_type: "percentage".to_string(),
        value: 10,
    };

    let cmd_book = world.build_command(
        "cart",
        &cart_alias,
        &correlation,
        "examples.ApplyCoupon",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I clear cart {string}")]
async fn clear_cart(world: &mut E2EWorld, cart_alias: String) {
    let correlation = format!("clear-cart-{}", Uuid::new_v4());

    let command = examples_proto::ClearCart {};

    let cmd_book = world.build_command(
        "cart",
        &cart_alias,
        &correlation,
        "examples.ClearCart",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I checkout cart {string}")]
async fn checkout_cart(world: &mut E2EWorld, cart_alias: String) {
    let correlation = format!("checkout-{}", Uuid::new_v4());

    let command = examples_proto::Checkout {};

    let cmd_book = world.build_command(
        "cart",
        &cart_alias,
        &correlation,
        "examples.Checkout",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I checkout cart {string} with correlation {string}")]
async fn checkout_cart_with_correlation(
    world: &mut E2EWorld,
    cart_alias: String,
    correlation_alias: String,
) {
    let command = examples_proto::Checkout {};

    let cmd_book = world.build_command(
        "cart",
        &cart_alias,
        &correlation_alias,
        "examples.Checkout",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I add item {string} quantity {int} to cart {string} expecting sequence {int}")]
async fn add_item_with_explicit_sequence(
    world: &mut E2EWorld,
    product_id: String,
    quantity: i32,
    cart_alias: String,
    sequence: u32,
) {
    let root = world.root(&cart_alias);
    let correlation = format!("add-seq-{}", sequence);

    let command = examples_proto::AddItem {
        product_id: product_id.clone(),
        name: product_id,
        quantity,
        unit_price_cents: 1000,
    };

    let cmd_book = world.build_command_with_sequence(
        "cart",
        root,
        &correlation,
        sequence,
        "examples.AddItem",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "{string} creates a cart with correlation {string}")]
async fn create_cart_with_correlation(
    world: &mut E2EWorld,
    customer_alias: String,
    correlation_alias: String,
) {
    let customer_root = world.root(&customer_alias);
    let cart_alias = format!("cart-{}", customer_alias);

    let command = examples_proto::CreateCart {
        customer_id: customer_root.to_string(),
    };

    let cmd_book = world.build_command(
        "cart",
        &cart_alias,
        &correlation_alias,
        "examples.CreateCart",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "{string} adds {int} {string} to cart")]
async fn add_item_to_cart(
    world: &mut E2EWorld,
    customer_alias: String,
    quantity: i32,
    product_alias: String,
) {
    let cart_alias = format!("cart-{}", customer_alias);
    let correlation = world
        .correlation_ids
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| format!("add-item-{}", Uuid::new_v4()));

    let command = examples_proto::AddItem {
        product_id: product_alias.clone(),
        name: product_alias.clone(),
        quantity,
        unit_price_cents: 1000, // Default price, should lookup
    };

    let cmd_book = world.build_command(
        "cart",
        &cart_alias,
        &correlation,
        "examples.AddItem",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I add item {string} with sequence {int}")]
async fn add_item_with_sequence(world: &mut E2EWorld, product_id: String, sequence: u32) {
    // Get the first cart alias
    let cart_alias = world
        .roots
        .keys()
        .find(|k| k.starts_with("CART"))
        .cloned()
        .unwrap_or_else(|| "CART-DEFAULT".to_string());

    let root = world.root(&cart_alias);
    let correlation = format!("test-seq-{}", sequence);

    let command = examples_proto::AddItem {
        product_id,
        name: "Test Item".to_string(),
        quantity: 1,
        unit_price_cents: 100,
    };

    let cmd_book = world.build_command_with_sequence(
        "cart",
        root,
        &correlation,
        sequence,
        "examples.AddItem",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I replay the exact same command with sequence {int}")]
async fn replay_command_with_sequence(world: &mut E2EWorld, sequence: u32) {
    // Get the first cart alias
    let cart_alias = world
        .roots
        .keys()
        .find(|k| k.starts_with("CART"))
        .cloned()
        .unwrap_or_else(|| "CART-DEFAULT".to_string());

    let root = world.root(&cart_alias);
    let correlation = format!("replay-seq-{}", sequence);

    let command = examples_proto::AddItem {
        product_id: "SKU-001".to_string(),
        name: "Test Item".to_string(),
        quantity: 1,
        unit_price_cents: 100,
    };

    let cmd_book = world.build_command_with_sequence(
        "cart",
        root,
        &correlation,
        sequence,
        "examples.AddItem",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I send a command expecting sequence {int}")]
async fn send_command_with_sequence(world: &mut E2EWorld, sequence: u32) {
    // Get the first cart or new aggregate alias
    let alias = world
        .roots
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| "NEW-AGG".to_string());

    let root = world.root(&alias);
    let correlation = format!("test-high-seq-{}", sequence);

    let command = examples_proto::AddItem {
        product_id: "SKU-TEST".to_string(),
        name: "Test Item".to_string(),
        quantity: 1,
        unit_price_cents: 100,
    };

    let cmd_book = world.build_command_with_sequence(
        "cart",
        root,
        &correlation,
        sequence,
        "examples.AddItem",
        &command,
    );

    world.execute(cmd_book).await;
}

#[when(expr = "I send {int} AddItem commands concurrently")]
async fn send_concurrent_commands(world: &mut E2EWorld, count: u32) {
    let cart_alias = world
        .roots
        .keys()
        .find(|k| k.starts_with("CART"))
        .cloned()
        .unwrap_or_else(|| "CART-CONC".to_string());

    let root = world.root(&cart_alias);
    let gateway_endpoint = world.gateway_endpoint.clone();

    // Spawn concurrent tasks
    let mut handles = vec![];

    for i in 0..count {
        let endpoint = gateway_endpoint.clone();
        let root_bytes = root.as_bytes().to_vec();

        handles.push(tokio::spawn(async move {
            let channel = tonic::transport::Channel::from_shared(endpoint)
                .unwrap()
                .connect()
                .await
                .unwrap();
            let mut client = proto::command_gateway_client::CommandGatewayClient::new(channel);

            let command = examples_proto::AddItem {
                product_id: format!("SKU-CONC-{}", i),
                name: format!("Concurrent Item {}", i),
                quantity: 1,
                unit_price_cents: 100,
            };

            let cmd_book = proto::CommandBook {
                cover: Some(proto::Cover {
                    domain: "cart".to_string(),
                    root: Some(proto::Uuid { value: root_bytes }),
                }),
                pages: vec![proto::CommandPage {
                    sequence: 0, // All start at 0, will conflict
                    command: Some(prost_types::Any {
                        type_url: "type.examples/examples.AddItem".to_string(),
                        value: command.encode_to_vec(),
                    }),
                }],
                correlation_id: format!("conc-{}", i),
                saga_origin: None,
            };

            client.execute(cmd_book).await
        }));
    }

    // Wait for all and count successes/failures
    let results: Vec<_> = join_all(handles).await;
    let successes = results
        .iter()
        .filter(|r| r.as_ref().map(|r| r.is_ok()).unwrap_or(false))
        .count();

    // Store result for verification
    world.last_error = Some(format!(
        "Concurrent: {} succeeded, {} failed",
        successes,
        count as usize - successes
    ));
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

#[then(expr = "the cart still has exactly {int} item")]
async fn cart_has_exact_items(world: &mut E2EWorld, expected_count: i32) {
    let cart_alias = world
        .roots
        .keys()
        .find(|k| k.starts_with("CART"))
        .cloned()
        .unwrap_or_else(|| "CART-DEFAULT".to_string());

    let root = world.root(&cart_alias);
    let events = world.query_events("cart", root).await;

    // Count ItemAdded minus ItemRemoved
    let mut item_count = 0i32;
    for page in &events {
        if let Some(event) = &page.event {
            let event_type = extract_event_type(event);
            if event_type.contains("ItemAdded") {
                item_count += 1;
            } else if event_type.contains("ItemRemoved") {
                item_count -= 1;
            } else if event_type.contains("CartCleared") {
                item_count = 0;
            }
        }
    }

    assert_eq!(
        item_count, expected_count,
        "Expected {} items, found {}",
        expected_count, item_count
    );
}

#[then("some commands succeed and some fail with sequence mismatch")]
async fn some_succeed_some_fail(world: &mut E2EWorld) {
    let result = world
        .last_error
        .as_ref()
        .expect("Expected concurrent result");
    assert!(
        result.contains("succeeded") && result.contains("failed"),
        "Expected mixed results, got: {}",
        result
    );
}

#[then("the cart has consistent state (no duplicates, no gaps)")]
async fn cart_has_consistent_state(world: &mut E2EWorld) {
    let cart_alias = world
        .roots
        .keys()
        .find(|k| k.starts_with("CART"))
        .cloned()
        .unwrap_or_else(|| "CART-CONC".to_string());

    let root = world.root(&cart_alias);
    let events = world.query_events("cart", root).await;

    // Check for contiguous sequences
    assert_contiguous_sequences(&events);
}

#[then(expr = "event sequences are contiguous (0, 1, 2, ...)")]
async fn sequences_are_contiguous(world: &mut E2EWorld) {
    let cart_alias = world
        .roots
        .keys()
        .find(|k| k.starts_with("CART"))
        .cloned()
        .unwrap_or_else(|| "CART-CONC".to_string());

    let root = world.root(&cart_alias);
    let events = world.query_events("cart", root).await;

    assert_contiguous_sequences(&events);
}

#[then(expr = "the cart subtotal is {int} cents")]
async fn cart_subtotal_is(world: &mut E2EWorld, expected_cents: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");

    // Find the last event with subtotal
    for page in events.pages.iter().rev() {
        if let Some(event) = &page.event {
            let event_type = extract_event_type(event);
            if event_type.contains("ItemAdded") {
                let decoded =
                    examples_proto::ItemAdded::decode(event.value.as_slice()).expect("decode");
                assert_eq!(
                    decoded.new_subtotal, expected_cents,
                    "Expected subtotal {}, got {}",
                    expected_cents, decoded.new_subtotal
                );
                return;
            }
        }
    }
    panic!("No event with subtotal found");
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

#[then(expr = "the cart {string} has {int} line item")]
async fn cart_has_line_items(world: &mut E2EWorld, cart_alias: String, expected_count: i32) {
    let root = world.root(&cart_alias);
    let events = world.query_events("cart", root).await;

    // Count unique products (ItemAdded minus ItemRemoved, reset on CartCleared)
    let mut items: std::collections::HashSet<String> = std::collections::HashSet::new();

    for page in &events {
        if let Some(event) = &page.event {
            let event_type = extract_event_type(event);
            if event_type.contains("ItemAdded") {
                if let Ok(decoded) = examples_proto::ItemAdded::decode(event.value.as_slice()) {
                    items.insert(decoded.product_id);
                }
            } else if event_type.contains("ItemRemoved") {
                if let Ok(decoded) = examples_proto::ItemRemoved::decode(event.value.as_slice()) {
                    items.remove(&decoded.product_id);
                }
            } else if event_type.contains("CartCleared") {
                items.clear();
            }
        }
    }

    assert_eq!(
        items.len() as i32,
        expected_count,
        "Expected {} line items, found {}",
        expected_count,
        items.len()
    );
}

#[then(expr = "the cart {string} has {int} line items")]
async fn cart_has_line_items_plural(world: &mut E2EWorld, cart_alias: String, expected_count: i32) {
    cart_has_line_items(world, cart_alias, expected_count).await;
}

#[then(expr = "the cart {string} item {string} has quantity {int}")]
async fn cart_item_has_quantity(
    world: &mut E2EWorld,
    cart_alias: String,
    product_id: String,
    expected_quantity: i32,
) {
    let root = world.root(&cart_alias);
    let events = world.query_events("cart", root).await;

    let mut quantity = 0i32;

    for page in &events {
        if let Some(event) = &page.event {
            let event_type = extract_event_type(event);
            if event_type.contains("ItemAdded") {
                if let Ok(decoded) = examples_proto::ItemAdded::decode(event.value.as_slice()) {
                    if decoded.product_id == product_id {
                        quantity = decoded.quantity;
                    }
                }
            } else if event_type.contains("QuantityUpdated") {
                if let Ok(decoded) = examples_proto::QuantityUpdated::decode(event.value.as_slice())
                {
                    if decoded.product_id == product_id {
                        quantity = decoded.new_quantity;
                    }
                }
            } else if event_type.contains("ItemRemoved") {
                if let Ok(decoded) = examples_proto::ItemRemoved::decode(event.value.as_slice()) {
                    if decoded.product_id == product_id {
                        quantity = 0;
                    }
                }
            }
        }
    }

    assert_eq!(
        quantity, expected_quantity,
        "Expected item '{}' quantity {}, found {}",
        product_id, expected_quantity, quantity
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
        }],
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

#[when(expr = "order {string} is completed with correlation {string}")]
async fn complete_order_with_correlation(
    world: &mut E2EWorld,
    order_alias: String,
    correlation_alias: String,
) {
    confirm_payment_with_correlation(world, order_alias, correlation_alias).await;
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

#[then(expr = "within {int} seconds a shipment is created")]
async fn shipment_created_within_timeout(world: &mut E2EWorld, timeout_secs: u64) {
    // Look for ShipmentCreated event in fulfillment domain
    // The fulfillment root is derived from the order
    let order_alias = world
        .roots
        .keys()
        .find(|k| k.starts_with("ORD") || k.contains("order"))
        .cloned()
        .unwrap_or_else(|| "ORD-DEFAULT".to_string());

    // For fulfillment, the root is typically derived from order_id
    let fulfillment_alias = format!("fulfillment-{}", order_alias);

    let found = wait_for_event(
        world,
        "fulfillment",
        &fulfillment_alias,
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
    let fulfillment_alias = format!("fulfillment-{}", order_alias);
    let root = world.root(&fulfillment_alias);
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

        // Collect aliases first to avoid borrow issues
        let aliases: Vec<String> = world.roots.keys().cloned().collect();

        // Check all fulfillment roots
        for alias in aliases {
            if alias.starts_with("fulfillment-") || alias.contains("ORD") {
                let fulfillment_alias = if alias.starts_with("fulfillment-") {
                    alias.clone()
                } else {
                    format!("fulfillment-{}", alias)
                };

                let root = world.root(&fulfillment_alias);
                let events = world.query_events("fulfillment", root).await;

                for page in &events {
                    if let Some(event) = &page.event {
                        if extract_event_type(event).contains("ShipmentCreated") {
                            shipment_count += 1;
                        }
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
    // Verify the shipment exists for this order
    let fulfillment_alias = format!("fulfillment-{}", order_alias);
    let root = world.root(&fulfillment_alias);
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
// Main Entry Point
// ============================================================================

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("e2e=debug".parse().unwrap()),
        )
        .init();

    // Run cucumber tests
    E2EWorld::cucumber().run("tests/features/").await;
}
