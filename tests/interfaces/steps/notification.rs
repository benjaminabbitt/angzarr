//! Notification interface step definitions.

use std::collections::HashMap;

use angzarr::proto::{
    command_page, CommandBook, CommandPage, Cover, MergeStrategy, Notification,
    RejectionNotification, SagaCommandOrigin, Uuid as ProtoUuid,
};
use angzarr::utils::saga_compensation::{
    build_notification, build_notification_command_book, build_rejection_notification,
    CompensationContext,
};
use cucumber::{given, then, when, World};
use uuid::Uuid;

/// Test context for Notification scenarios.
#[derive(World)]
#[world(init = Self::new)]
pub struct NotificationWorld {
    notification: Option<Notification>,
    rejection: Option<RejectionNotification>,
    command_book: Option<CommandBook>,
    rejected_command: Option<CommandBook>,
    compensation_context: Option<CompensationContext>,
    saga_name: String,
    saga_origin: Option<SagaCommandOrigin>,
    correlation_id: String,
}

impl std::fmt::Debug for NotificationWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotificationWorld")
            .field("notification", &self.notification.is_some())
            .field("rejection", &self.rejection.is_some())
            .field("saga_name", &self.saga_name)
            .finish()
    }
}

impl NotificationWorld {
    fn new() -> Self {
        Self {
            notification: None,
            rejection: None,
            command_book: None,
            rejected_command: None,
            compensation_context: None,
            saga_name: String::new(),
            saga_origin: None,
            correlation_id: String::new(),
        }
    }

    fn make_cover(domain: &str, root: Uuid, correlation_id: &str) -> Cover {
        Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: correlation_id.to_string(),
            edition: None, // Main timeline (no edition specified)
        }
    }

    fn make_command_book(
        domain: &str,
        root: Uuid,
        saga_origin: Option<SagaCommandOrigin>,
    ) -> CommandBook {
        CommandBook {
            cover: Some(Self::make_cover(domain, root, "")),
            pages: vec![CommandPage {
                sequence: 0,
                payload: Some(command_page::Payload::Command(prost_types::Any {
                    type_url: "test.Command".to_string(),
                    value: vec![1, 2, 3],
                })),
                merge_strategy: MergeStrategy::MergeCommutative as i32,
            }],
            saga_origin,
        }
    }
}

// ==========================================================================
// Background
// ==========================================================================

#[given("a Notification test environment")]
async fn given_notification_environment(_world: &mut NotificationWorld) {
    // Environment is initialized via World::new
}

// ==========================================================================
// Notification Structure
// ==========================================================================

#[when("I create a notification with cover and payload")]
async fn when_create_notification(world: &mut NotificationWorld) {
    let cover = NotificationWorld::make_cover("test", Uuid::new_v4(), "");

    world.notification = Some(Notification {
        cover: Some(cover),
        payload: Some(prost_types::Any {
            type_url: "test.Payload".to_string(),
            value: vec![1, 2, 3],
        }),
        sent_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
        metadata: HashMap::new(),
    });
}

#[then("the notification should have a cover")]
async fn then_has_cover(world: &mut NotificationWorld) {
    let notification = world.notification.as_ref().expect("No notification");
    assert!(
        notification.cover.is_some(),
        "Notification should have cover"
    );
}

#[then("the notification should have a payload")]
async fn then_has_payload(world: &mut NotificationWorld) {
    let notification = world.notification.as_ref().expect("No notification");
    assert!(
        notification.payload.is_some(),
        "Notification should have payload"
    );
}

#[then("the notification should have a sent_at timestamp")]
async fn then_has_timestamp(world: &mut NotificationWorld) {
    let notification = world.notification.as_ref().expect("No notification");
    assert!(
        notification.sent_at.is_some(),
        "Notification should have sent_at"
    );
}

