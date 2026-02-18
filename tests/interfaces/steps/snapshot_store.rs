//! SnapshotStore interface step definitions.

use std::collections::HashMap;

use angzarr::proto::{Snapshot, SnapshotRetention};
use angzarr::storage::SnapshotStore;
use cucumber::{given, then, when, World};
use prost_types::Any;
use uuid::Uuid;

use crate::backend::{StorageBackend, StorageContext};

/// Test context for SnapshotStore scenarios.
#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct SnapshotStoreWorld {
    backend: StorageBackend,
    context: Option<StorageContext>,
    current_domain: String,
    current_root: Uuid,
    aggregates: HashMap<String, AggregateState>,
    last_snapshot: Option<Snapshot>,
    last_error: Option<String>,
}

#[derive(Debug, Clone)]
struct AggregateState {
    domain: String,
    root: Uuid,
}

impl SnapshotStoreWorld {
    fn new() -> Self {
        Self {
            backend: StorageBackend::from_env(),
            context: None,
            current_domain: String::new(),
            current_root: Uuid::nil(),
            aggregates: HashMap::new(),
            last_snapshot: None,
            last_error: None,
        }
    }

    fn store(&self) -> &dyn SnapshotStore {
        self.context
            .as_ref()
            .expect("Storage context not initialized")
            .snapshot_store
            .as_ref()
    }

    fn make_snapshot(&self, seq: u32) -> Snapshot {
        Snapshot {
            sequence: seq,
            state: Some(Any {
                type_url: format!("type.test/State{}", seq),
                value: vec![seq as u8],
            }),
            retention: SnapshotRetention::RetentionDefault as i32,
        }
    }

    fn make_snapshot_with_data(&self, seq: u32, data: &str) -> Snapshot {
        Snapshot {
            sequence: seq,
            state: Some(Any {
                type_url: "type.test/CustomState".to_string(),
                value: data.as_bytes().to_vec(),
            }),
            retention: SnapshotRetention::RetentionDefault as i32,
        }
    }

    fn agg_key(&self, domain: &str, root: Uuid) -> String {
        format!("{}:{}", domain, root)
    }
}

// --- Background ---

#[given("a SnapshotStore backend")]
async fn given_snapshot_store_backend(world: &mut SnapshotStoreWorld) {
    println!("Using backend: {}", world.backend.name());
    let ctx = StorageContext::new(world.backend).await;
    world.context = Some(ctx);
}

// --- Given steps ---

#[given(expr = "an aggregate {string} with no snapshot")]
async fn given_aggregate_no_snapshot(world: &mut SnapshotStoreWorld, domain: String) {
    let root = Uuid::new_v4();
    world.current_domain = domain.clone();
    world.current_root = root;

    let key = world.agg_key(&domain, root);
    world
        .aggregates
        .insert(key, AggregateState { domain, root });
}

#[given(expr = "an aggregate {string} with a snapshot at sequence {int}")]
async fn given_aggregate_with_snapshot(world: &mut SnapshotStoreWorld, domain: String, seq: u32) {
    let root = Uuid::new_v4();
    world.current_domain = domain.clone();
    world.current_root = root;

    let snapshot = world.make_snapshot(seq);

    world
        .store()
        .put(&domain, "test", root, snapshot)
        .await
        .expect("Failed to put snapshot");

    let key = world.agg_key(&domain, root);
    world
        .aggregates
        .insert(key, AggregateState { domain, root });
}

#[given(expr = "an aggregate {string} with root {string} and a snapshot at sequence {int}")]
async fn given_aggregate_with_root_and_snapshot(
    world: &mut SnapshotStoreWorld,
    domain: String,
    root_name: String,
    seq: u32,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    world.current_domain = domain.clone();
    world.current_root = root;

    let snapshot = world.make_snapshot(seq);

    world
        .store()
        .put(&domain, "test", root, snapshot)
        .await
        .expect("Failed to put snapshot");

    let key = world.agg_key(&domain, root);
    world
        .aggregates
        .insert(key, AggregateState { domain, root });
}

// --- When steps ---

#[when("I get the snapshot for the aggregate")]
async fn when_get_snapshot(world: &mut SnapshotStoreWorld) {
    world.last_snapshot = world
        .store()
        .get(&world.current_domain, "test", world.current_root)
        .await
        .expect("Failed to get snapshot");
}

