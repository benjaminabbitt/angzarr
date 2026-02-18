//! PositionStore interface step definitions.

use angzarr::storage::PositionStore;
use cucumber::{given, then, when, World};
use uuid::Uuid;

use crate::backend::{StorageBackend, StorageContext};

/// Test context for PositionStore scenarios.
#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct PositionStoreWorld {
    backend: StorageBackend,
    context: Option<StorageContext>,
    current_handler: String,
    current_domain: String,
    current_root: Vec<u8>,
    handlers: Vec<HandlerState>,
    last_position: Option<u32>,
    last_error: Option<String>,
}

#[derive(Debug, Clone)]
struct HandlerState {
    handler: String,
    domain: String,
    root: Vec<u8>,
}

impl PositionStoreWorld {
    fn new() -> Self {
        Self {
            backend: StorageBackend::from_env(),
            context: None,
            current_handler: String::new(),
            current_domain: String::new(),
            current_root: Vec::new(),
            handlers: Vec::new(),
            last_position: None,
            last_error: None,
        }
    }

    fn store(&self) -> &dyn PositionStore {
        self.context
            .as_ref()
            .expect("Storage context not initialized")
            .position_store
            .as_ref()
    }
}

// --- Background ---

#[given("a PositionStore backend")]
async fn given_position_store_backend(world: &mut PositionStoreWorld) {
    println!("Using backend: {}", world.backend.name());
    let ctx = StorageContext::new(world.backend).await;
    world.context = Some(ctx);
}

// --- Given steps ---

#[given(expr = "a handler {string} tracking domain {string}")]
async fn given_handler_tracking_domain(
    world: &mut PositionStoreWorld,
    handler: String,
    domain: String,
) {
    let root = Uuid::new_v4().as_bytes().to_vec();
    world.current_handler = handler.clone();
    world.current_domain = domain.clone();
    world.current_root = root.clone();

    world.handlers.push(HandlerState {
        handler,
        domain,
        root,
    });
}

#[given(expr = "a handler {string} tracking domain {string} with root {string}")]
async fn given_handler_tracking_domain_with_root(
    world: &mut PositionStoreWorld,
    handler: String,
    domain: String,
    root_name: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes())
        .as_bytes()
        .to_vec();
    world.current_handler = handler.clone();
    world.current_domain = domain.clone();
    world.current_root = root.clone();

    world.handlers.push(HandlerState {
        handler,
        domain,
        root,
    });
}

#[given(expr = "{int} handlers tracking domain {string} with root {string}")]
async fn given_multiple_handlers(
    world: &mut PositionStoreWorld,
    count: u32,
    domain: String,
    root_name: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes())
        .as_bytes()
        .to_vec();
    world.current_domain = domain.clone();
    world.current_root = root.clone();

    for i in 0..count {
        let handler = format!("handler-{}", i);
        world.handlers.push(HandlerState {
            handler: handler.clone(),
            domain: domain.clone(),
            root: root.clone(),
        });
    }
}

// --- When steps ---

#[when("I get the position for the handler")]
async fn when_get_position(world: &mut PositionStoreWorld) {
    world.last_position = world
        .store()
        .get(
            &world.current_handler,
            &world.current_domain,
            "test",
            &world.current_root,
        )
        .await
        .expect("Failed to get position");
}

#[when(expr = "I put position {int} for the handler")]
async fn when_put_position(world: &mut PositionStoreWorld, position: u32) {
    match world
        .store()
        .put(
            &world.current_handler,
            &world.current_domain,
            "test",
            &world.current_root,
            position,
        )
        .await
    {
        Ok(_) => world.last_error = None,
        Err(e) => world.last_error = Some(e.to_string()),
    }
}

#[when(expr = "I put position {int} for handler {string}")]
async fn when_put_position_for_handler(
    world: &mut PositionStoreWorld,
    position: u32,
    handler: String,
) {
    world
        .store()
        .put(
            &handler,
            &world.current_domain,
            "test",
            &world.current_root,
            position,
        )
        .await
        .expect("Failed to put position");
}