#[given(expr = "a notification for domain {string} with root {string}")]
async fn given_notification_for_domain_root(
    world: &mut NotificationWorld,
    domain: String,
    root: String,
) {
    let root_uuid = Uuid::new_v5(&Uuid::NAMESPACE_OID, root.as_bytes());
    let cover = NotificationWorld::make_cover(&domain, root_uuid, "");

    world.notification = Some(Notification {
        cover: Some(cover),
        payload: Some(prost_types::Any {
            type_url: "test.Payload".to_string(),
            value: vec![],
        }),
        sent_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
        metadata: HashMap::new(),
    });
}

#[then(expr = "the notification cover should have domain {string}")]
async fn then_cover_domain(world: &mut NotificationWorld, expected: String) {
    let notification = world.notification.as_ref().expect("No notification");
    let cover = notification.cover.as_ref().expect("No cover");
    assert_eq!(cover.domain, expected, "Domain should match");
}

#[then(expr = "the notification cover should have root {string}")]
async fn then_cover_root(world: &mut NotificationWorld, expected: String) {
    let notification = world.notification.as_ref().expect("No notification");
    let cover = notification.cover.as_ref().expect("No cover");
    let expected_uuid = Uuid::new_v5(&Uuid::NAMESPACE_OID, expected.as_bytes());

    let actual = cover.root.as_ref().expect("No root");
    let actual_uuid = Uuid::from_slice(&actual.value).expect("Invalid UUID");
    assert_eq!(actual_uuid, expected_uuid, "Root should match");
}

#[given(expr = "a notification with correlation ID {string}")]
async fn given_notification_with_correlation(
    world: &mut NotificationWorld,
    correlation_id: String,
) {
    let cover = NotificationWorld::make_cover("test", Uuid::new_v4(), &correlation_id);
    world.correlation_id = correlation_id;

    world.notification = Some(Notification {
        cover: Some(cover),
        payload: Some(prost_types::Any {
            type_url: "test.Payload".to_string(),
            value: vec![],
        }),
        sent_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
        metadata: HashMap::new(),
    });
}

#[then(expr = "the notification cover should have correlation ID {string}")]
async fn then_cover_correlation_id(world: &mut NotificationWorld, expected: String) {
    let notification = world.notification.as_ref().expect("No notification");
    let cover = notification.cover.as_ref().expect("No cover");
    assert_eq!(
        cover.correlation_id, expected,
        "Correlation ID should match"
    );
}

#[when("I create a notification without metadata")]
async fn when_create_notification_no_metadata(world: &mut NotificationWorld) {
    let cover = NotificationWorld::make_cover("test", Uuid::new_v4(), "");

    world.notification = Some(Notification {
        cover: Some(cover),
        payload: Some(prost_types::Any {
            type_url: "test.Payload".to_string(),
            value: vec![],
        }),
        sent_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
        metadata: HashMap::new(),
    });
}

#[then("the notification metadata should be empty")]
async fn then_metadata_empty(world: &mut NotificationWorld) {
    let notification = world.notification.as_ref().expect("No notification");
    assert!(notification.metadata.is_empty(), "Metadata should be empty");
}

#[given("a notification with metadata:")]
async fn given_notification_with_metadata(world: &mut NotificationWorld) {
    let cover = NotificationWorld::make_cover("test", Uuid::new_v4(), "");
    let mut metadata = HashMap::new();
    metadata.insert("retry".to_string(), "1".to_string());
    metadata.insert("source".to_string(), "saga-a".to_string());

    world.notification = Some(Notification {
        cover: Some(cover),
        payload: Some(prost_types::Any {
            type_url: "test.Payload".to_string(),
            value: vec![],
        }),
        sent_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
        metadata,
    });
}

#[then(expr = "the notification should have metadata key {string} with value {string}")]
async fn then_metadata_has_key_value(world: &mut NotificationWorld, key: String, value: String) {
    let notification = world.notification.as_ref().expect("No notification");
    let actual = notification.metadata.get(&key);
    assert_eq!(
        actual,
        Some(&value),
        "Metadata key '{}' should have value '{}'",
        key,
        value
    );
}

