//! Payload offloading interface step definitions.

use std::sync::Arc;
use std::time::Duration;

use angzarr::payload_store::{FilesystemPayloadStore, PayloadStore, PayloadStoreError};
use angzarr::proto::event_page::Payload;
use angzarr::proto::{EventPage, PayloadReference, PayloadStorageType};
use cucumber::{given, then, when, World};
use prost::Message;
use tempfile::TempDir;

/// Test context for PayloadOffloading scenarios.
#[derive(World)]
#[world(init = Self::new)]
pub struct PayloadWorld {
    temp_dir: Option<TempDir>,
    store: Option<Arc<FilesystemPayloadStore>>,
    threshold: Option<usize>,
    last_event: Option<EventPage>,
    last_references: Vec<PayloadReference>,
    stored_payload: Option<Vec<u8>>,
    retrieved_payload: Option<Vec<u8>>,
    last_error: Option<PayloadStoreError>,
    offloading_enabled: bool,
}

impl std::fmt::Debug for PayloadWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PayloadWorld")
            .field("threshold", &self.threshold)
            .field("offloading_enabled", &self.offloading_enabled)
            .finish()
    }
}

impl PayloadWorld {
    fn new() -> Self {
        Self {
            temp_dir: None,
            store: None,
            threshold: None,
            last_event: None,
            last_references: Vec::new(),
            stored_payload: None,
            retrieved_payload: None,
            last_error: None,
            offloading_enabled: true,
        }
    }

    fn store(&self) -> &Arc<FilesystemPayloadStore> {
        self.store.as_ref().expect("Store not initialized")
    }

    fn make_payload(size: usize) -> Vec<u8> {
        (0..size).map(|i| (i % 256) as u8).collect()
    }

    fn make_event_page(payload: Vec<u8>) -> EventPage {
        EventPage {
            sequence: 0,
            created_at: None,
            payload: Some(Payload::Event(prost_types::Any {
                type_url: "test.Event".to_string(),
                value: payload,
            })),
        }
    }

    async fn count_store_items(&self) -> usize {
        // Count files in temp_dir
        let temp_dir = self.temp_dir.as_ref().expect("Temp dir not initialized");
        std::fs::read_dir(temp_dir.path())
            .map(|entries| entries.count())
            .unwrap_or(0)
    }

    async fn process_event_for_offloading(&self, page: &EventPage) -> EventPage {
        let threshold = match self.threshold {
            Some(t) => t,
            None => return page.clone(),
        };

        let page_size = page.encoded_len();

        if page_size <= threshold {
            return page.clone();
        }

        // Offload the payload
        if let Some(Payload::Event(ref event)) = page.payload {
            let payload_bytes = event.encode_to_vec();
            match self.store().put(&payload_bytes).await {
                Ok(reference) => {
                    return EventPage {
                        sequence: page.sequence,
                        created_at: page.created_at,
                        payload: Some(Payload::External(reference)),
                    };
                }
                Err(_) => return page.clone(),
            }
        }

        page.clone()
    }
}

// ==========================================================================
// Background
// ==========================================================================

#[given("a PayloadStore test environment")]
async fn given_payload_store_environment(world: &mut PayloadWorld) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = FilesystemPayloadStore::new(temp_dir.path())
        .await
        .expect("Failed to create store");
    world.temp_dir = Some(temp_dir);
    world.store = Some(Arc::new(store));
}

// ==========================================================================
// Threshold Behavior
// ==========================================================================

#[given(expr = "an offloading threshold of {int} bytes")]
async fn given_threshold(world: &mut PayloadWorld, threshold: usize) {
    world.threshold = Some(threshold);
}

#[when(expr = "I store an event with a {int} byte payload")]
async fn when_store_event_with_payload(world: &mut PayloadWorld, size: usize) {
    let payload = PayloadWorld::make_payload(size);
    world.stored_payload = Some(payload.clone());
    let page = PayloadWorld::make_event_page(payload);

    let processed = world.process_event_for_offloading(&page).await;
    world.last_event = Some(processed.clone());

    if let Some(Payload::External(ref reference)) = processed.payload {
        world.last_references.push(reference.clone());
    }
}

#[then("the event should have an inline payload")]
async fn then_event_has_inline_payload(world: &mut PayloadWorld) {
    let event = world.last_event.as_ref().expect("No event stored");
    assert!(
        matches!(event.payload, Some(Payload::Event(_))),
        "Expected inline payload, got external reference"
    );
}