#[when(expr = "I put position {int} for domain {string}")]
async fn when_put_position_for_domain(
    world: &mut PositionStoreWorld,
    position: u32,
    domain: String,
) {
    let state = world
        .handlers
        .iter()
        .find(|h| h.domain == domain)
        .expect("Handler not found for domain");

    world
        .store()
        .put(&state.handler, &domain, "test", &state.root, position)
        .await
        .expect("Failed to put position");
}

#[when(expr = "I put position {int} for root {string}")]
async fn when_put_position_for_root(
    world: &mut PositionStoreWorld,
    position: u32,
    root_name: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes())
        .as_bytes()
        .to_vec();

    world
        .store()
        .put(
            &world.current_handler,
            &world.current_domain,
            "test",
            &root,
            position,
        )
        .await
        .expect("Failed to put position");
}

#[when("each handler puts its index times 10 as position")]
async fn when_each_handler_puts_position(world: &mut PositionStoreWorld) {
    for (i, state) in world.handlers.iter().enumerate() {
        world
            .store()
            .put(
                &state.handler,
                &state.domain,
                "test",
                &state.root,
                (i * 10) as u32,
            )
            .await
            .expect("Failed to put position");
    }
}

// --- Then steps ---

#[then("the position should not exist")]
fn then_position_not_exist(world: &mut PositionStoreWorld) {
    assert!(
        world.last_position.is_none(),
        "Expected no position but found {:?}",
        world.last_position
    );
}

#[then(expr = "the position should be {int}")]
fn then_position_is(world: &mut PositionStoreWorld, expected: u32) {
    let position = world.last_position.expect("No position found");
    assert_eq!(
        position, expected,
        "Expected position {}, got {}",
        expected, position
    );
}

#[then(expr = "the position for handler {string} should be {int}")]
async fn then_position_for_handler(world: &mut PositionStoreWorld, handler: String, expected: u32) {
    let position = world
        .store()
        .get(&handler, &world.current_domain, "test", &world.current_root)
        .await
        .expect("Failed to get position")
        .expect("Position not found");

    assert_eq!(
        position, expected,
        "Expected position {} for handler {}, got {}",
        expected, handler, position
    );
}

#[then(expr = "the position for domain {string} should be {int}")]
async fn then_position_for_domain(world: &mut PositionStoreWorld, domain: String, expected: u32) {
    let state = world
        .handlers
        .iter()
        .find(|h| h.domain == domain)
        .expect("Handler not found for domain");

    let position = world
        .store()
        .get(&state.handler, &domain, "test", &state.root)
        .await
        .expect("Failed to get position")
        .expect("Position not found");

    assert_eq!(
        position, expected,
        "Expected position {} for domain {}, got {}",
        expected, domain, position
    );
}

#[then(expr = "the position for root {string} should be {int}")]
async fn then_position_for_root(world: &mut PositionStoreWorld, root_name: String, expected: u32) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes())
        .as_bytes()
        .to_vec();

    let position = world
        .store()
        .get(&world.current_handler, &world.current_domain, "test", &root)
        .await
        .expect("Failed to get position")
        .expect("Position not found");

    assert_eq!(
        position, expected,
        "Expected position {} for root {}, got {}",
        expected, root_name, position
    );
}

#[then("each handler should have position equal to its index times 10")]
async fn then_each_handler_has_correct_position(world: &mut PositionStoreWorld) {
    for (i, state) in world.handlers.iter().enumerate() {
        let expected = (i * 10) as u32;
        let position = world
            .store()
            .get(&state.handler, &state.domain, "test", &state.root)
            .await
            .expect("Failed to get position")
            .expect("Position not found");

        assert_eq!(
            position, expected,
            "Expected position {} for handler {}, got {}",
            expected, state.handler, position
        );
    }
}