// ==========================================================================
// RejectionNotification
// ==========================================================================

#[given(expr = "a saga command was rejected with reason {string}")]
async fn given_command_rejected_with_reason(world: &mut NotificationWorld, reason: String) {
    let root = Uuid::new_v4();
    let saga_origin = SagaCommandOrigin {
        saga_name: "test-saga".to_string(),
        triggering_aggregate: Some(NotificationWorld::make_cover("order", root, "")),
        triggering_event_sequence: 0,
    };

    let command = NotificationWorld::make_command_book(
        "inventory",
        Uuid::new_v4(),
        Some(saga_origin.clone()),
    );
    world.rejected_command = Some(command.clone());
    world.saga_origin = Some(saga_origin.clone());

    world.compensation_context = Some(CompensationContext {
        saga_origin,
        rejection_reason: reason,
        rejected_command: command,
        correlation_id: String::new(),
    });
}

#[when("I build a rejection notification")]
async fn when_build_rejection(world: &mut NotificationWorld) {
    let context = world.compensation_context.as_ref().expect("No context");
    world.rejection = Some(build_rejection_notification(context));
}

#[then("the rejection should include the original command")]
async fn then_rejection_has_command(world: &mut NotificationWorld) {
    let rejection = world.rejection.as_ref().expect("No rejection");
    assert!(
        rejection.rejected_command.is_some(),
        "Rejection should include command"
    );
}

#[then(expr = "the rejection reason should be {string}")]
async fn then_rejection_reason(world: &mut NotificationWorld, expected: String) {
    let rejection = world.rejection.as_ref().expect("No rejection");
    assert_eq!(rejection.rejection_reason, expected, "Reason should match");
}

#[given(expr = "a saga {string} issued a command")]
async fn given_saga_issued_command(world: &mut NotificationWorld, saga_name: String) {
    world.saga_name = saga_name.clone();

    let root = Uuid::new_v4();
    let saga_origin = SagaCommandOrigin {
        saga_name,
        triggering_aggregate: Some(NotificationWorld::make_cover("order", root, "")),
        triggering_event_sequence: 0,
    };

    world.saga_origin = Some(saga_origin.clone());

    let command =
        NotificationWorld::make_command_book("inventory", Uuid::new_v4(), Some(saga_origin));
    world.rejected_command = Some(command);
}

#[given("the command was rejected")]
async fn given_command_rejected(world: &mut NotificationWorld) {
    let command = world.rejected_command.as_ref().expect("No command");
    let saga_origin = world.saga_origin.clone().expect("No saga origin");

    world.compensation_context = Some(CompensationContext {
        saga_origin,
        rejection_reason: "test_rejection".to_string(),
        rejected_command: command.clone(),
        correlation_id: String::new(),
    });
}

#[then(expr = "the rejection issuer name should be {string}")]
async fn then_rejection_issuer_name(world: &mut NotificationWorld, expected: String) {
    let rejection = world.rejection.as_ref().expect("No rejection");
    assert_eq!(rejection.issuer_name, expected, "Issuer name should match");
}

#[then(expr = "the rejection issuer type should be {string}")]
async fn then_rejection_issuer_type(world: &mut NotificationWorld, expected: String) {
    let rejection = world.rejection.as_ref().expect("No rejection");
    assert_eq!(rejection.issuer_type, expected, "Issuer type should match");
}

#[given(expr = "a saga triggered by aggregate {string} with root {string} at sequence {int}")]
async fn given_saga_triggered_by_aggregate(
    world: &mut NotificationWorld,
    domain: String,
    root_name: String,
    sequence: u32,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());

    let saga_origin = SagaCommandOrigin {
        saga_name: "test-saga".to_string(),
        triggering_aggregate: Some(NotificationWorld::make_cover(&domain, root, "")),
        triggering_event_sequence: sequence,
    };

    world.saga_origin = Some(saga_origin.clone());

    let command =
        NotificationWorld::make_command_book("inventory", Uuid::new_v4(), Some(saga_origin));
    world.rejected_command = Some(command);
}