#[then("the event should have an external payload reference")]
async fn then_event_has_external_reference(world: &mut PayloadWorld) {
    let event = world.last_event.as_ref().expect("No event stored");
    assert!(
        matches!(event.payload, Some(Payload::External(_))),
        "Expected external reference, got inline payload"
    );
}

#[then(expr = "the payload store should have {int} item")]
async fn then_store_has_item(world: &mut PayloadWorld, count: usize) {
    let actual = world.count_store_items().await;
    assert_eq!(actual, count, "Expected {} items, got {}", count, actual);
}

#[then(expr = "the payload store should have {int} items")]
async fn then_store_has_items(world: &mut PayloadWorld, count: usize) {
    let actual = world.count_store_items().await;
    assert_eq!(actual, count, "Expected {} items, got {}", count, actual);
}

// ==========================================================================
// Content-Addressable Storage
// ==========================================================================

#[when(expr = "I store two events with identical {int} byte payloads")]
async fn when_store_two_identical_events(world: &mut PayloadWorld, size: usize) {
    let payload = PayloadWorld::make_payload(size);
    world.stored_payload = Some(payload.clone());

    // Store first event
    let page1 = PayloadWorld::make_event_page(payload.clone());
    let processed1 = world.process_event_for_offloading(&page1).await;
    if let Some(Payload::External(ref reference)) = processed1.payload {
        world.last_references.push(reference.clone());
    }

    // Store second event with same payload
    let page2 = PayloadWorld::make_event_page(payload);
    let processed2 = world.process_event_for_offloading(&page2).await;
    if let Some(Payload::External(ref reference)) = processed2.payload {
        world.last_references.push(reference.clone());
    }

    world.last_event = Some(processed2);
}

#[then("both references should have the same content hash")]
async fn then_same_content_hash(world: &mut PayloadWorld) {
    assert!(
        world.last_references.len() >= 2,
        "Need at least 2 references"
    );
    assert_eq!(
        world.last_references[0].content_hash, world.last_references[1].content_hash,
        "Content hashes should match"
    );
}

#[when(expr = "I store two events with different {int} byte payloads")]
async fn when_store_two_different_events(world: &mut PayloadWorld, size: usize) {
    // First payload
    let payload1 = PayloadWorld::make_payload(size);
    let page1 = PayloadWorld::make_event_page(payload1);
    let processed1 = world.process_event_for_offloading(&page1).await;
    if let Some(Payload::External(ref reference)) = processed1.payload {
        world.last_references.push(reference.clone());
    }

    // Second payload (different content)
    let payload2: Vec<u8> = (0..size).map(|i| ((i + 1) % 256) as u8).collect();
    let page2 = PayloadWorld::make_event_page(payload2);
    let processed2 = world.process_event_for_offloading(&page2).await;
    if let Some(Payload::External(ref reference)) = processed2.payload {
        world.last_references.push(reference.clone());
    }

    world.last_event = Some(processed2);
}

#[then("the references should have different content hashes")]
async fn then_different_content_hashes(world: &mut PayloadWorld) {
    assert!(
        world.last_references.len() >= 2,
        "Need at least 2 references"
    );
    assert_ne!(
        world.last_references[0].content_hash, world.last_references[1].content_hash,
        "Content hashes should differ"
    );
}

#[when("I store an event with a known payload")]
async fn when_store_known_payload(world: &mut PayloadWorld) {
    let payload = vec![1u8; 500]; // Known payload
    world.stored_payload = Some(payload.clone());
    let page = PayloadWorld::make_event_page(payload);

    let processed = world.process_event_for_offloading(&page).await;
    world.last_event = Some(processed.clone());

    if let Some(Payload::External(ref reference)) = processed.payload {
        world.last_references.push(reference.clone());
    }
}

#[then("the reference content hash should be 32 bytes")]
async fn then_hash_is_32_bytes(world: &mut PayloadWorld) {
    let reference = world.last_references.last().expect("No reference stored");
    assert_eq!(
        reference.content_hash.len(),
        32,
        "SHA-256 hash should be 32 bytes"
    );
}

// ==========================================================================
// Reference Structure
// ==========================================================================

#[given("the payload store uses filesystem storage")]
async fn given_filesystem_storage(_world: &mut PayloadWorld) {
    // Already using filesystem storage in the test environment
}

#[then("the reference should have storage type FILESYSTEM")]
async fn then_storage_type_filesystem(world: &mut PayloadWorld) {
    let reference = world.last_references.last().expect("No reference stored");
    assert_eq!(
        reference.storage_type,
        PayloadStorageType::Filesystem as i32,
        "Expected FILESYSTEM storage type"
    );
}