#[when(expr = "I put a snapshot at sequence {int}")]
async fn when_put_snapshot(world: &mut SnapshotStoreWorld, seq: u32) {
    let snapshot = world.make_snapshot(seq);

    match world
        .store()
        .put(&world.current_domain, "test", world.current_root, snapshot)
        .await
    {
        Ok(_) => world.last_error = None,
        Err(e) => world.last_error = Some(e.to_string()),
    }

    // Update last_snapshot for verification
    world.last_snapshot = world
        .store()
        .get(&world.current_domain, "test", world.current_root)
        .await
        .expect("Failed to get snapshot");
}

#[when(expr = "I put a snapshot at sequence {int} with data {string}")]
async fn when_put_snapshot_with_data(world: &mut SnapshotStoreWorld, seq: u32, data: String) {
    let snapshot = world.make_snapshot_with_data(seq, &data);

    world
        .store()
        .put(&world.current_domain, "test", world.current_root, snapshot)
        .await
        .expect("Failed to put snapshot");
}

#[when("I delete the snapshot for the aggregate")]
async fn when_delete_snapshot(world: &mut SnapshotStoreWorld) {
    match world
        .store()
        .delete(&world.current_domain, "test", world.current_root)
        .await
    {
        Ok(_) => world.last_error = None,
        Err(e) => world.last_error = Some(e.to_string()),
    }
}

#[when(expr = "I get the snapshot for root {string} in domain {string}")]
async fn when_get_snapshot_for_root(
    world: &mut SnapshotStoreWorld,
    root_name: String,
    domain: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());

    world.last_snapshot = world
        .store()
        .get(&domain, "test", root)
        .await
        .expect("Failed to get snapshot");
}

#[when(expr = "I delete the snapshot for root {string} in domain {string}")]
async fn when_delete_snapshot_for_root(
    world: &mut SnapshotStoreWorld,
    root_name: String,
    domain: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());

    world
        .store()
        .delete(&domain, "test", root)
        .await
        .expect("Failed to delete snapshot");
}

#[when(expr = "I get the snapshot for domain {string}")]
async fn when_get_snapshot_for_domain(world: &mut SnapshotStoreWorld, domain: String) {
    // Find the root for the specified domain
    let key = world
        .aggregates
        .keys()
        .find(|k| k.starts_with(&format!("{}:", domain)))
        .expect("No aggregate found for domain");
    let state = world.aggregates.get(key).expect("Aggregate not found");

    world.last_snapshot = world
        .store()
        .get(&domain, "test", state.root)
        .await
        .expect("Failed to get snapshot");
}

// --- Then steps ---

#[then("the snapshot should not exist")]
fn then_snapshot_not_exist(world: &mut SnapshotStoreWorld) {
    assert!(
        world.last_snapshot.is_none(),
        "Expected no snapshot but found one"
    );
}

#[then("the snapshot should exist")]
fn then_snapshot_exists(world: &mut SnapshotStoreWorld) {
    assert!(
        world.last_snapshot.is_some(),
        "Expected snapshot but found none"
    );
}

#[then(expr = "the snapshot should have sequence {int}")]
fn then_snapshot_sequence(world: &mut SnapshotStoreWorld, expected_seq: u32) {
    let snapshot = world.last_snapshot.as_ref().expect("No snapshot found");
    assert_eq!(
        snapshot.sequence, expected_seq,
        "Expected sequence {}, got {}",
        expected_seq, snapshot.sequence
    );
}

#[then(expr = "the snapshot should have data {string}")]
fn then_snapshot_data(world: &mut SnapshotStoreWorld, expected_data: String) {
    let snapshot = world.last_snapshot.as_ref().expect("No snapshot found");
    let state = snapshot.state.as_ref().expect("No state in snapshot");
    let data = String::from_utf8_lossy(&state.value);
    assert_eq!(
        data, expected_data,
        "Expected data '{}', got '{}'",
        expected_data, data
    );
}

#[then("the operation should succeed")]
fn then_operation_succeeds(world: &mut SnapshotStoreWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
}

#[then(expr = "the snapshot for root {string} should not exist")]
async fn then_snapshot_for_root_not_exist(world: &mut SnapshotStoreWorld, root_name: String) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());

    let snapshot = world
        .store()
        .get(&world.current_domain, "test", root)
        .await
        .expect("Failed to get snapshot");

    assert!(
        snapshot.is_none(),
        "Expected no snapshot for root {} but found one",
        root_name
    );
}

#[then(expr = "the snapshot for root {string} should exist")]
async fn then_snapshot_for_root_exists(world: &mut SnapshotStoreWorld, root_name: String) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());

    let snapshot = world
        .store()
        .get(&world.current_domain, "test", root)
        .await
        .expect("Failed to get snapshot");

    assert!(
        snapshot.is_some(),
        "Expected snapshot for root {} but found none",
        root_name
    );
}