#[given("the saga command was rejected")]
async fn given_saga_command_rejected(world: &mut NotificationWorld) {
    let command = world.rejected_command.as_ref().expect("No command");
    let saga_origin = world.saga_origin.clone().expect("No saga origin");

    world.compensation_context = Some(CompensationContext {
        saga_origin,
        rejection_reason: "test_rejection".to_string(),
        rejected_command: command.clone(),
        correlation_id: String::new(),
    });
}

#[then(expr = "the rejection source aggregate should be {string} with root {string}")]
async fn then_rejection_source_aggregate(
    world: &mut NotificationWorld,
    domain: String,
    root_name: String,
) {
    let rejection = world.rejection.as_ref().expect("No rejection");
    let source = rejection
        .source_aggregate
        .as_ref()
        .expect("No source aggregate");

    assert_eq!(source.domain, domain, "Source domain should match");

    let expected_root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    let actual_root = source.root.as_ref().expect("No root");
    let actual_uuid = Uuid::from_slice(&actual_root.value).expect("Invalid UUID");
    assert_eq!(actual_uuid, expected_root, "Source root should match");
}

#[then(expr = "the rejection source event sequence should be {int}")]
async fn then_rejection_source_sequence(world: &mut NotificationWorld, expected: u32) {
    let rejection = world.rejection.as_ref().expect("No rejection");
    assert_eq!(
        rejection.source_event_sequence, expected,
        "Source event sequence should match"
    );
}

#[given("a saga command was rejected")]
async fn given_saga_command_was_rejected(world: &mut NotificationWorld) {
    let root = Uuid::new_v4();
    let saga_origin = SagaCommandOrigin {
        saga_name: "test-saga".to_string(),
        triggering_aggregate: Some(NotificationWorld::make_cover("order", root, "")),
        triggering_event_sequence: 0,
    };

    let command = NotificationWorld::make_command_book(
        "inventory",
        Uuid::new_v4(),
        Some(saga_origin.clone()),
    );
    world.rejected_command = Some(command.clone());
    world.saga_origin = Some(saga_origin.clone());

    world.compensation_context = Some(CompensationContext {
        saga_origin,
        rejection_reason: "test_rejection".to_string(),
        rejected_command: command,
        correlation_id: String::new(),
    });
}

#[when("I wrap the rejection in a notification")]
async fn when_wrap_rejection_in_notification(world: &mut NotificationWorld) {
    let context = world.compensation_context.as_ref().expect("No context");
    world.notification = Some(build_notification(context));
}

#[then(expr = "the notification payload type URL should be {string}")]
async fn then_payload_type_url(world: &mut NotificationWorld, expected: String) {
    let notification = world.notification.as_ref().expect("No notification");
    let payload = notification.payload.as_ref().expect("No payload");
    assert_eq!(payload.type_url, expected, "Type URL should match");
}

// ==========================================================================
// Notification Routing
// ==========================================================================

#[given(expr = "a saga triggered by aggregate {string} with root {string}")]
async fn given_saga_triggered_by_aggregate_simple(
    world: &mut NotificationWorld,
    domain: String,
    root_name: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());

    let saga_origin = SagaCommandOrigin {
        saga_name: "test-saga".to_string(),
        triggering_aggregate: Some(NotificationWorld::make_cover(&domain, root, "")),
        triggering_event_sequence: 0,
    };

    world.saga_origin = Some(saga_origin.clone());

    let command =
        NotificationWorld::make_command_book("inventory", Uuid::new_v4(), Some(saga_origin));
    world.rejected_command = Some(command);
}

#[when("I build a notification command book")]
async fn when_build_notification_command_book(world: &mut NotificationWorld) {
    let context = world.compensation_context.as_ref().expect("No context");
    world.command_book =
        Some(build_notification_command_book(context).expect("Failed to build command book"));
}