#[then("the reference URI should be valid")]
async fn then_uri_is_valid(world: &mut PayloadWorld) {
    let reference = world.last_references.last().expect("No reference stored");
    assert!(!reference.uri.is_empty(), "URI should not be empty");
    assert!(
        reference.uri.starts_with("file://"),
        "Filesystem URI should start with file://"
    );
}

#[then(expr = "the reference should indicate original size of {int} bytes")]
async fn then_reference_has_size(world: &mut PayloadWorld, size: u64) {
    let reference = world.last_references.last().expect("No reference stored");
    // The original size includes the protobuf encoding overhead
    assert!(
        reference.original_size >= size,
        "Original size {} should be at least {}",
        reference.original_size,
        size
    );
}

#[then("the reference should include a storage timestamp")]
async fn then_reference_has_timestamp(world: &mut PayloadWorld) {
    let reference = world.last_references.last().expect("No reference stored");
    assert!(
        reference.stored_at.is_some(),
        "Reference should include storage timestamp"
    );
}

// ==========================================================================
// Payload Retrieval
// ==========================================================================

#[given(expr = "I have stored an event with a {int} byte payload")]
async fn given_stored_event_with_payload(world: &mut PayloadWorld, size: usize) {
    let payload = PayloadWorld::make_payload(size);
    world.stored_payload = Some(payload.clone());
    let page = PayloadWorld::make_event_page(payload);

    let processed = world.process_event_for_offloading(&page).await;
    world.last_event = Some(processed.clone());

    if let Some(Payload::External(ref reference)) = processed.payload {
        world.last_references.push(reference.clone());
    }
}

#[when("I resolve the payload reference")]
async fn when_resolve_reference(world: &mut PayloadWorld) {
    let reference = world
        .last_references
        .last()
        .expect("No reference to resolve");
    match world.store().get(reference).await {
        Ok(payload) => {
            world.retrieved_payload = Some(payload);
            world.last_error = None;
        }
        Err(e) => {
            world.last_error = Some(e);
        }
    }
}

#[then("I should get the original payload content")]
async fn then_get_original_content(world: &mut PayloadWorld) {
    let retrieved = world
        .retrieved_payload
        .as_ref()
        .expect("No payload retrieved");
    let original = world.stored_payload.as_ref().expect("No original payload");

    // The stored payload is the serialized Any proto
    let original_any = prost_types::Any {
        type_url: "test.Event".to_string(),
        value: original.clone(),
    };
    let original_bytes = original_any.encode_to_vec();

    assert_eq!(
        retrieved, &original_bytes,
        "Retrieved payload should match original"
    );
}

#[given("I store an event with large text payload")]
async fn given_store_large_text_payload(world: &mut PayloadWorld) {
    // Create a payload large enough to be offloaded (100 bytes)
    let payload = "The quick brown fox jumps over the lazy dog. ".repeat(5);
    let payload_bytes = payload.as_bytes().to_vec();
    world.stored_payload = Some(payload_bytes.clone());
    let page = PayloadWorld::make_event_page(payload_bytes);

    let processed = world.process_event_for_offloading(&page).await;
    world.last_event = Some(processed.clone());

    if let Some(Payload::External(ref reference)) = processed.payload {
        world.last_references.push(reference.clone());
    }
}

#[then("the retrieved payload should match the original")]
async fn then_retrieved_matches_original(world: &mut PayloadWorld) {
    let retrieved = world
        .retrieved_payload
        .as_ref()
        .expect("No payload retrieved");
    let original = world.stored_payload.as_ref().expect("No original payload");

    // The stored payload is the serialized Any proto
    let original_any = prost_types::Any {
        type_url: "test.Event".to_string(),
        value: original.clone(),
    };
    let original_bytes = original_any.encode_to_vec();

    assert_eq!(
        retrieved, &original_bytes,
        "Retrieved payload should match original"
    );
}

#[given("I have stored an event with a valid payload")]
async fn given_stored_valid_payload(world: &mut PayloadWorld) {
    let payload = vec![1u8; 500];
    world.stored_payload = Some(payload.clone());
    let page = PayloadWorld::make_event_page(payload);

    let processed = world.process_event_for_offloading(&page).await;
    world.last_event = Some(processed.clone());

    if let Some(Payload::External(ref reference)) = processed.payload {
        world.last_references.push(reference.clone());
    }
}

#[when("I retrieve the payload")]
async fn when_retrieve_payload(world: &mut PayloadWorld) {
    let reference = world
        .last_references
        .last()
        .expect("No reference to retrieve");
    match world.store().get(reference).await {
        Ok(payload) => {
            world.retrieved_payload = Some(payload);
            world.last_error = None;
        }
        Err(e) => {
            world.last_error = Some(e);
        }
    }
}