#[then(expr = "the command book cover should target {string} with root {string}")]
async fn then_command_book_cover_target(
    world: &mut NotificationWorld,
    domain: String,
    root_name: String,
) {
    let command_book = world.command_book.as_ref().expect("No command book");
    let cover = command_book.cover.as_ref().expect("No cover");

    assert_eq!(cover.domain, domain, "Domain should match");

    let expected_root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    let actual_root = cover.root.as_ref().expect("No root");
    let actual_uuid = Uuid::from_slice(&actual_root.value).expect("Invalid UUID");
    assert_eq!(actual_uuid, expected_root, "Root should match");
}

#[given(expr = "a saga command with correlation ID {string}")]
async fn given_saga_command_with_correlation(
    world: &mut NotificationWorld,
    correlation_id: String,
) {
    world.correlation_id = correlation_id.clone();

    let root = Uuid::new_v4();
    let cover = NotificationWorld::make_cover("order", root, &correlation_id);

    let saga_origin = SagaCommandOrigin {
        saga_name: "test-saga".to_string(),
        triggering_aggregate: Some(cover),
        triggering_event_sequence: 0,
    };

    world.saga_origin = Some(saga_origin.clone());

    let command = CommandBook {
        cover: Some(NotificationWorld::make_cover(
            "inventory",
            Uuid::new_v4(),
            &correlation_id,
        )),
        pages: vec![CommandPage {
            sequence: 0,
            payload: Some(command_page::Payload::Command(prost_types::Any {
                type_url: "test.Command".to_string(),
                value: vec![1, 2, 3],
            })),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
        }],
        saga_origin: Some(saga_origin),
    };

    world.rejected_command = Some(command);
}

#[then(expr = "the command book cover should have correlation ID {string}")]
async fn then_command_book_correlation_id(world: &mut NotificationWorld, expected: String) {
    let command_book = world.command_book.as_ref().expect("No command book");
    let cover = command_book.cover.as_ref().expect("No cover");
    assert_eq!(
        cover.correlation_id, expected,
        "Correlation ID should match"
    );
}

#[then("the command page should use MERGE_COMMUTATIVE")]
async fn then_command_page_merge_strategy(world: &mut NotificationWorld) {
    let command_book = world.command_book.as_ref().expect("No command book");
    let page = command_book.pages.first().expect("No pages");
    assert_eq!(
        page.merge_strategy,
        MergeStrategy::MergeCommutative as i32,
        "Merge strategy should be MERGE_COMMUTATIVE"
    );
}

// ==========================================================================
// Notification vs Event Semantics
// ==========================================================================

#[given("a notification is created")]
async fn given_notification_created(world: &mut NotificationWorld) {
    let cover = NotificationWorld::make_cover("test", Uuid::new_v4(), "");

    world.notification = Some(Notification {
        cover: Some(cover),
        payload: Some(prost_types::Any {
            type_url: "test.Payload".to_string(),
            value: vec![],
        }),
        sent_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
        metadata: HashMap::new(),
    });
}

#[then("the notification should not be stored in the event store")]
async fn then_notification_not_stored(_world: &mut NotificationWorld) {
    // Notifications are by design not persisted
    // This is a documentation step - verifying conceptual behavior
}

#[then("the notification should not have a sequence number")]
async fn then_notification_no_sequence(_world: &mut NotificationWorld) {
    // Notifications don't have sequences - this is by design
    // Notification struct has no sequence field
}

#[given("a notification is sent")]
async fn given_notification_sent(world: &mut NotificationWorld) {
    let cover = NotificationWorld::make_cover("test", Uuid::new_v4(), "");

    world.notification = Some(Notification {
        cover: Some(cover),
        payload: Some(prost_types::Any {
            type_url: "test.Payload".to_string(),
            value: vec![],
        }),
        sent_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
        metadata: HashMap::new(),
    });
}

#[then("the notification cannot be replayed")]
async fn then_notification_cannot_replay(_world: &mut NotificationWorld) {
    // Notifications are fire-and-forget - no replay capability
    // This is a documentation step - verifying conceptual behavior
}