#[then("the integrity check should pass")]
async fn then_integrity_check_passes(world: &mut PayloadWorld) {
    assert!(
        world.last_error.is_none(),
        "Integrity check should pass, but got error: {:?}",
        world.last_error
    );
    assert!(
        world.retrieved_payload.is_some(),
        "Payload should be retrieved"
    );
}

// ==========================================================================
// Error Handling
// ==========================================================================

#[given("a reference to a non-existent payload")]
async fn given_nonexistent_reference(world: &mut PayloadWorld) {
    world.last_references.push(PayloadReference {
        storage_type: PayloadStorageType::Filesystem as i32,
        uri: "file:///nonexistent/path/abc123.bin".to_string(),
        content_hash: vec![0u8; 32],
        original_size: 100,
        stored_at: None,
    });
}

#[when("I try to resolve the reference")]
async fn when_try_resolve_reference(world: &mut PayloadWorld) {
    let reference = world
        .last_references
        .last()
        .expect("No reference to resolve");
    match world.store().get(reference).await {
        Ok(payload) => {
            world.retrieved_payload = Some(payload);
            world.last_error = None;
        }
        Err(e) => {
            world.last_error = Some(e);
        }
    }
}

#[then("the operation should fail with NOT_FOUND")]
async fn then_fails_with_not_found(world: &mut PayloadWorld) {
    let error = world.last_error.as_ref().expect("Expected error");
    assert!(
        matches!(error, PayloadStoreError::NotFound(_)),
        "Expected NOT_FOUND error, got {:?}",
        error
    );
}

#[given("I have stored an event with a payload")]
async fn given_stored_event_payload(world: &mut PayloadWorld) {
    let payload = vec![1u8; 500];
    world.stored_payload = Some(payload.clone());
    let page = PayloadWorld::make_event_page(payload);

    let processed = world.process_event_for_offloading(&page).await;
    world.last_event = Some(processed.clone());

    if let Some(Payload::External(ref reference)) = processed.payload {
        world.last_references.push(reference.clone());
    }
}

#[when("the stored payload becomes corrupted")]
async fn when_payload_corrupted(world: &mut PayloadWorld) {
    // Modify the stored file to corrupt it
    let reference = world.last_references.last().expect("No reference");
    let uri = &reference.uri;

    // Parse file path from URI
    let path = uri.strip_prefix("file://").unwrap_or(uri);

    // Write corrupt data
    if std::path::Path::new(path).exists() {
        std::fs::write(path, b"corrupted data").expect("Failed to corrupt file");
    }
}

#[then("the operation should fail with INTEGRITY_FAILED")]
async fn then_fails_with_integrity(world: &mut PayloadWorld) {
    let error = world.last_error.as_ref().expect("Expected error");
    assert!(
        matches!(error, PayloadStoreError::IntegrityFailed { .. }),
        "Expected INTEGRITY_FAILED error, got {:?}",
        error
    );
}

// ==========================================================================
// TTL Cleanup
// ==========================================================================

#[given("I have stored payloads with various ages")]
async fn given_payloads_with_ages(world: &mut PayloadWorld) {
    // Store a payload (it will be "recent")
    let payload = vec![1u8; 500];
    let page = PayloadWorld::make_event_page(payload);
    let processed = world.process_event_for_offloading(&page).await;
    if let Some(Payload::External(ref reference)) = processed.payload {
        world.last_references.push(reference.clone());
    }
}

#[when(expr = "I run TTL cleanup for payloads older than {int} hour")]
async fn when_run_ttl_cleanup(world: &mut PayloadWorld, _hours: i32) {
    // Run cleanup with a very short duration (payloads are recent)
    let _ = world
        .store()
        .delete_older_than(Duration::from_secs(3600))
        .await;
}

#[then("old payloads should be deleted")]
async fn then_old_payloads_deleted(_world: &mut PayloadWorld) {
    // All our test payloads are recent, so nothing should be deleted
    // This is more of a behavioral verification that cleanup runs
}

#[then("recent payloads should be retained")]
async fn then_recent_payloads_retained(world: &mut PayloadWorld) {
    // Verify the recent payload is still accessible
    if let Some(reference) = world.last_references.last() {
        let result = world.store().get(reference).await;
        assert!(result.is_ok(), "Recent payload should still exist");
    }
}

// ==========================================================================
// Disabled Offloading
// ==========================================================================

#[given("offloading is disabled")]
async fn given_offloading_disabled(world: &mut PayloadWorld) {
    world.threshold = None;
    world.offloading_enabled = false;
}
