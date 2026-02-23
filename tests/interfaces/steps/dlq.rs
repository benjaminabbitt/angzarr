//! Dead Letter Queue interface step definitions.

use std::time::Duration;

use angzarr::dlq::{AngzarrDeadLetter, DeadLetterPublisher, DlqBackend, DlqConfig};
use angzarr::proto::{
    command_page, event_page, CommandBook, CommandPage, Cover, EventBook, EventPage, MergeStrategy,
    Uuid as ProtoUuid,
};
use cucumber::{given, then, when, World};
use prost_types::Any;
use uuid::Uuid;

use crate::bus_backend::{BusBackend, DlqContext};

/// Test context for DLQ scenarios.
#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct DlqWorld {
    backend: BusBackend,
    context: Option<DlqContext>,

    /// Last created command for testing.
    last_command: Option<CommandBook>,

    /// Last created event book for testing.
    last_events: Option<EventBook>,

    /// Last created dead letter.
    last_dead_letter: Option<AngzarrDeadLetter>,

    /// Received dead letters (for channel backend).
    received_dead_letters: Vec<AngzarrDeadLetter>,

    /// Last publish result.
    last_publish_success: bool,

    /// Last DLQ config created.
    last_config: Option<DlqConfig>,
}

impl DlqWorld {
    fn new() -> Self {
        Self {
            backend: BusBackend::from_env(),
            context: None,
            last_command: None,
            last_events: None,
            last_dead_letter: None,
            received_dead_letters: Vec::new(),
            last_publish_success: false,
            last_config: None,
        }
    }

    fn publisher(&self) -> &dyn DeadLetterPublisher {
        self.context
            .as_ref()
            .expect("DLQ context not initialized")
            .publisher
            .as_ref()
    }

    fn make_command(&self, domain: &str, sequence: u32) -> CommandBook {
        let root = Uuid::new_v4();
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: "test-correlation".to_string(),
                edition: None,
            }),
            pages: vec![CommandPage {
                sequence,
                payload: Some(command_page::Payload::Command(Any {
                    type_url: "type.test/TestCommand".to_string(),
                    value: vec![1, 2, 3],
                })),
                merge_strategy: MergeStrategy::MergeManual as i32,
            }],
            saga_origin: None,
        }
    }

    fn make_command_with_correlation(&self, domain: &str, correlation_id: &str) -> CommandBook {
        let root = Uuid::new_v4();
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: correlation_id.to_string(),
                edition: None,
            }),
            pages: vec![CommandPage {
                sequence: 0,
                payload: Some(command_page::Payload::Command(Any {
                    type_url: "type.test/TestCommand".to_string(),
                    value: vec![1, 2, 3],
                })),
                merge_strategy: MergeStrategy::MergeManual as i32,
            }],
            saga_origin: None,
        }
    }

    fn make_event_book(&self, domain: &str) -> EventBook {
        let root = Uuid::new_v4();
        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: "test-correlation".to_string(),
                edition: None,
            }),
            snapshot: None,
            pages: vec![EventPage {
                sequence: 0,
                created_at: None,
                payload: Some(event_page::Payload::Event(Any {
                    type_url: "type.test/TestEvent".to_string(),
                    value: vec![1, 2, 3],
                })),
            }],
            next_sequence: 1,
        }
    }

    async fn receive_dead_letters(&mut self) {
        if let Some(ref mut ctx) = self.context {
            if let Some(ref mut rx) = ctx.receiver {
                // Drain all available dead letters
                while let Ok(dl) = rx.try_recv() {
                    self.received_dead_letters.push(dl);
                }
            }
        }
    }
}

// =============================================================================
// Background
// =============================================================================

#[given("a DLQ publisher")]
async fn given_dlq_publisher(world: &mut DlqWorld) {
    println!("Using backend: {}", world.backend.name());
    let ctx = DlqContext::new(world.backend).await;
    world.context = Some(ctx);
}

// =============================================================================
// Basic Publishing - Sequence Mismatch
// =============================================================================

#[given(expr = "a command with sequence {int} for domain {string}")]
async fn given_command_with_sequence(world: &mut DlqWorld, sequence: u32, domain: String) {
    let cmd = world.make_command(&domain, sequence);
    world.last_command = Some(cmd);
}

#[when(expr = "the aggregate rejects it with actual sequence {int}")]
async fn when_aggregate_rejects_with_actual(world: &mut DlqWorld, actual_sequence: u32) {
    let cmd = world.last_command.as_ref().expect("No command created");
    let expected_sequence = cmd.pages.first().map(|p| p.sequence).unwrap_or(0);

    let dead_letter = AngzarrDeadLetter::from_sequence_mismatch(
        cmd,
        expected_sequence,
        actual_sequence,
        MergeStrategy::MergeManual,
        "test-aggregate",
    );

    world.last_dead_letter = Some(dead_letter);
}

#[when("the dead letter is published to DLQ")]
async fn when_dead_letter_published_to_dlq(world: &mut DlqWorld) {
    let dl = world
        .last_dead_letter
        .take()
        .expect("No dead letter created");

    match world.publisher().publish(dl.clone()).await {
        Ok(_) => {
            world.last_publish_success = true;
            world.last_dead_letter = Some(dl);
        }
        Err(_) => {
            world.last_publish_success = false;
            world.last_dead_letter = Some(dl);
        }
    }

    // Give time for message to arrive at receiver
    tokio::time::sleep(Duration::from_millis(100)).await;
    world.receive_dead_letters().await;
}

#[then(expr = "the DLQ receives a message with reason {string}")]
async fn then_dlq_receives_message_with_reason(world: &mut DlqWorld, reason: String) {
    // For channel backend, check received_dead_letters
    // For other backends, we rely on publish success
    if !world.received_dead_letters.is_empty() {
        let dl = world
            .received_dead_letters
            .first()
            .expect("No dead letters");
        assert!(
            dl.rejection_reason.contains(&reason),
            "Expected reason containing '{}', got '{}'",
            reason,
            dl.rejection_reason
        );
    } else {
        // For non-channel backends, verify the dead letter was created correctly
        let dl = world.last_dead_letter.as_ref().expect("No dead letter");
        assert!(
            dl.rejection_reason.contains(&reason),
            "Expected reason containing '{}', got '{}'",
            reason,
            dl.rejection_reason
        );
    }
}

#[then("the payload contains the original command")]
async fn then_payload_contains_original_command(world: &mut DlqWorld) {
    let dl = if !world.received_dead_letters.is_empty() {
        world.received_dead_letters.first().unwrap()
    } else {
        world.last_dead_letter.as_ref().expect("No dead letter")
    };

    match &dl.payload {
        angzarr::dlq::DeadLetterPayload::Command(cmd) => {
            assert!(cmd.cover.is_some(), "Command should have cover");
        }
        _ => panic!("Expected Command payload, got Events"),
    }
}

#[then(expr = "the rejection details show expected {int} and actual {int}")]
async fn then_rejection_details_show_sequences(world: &mut DlqWorld, expected: u32, actual: u32) {
    let dl = if !world.received_dead_letters.is_empty() {
        world.received_dead_letters.first().unwrap()
    } else {
        world.last_dead_letter.as_ref().expect("No dead letter")
    };

    match &dl.rejection_details {
        Some(angzarr::dlq::RejectionDetails::SequenceMismatch(details)) => {
            assert_eq!(details.expected_sequence, expected);
            assert_eq!(details.actual_sequence, actual);
        }
        _ => panic!("Expected SequenceMismatch rejection details"),
    }
}

// =============================================================================
// Basic Publishing - Handler Failure
// =============================================================================

#[given(expr = "an event book for domain {string}")]
async fn given_event_book_for_domain(world: &mut DlqWorld, domain: String) {
    let events = world.make_event_book(&domain);
    world.last_events = Some(events);
}

#[when(expr = "the saga handler fails with error {string}")]
async fn when_saga_handler_fails(world: &mut DlqWorld, error: String) {
    let events = world.last_events.as_ref().expect("No events created");

    let dead_letter = AngzarrDeadLetter::from_event_processing_failure(
        events,
        &error,
        1,     // retry_count
        false, // is_transient
        "test-saga",
        "saga",
    );

    world.last_dead_letter = Some(dead_letter);
}

#[then(expr = "the DLQ receives a message with reason containing {string}")]
async fn then_dlq_receives_message_containing(world: &mut DlqWorld, substring: String) {
    let dl = if !world.received_dead_letters.is_empty() {
        world.received_dead_letters.first().unwrap()
    } else {
        world.last_dead_letter.as_ref().expect("No dead letter")
    };

    assert!(
        dl.rejection_reason.contains(&substring),
        "Expected reason containing '{}', got '{}'",
        substring,
        dl.rejection_reason
    );
}

#[then("the payload contains the original events")]
async fn then_payload_contains_original_events(world: &mut DlqWorld) {
    let dl = if !world.received_dead_letters.is_empty() {
        world.received_dead_letters.first().unwrap()
    } else {
        world.last_dead_letter.as_ref().expect("No dead letter")
    };

    match &dl.payload {
        angzarr::dlq::DeadLetterPayload::Events(events) => {
            assert!(events.cover.is_some(), "Events should have cover");
        }
        _ => panic!("Expected Events payload, got Command"),
    }
}

#[then(expr = "the source component type is {string}")]
async fn then_source_component_type_is(world: &mut DlqWorld, expected_type: String) {
    let dl = if !world.received_dead_letters.is_empty() {
        world.received_dead_letters.first().unwrap()
    } else {
        world.last_dead_letter.as_ref().expect("No dead letter")
    };

    assert_eq!(dl.source_component_type, expected_type);
}

// =============================================================================
// Basic Publishing - Payload Retrieval Failure
// =============================================================================

#[given("an event book with external payload reference")]
async fn given_event_book_with_external_payload(world: &mut DlqWorld) {
    let events = world.make_event_book("payload-test");
    world.last_events = Some(events);
}

#[when(expr = "the payload retrieval fails from {string} with error {string}")]
async fn when_payload_retrieval_fails(world: &mut DlqWorld, storage_type: String, error: String) {
    let events = world.last_events.as_ref().expect("No events created");

    let dead_letter = AngzarrDeadLetter::from_payload_retrieval_failure(
        events,
        &storage_type,
        "gs://test-bucket/payload.bin",
        &[0xab, 0xcd, 0xef],
        1024,
        &error,
        "offloading-bus",
    );

    world.last_dead_letter = Some(dead_letter);
}

#[then(expr = "the rejection details show storage type {string}")]
async fn then_rejection_details_show_storage_type(world: &mut DlqWorld, expected_type: String) {
    let dl = if !world.received_dead_letters.is_empty() {
        world.received_dead_letters.first().unwrap()
    } else {
        world.last_dead_letter.as_ref().expect("No dead letter")
    };

    match &dl.rejection_details {
        Some(angzarr::dlq::RejectionDetails::PayloadRetrievalFailed(details)) => {
            assert_eq!(details.storage_type, expected_type);
        }
        _ => panic!("Expected PayloadRetrievalFailed rejection details"),
    }
}

// =============================================================================
// Topic Routing
// =============================================================================

#[given(expr = "a command for domain {string}")]
async fn given_command_for_domain(world: &mut DlqWorld, domain: String) {
    let cmd = world.make_command(&domain, 0);
    world.last_command = Some(cmd);

    // Create dead letter immediately
    let cmd = world.last_command.as_ref().unwrap();
    let dead_letter = AngzarrDeadLetter::from_sequence_mismatch(
        cmd,
        0,
        5,
        MergeStrategy::MergeManual,
        "test-aggregate",
    );
    world.last_dead_letter = Some(dead_letter);
}

#[when("the dead letter is published")]
async fn when_dead_letter_is_published(world: &mut DlqWorld) {
    when_dead_letter_published_to_dlq(world).await;
}

#[then(expr = "it is published to topic {string}")]
async fn then_published_to_topic(world: &mut DlqWorld, expected_topic: String) {
    let dl = world.last_dead_letter.as_ref().expect("No dead letter");
    let actual_topic = dl.topic();
    assert_eq!(actual_topic, expected_topic);
}

#[given(expr = "a command with correlation ID {string}")]
async fn given_command_with_correlation_id(world: &mut DlqWorld, correlation_id: String) {
    let cmd = world.make_command_with_correlation("test", &correlation_id);
    world.last_command = Some(cmd);

    // Create dead letter
    let cmd = world.last_command.as_ref().unwrap();
    let dead_letter = AngzarrDeadLetter::from_sequence_mismatch(
        cmd,
        0,
        5,
        MergeStrategy::MergeManual,
        "test-aggregate",
    );
    world.last_dead_letter = Some(dead_letter);
}

#[then(expr = "the DLQ message has correlation ID {string}")]
async fn then_dlq_message_has_correlation_id(world: &mut DlqWorld, expected_id: String) {
    let dl = if !world.received_dead_letters.is_empty() {
        world.received_dead_letters.first().unwrap()
    } else {
        world.last_dead_letter.as_ref().expect("No dead letter")
    };

    if let Some(cover) = &dl.cover {
        assert_eq!(cover.correlation_id, expected_id);
    } else {
        panic!("Dead letter has no cover");
    }
}

// =============================================================================
// Metadata
// =============================================================================

#[when("a dead letter is published")]
async fn when_a_dead_letter_is_published(world: &mut DlqWorld) {
    // Create a simple dead letter
    let cmd = world.make_command("metadata-test", 0);
    let dead_letter = AngzarrDeadLetter::from_sequence_mismatch(
        &cmd,
        0,
        5,
        MergeStrategy::MergeManual,
        "test-aggregate",
    );
    world.last_dead_letter = Some(dead_letter.clone());

    match world.publisher().publish(dead_letter).await {
        Ok(_) => world.last_publish_success = true,
        Err(_) => world.last_publish_success = false,
    }

    tokio::time::sleep(Duration::from_millis(100)).await;
    world.receive_dead_letters().await;
}

#[then("the occurred_at timestamp is within the last minute")]
async fn then_timestamp_is_recent(world: &mut DlqWorld) {
    let dl = if !world.received_dead_letters.is_empty() {
        world.received_dead_letters.first().unwrap()
    } else {
        world.last_dead_letter.as_ref().expect("No dead letter")
    };

    assert!(
        dl.occurred_at.is_some(),
        "Dead letter should have timestamp"
    );

    if let Some(ts) = &dl.occurred_at {
        let now = std::time::SystemTime::now();
        let ts_system =
            std::time::UNIX_EPOCH + std::time::Duration::new(ts.seconds as u64, ts.nanos as u32);
        let diff = now
            .duration_since(ts_system)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0));
        assert!(
            diff < std::time::Duration::from_secs(60),
            "Timestamp should be within last minute"
        );
    }
}

#[given("a command that fails")]
async fn given_command_that_fails(world: &mut DlqWorld) {
    let cmd = world.make_command("metadata-test", 0);
    world.last_command = Some(cmd);

    let cmd = world.last_command.as_ref().unwrap();
    let dead_letter = AngzarrDeadLetter::from_sequence_mismatch(
        cmd,
        0,
        5,
        MergeStrategy::MergeManual,
        "test-aggregate",
    );
    world.last_dead_letter = Some(dead_letter);
}

#[when(expr = "metadata {string} = {string} is added")]
async fn when_metadata_is_added(world: &mut DlqWorld, key: String, value: String) {
    let dl = world.last_dead_letter.take().expect("No dead letter");
    let dl = dl.with_metadata(&key, &value);
    world.last_dead_letter = Some(dl);
}

#[then(expr = "the DLQ message metadata contains {string} = {string}")]
async fn then_metadata_contains(world: &mut DlqWorld, key: String, value: String) {
    let dl = if !world.received_dead_letters.is_empty() {
        world.received_dead_letters.first().unwrap()
    } else {
        world.last_dead_letter.as_ref().expect("No dead letter")
    };

    assert_eq!(
        dl.metadata.get(&key),
        Some(&value),
        "Expected metadata {} = {}",
        key,
        value
    );
}

#[given(expr = "a dead letter from component {string} of type {string}")]
async fn given_dead_letter_from_component(
    world: &mut DlqWorld,
    component: String,
    component_type: String,
) {
    let events = world.make_event_book("source-test");
    let dead_letter = AngzarrDeadLetter::from_event_processing_failure(
        &events,
        "test error",
        1,
        false,
        &component,
        &component_type,
    );
    world.last_dead_letter = Some(dead_letter);
}

#[when("it is published")]
async fn when_it_is_published(world: &mut DlqWorld) {
    when_dead_letter_published_to_dlq(world).await;
}

#[then(expr = "the DLQ message shows source {string}")]
async fn then_dlq_message_shows_source(world: &mut DlqWorld, expected_source: String) {
    let dl = if !world.received_dead_letters.is_empty() {
        world.received_dead_letters.first().unwrap()
    } else {
        world.last_dead_letter.as_ref().expect("No dead letter")
    };

    assert_eq!(dl.source_component, expected_source);
}

#[then(expr = "the DLQ message shows source type {string}")]
async fn then_dlq_message_shows_source_type(world: &mut DlqWorld, expected_type: String) {
    let dl = if !world.received_dead_letters.is_empty() {
        world.received_dead_letters.first().unwrap()
    } else {
        world.last_dead_letter.as_ref().expect("No dead letter")
    };

    assert_eq!(dl.source_component_type, expected_type);
}

// =============================================================================
// Channel Backend
// =============================================================================

#[given("a channel DLQ publisher and receiver")]
async fn given_channel_dlq_publisher_and_receiver(world: &mut DlqWorld) {
    // Force channel backend
    world.backend = BusBackend::Channel;
    let ctx = DlqContext::new(world.backend).await;
    world.context = Some(ctx);
}

#[then("the receiver receives the dead letter")]
async fn then_receiver_receives_dead_letter(world: &mut DlqWorld) {
    world.receive_dead_letters().await;
    assert!(
        !world.received_dead_letters.is_empty(),
        "Receiver should have received at least one dead letter"
    );
}

#[then("the payload is intact")]
async fn then_payload_is_intact(world: &mut DlqWorld) {
    let dl = world
        .received_dead_letters
        .first()
        .expect("No dead letters received");

    // Verify payload exists
    match &dl.payload {
        angzarr::dlq::DeadLetterPayload::Command(cmd) => {
            assert!(cmd.cover.is_some());
        }
        angzarr::dlq::DeadLetterPayload::Events(events) => {
            assert!(events.cover.is_some());
        }
    }
}

#[when(expr = "{int} dead letters are published")]
async fn when_multiple_dead_letters_published(world: &mut DlqWorld, count: usize) {
    for i in 0..count {
        let cmd = world.make_command(&format!("batch-{}", i), 0);
        let dead_letter = AngzarrDeadLetter::from_sequence_mismatch(
            &cmd,
            0,
            5,
            MergeStrategy::MergeManual,
            &format!("aggregate-{}", i),
        );

        world
            .publisher()
            .publish(dead_letter)
            .await
            .expect("Failed to publish");
    }

    tokio::time::sleep(Duration::from_millis(100)).await;
    world.receive_dead_letters().await;
}

#[then(expr = "the receiver receives all {int} dead letters in order")]
async fn then_receiver_receives_all_in_order(world: &mut DlqWorld, count: usize) {
    assert_eq!(
        world.received_dead_letters.len(),
        count,
        "Expected {} dead letters, got {}",
        count,
        world.received_dead_letters.len()
    );

    // Verify order by checking source component names
    for (i, dl) in world.received_dead_letters.iter().enumerate() {
        assert_eq!(
            dl.source_component,
            format!("aggregate-{}", i),
            "Dead letters should be in order"
        );
    }
}

// =============================================================================
// Noop Backend
// =============================================================================

#[given("a noop DLQ publisher")]
async fn given_noop_dlq_publisher(world: &mut DlqWorld) {
    // Create a minimal context with noop publisher
    world.context = Some(DlqContext::noop());
}

#[then("publish succeeds")]
async fn then_publish_succeeds(world: &mut DlqWorld) {
    assert!(world.last_publish_success, "Publish should have succeeded");
}

#[then("is_configured returns false")]
async fn then_is_configured_returns_false(world: &mut DlqWorld) {
    let is_configured = world.publisher().is_configured();
    assert!(!is_configured, "Noop publisher should not be configured");
}

// =============================================================================
// Configuration
// =============================================================================

#[when(regex = r"DlqConfig::channel\(\) is created")]
async fn when_dlq_config_channel_created(world: &mut DlqWorld) {
    world.last_config = Some(DlqConfig::channel());
}

#[then("the backend is Channel")]
async fn then_backend_is_channel(world: &mut DlqWorld) {
    let config = world.last_config.as_ref().expect("No config created");
    assert_eq!(config.backend, DlqBackend::Channel);
}

#[then("is_configured returns true")]
async fn then_is_configured_returns_true(world: &mut DlqWorld) {
    let config = world.last_config.as_ref().expect("No config created");
    assert!(config.is_configured());
}

#[when(regex = r#"DlqConfig::amqp\("(.+)"\) is created"#)]
async fn when_dlq_config_amqp_created(world: &mut DlqWorld, url: String) {
    world.last_config = Some(DlqConfig::amqp(&url));
}

#[then("the backend is Amqp")]
async fn then_backend_is_amqp(world: &mut DlqWorld) {
    let config = world.last_config.as_ref().expect("No config created");
    assert_eq!(config.backend, DlqBackend::Amqp);
}

#[then(expr = "amqp_url is {string}")]
async fn then_amqp_url_is(world: &mut DlqWorld, expected_url: String) {
    let config = world.last_config.as_ref().expect("No config created");
    assert_eq!(config.amqp_url, Some(expected_url));
}

#[when(regex = r#"DlqConfig::kafka\("(.+)"\) is created"#)]
async fn when_dlq_config_kafka_created(world: &mut DlqWorld, brokers: String) {
    world.last_config = Some(DlqConfig::kafka(&brokers));
}

#[then("the backend is Kafka")]
async fn then_backend_is_kafka(world: &mut DlqWorld) {
    let config = world.last_config.as_ref().expect("No config created");
    assert_eq!(config.backend, DlqBackend::Kafka);
}

#[then(expr = "kafka_brokers is {string}")]
async fn then_kafka_brokers_is(world: &mut DlqWorld, expected_brokers: String) {
    let config = world.last_config.as_ref().expect("No config created");
    assert_eq!(config.kafka_brokers, Some(expected_brokers));
}

#[when(regex = r"DlqConfig::pubsub\(\) is created")]
async fn when_dlq_config_pubsub_created(world: &mut DlqWorld) {
    world.last_config = Some(DlqConfig::pubsub());
}

#[then("the backend is PubSub")]
async fn then_backend_is_pubsub(world: &mut DlqWorld) {
    let config = world.last_config.as_ref().expect("No config created");
    assert_eq!(config.backend, DlqBackend::PubSub);
}

#[when(regex = r#"DlqConfig::sns_sqs\(\).with_aws_region\("(.+)"\) is created"#)]
async fn when_dlq_config_sns_sqs_created(world: &mut DlqWorld, region: String) {
    world.last_config = Some(DlqConfig::sns_sqs().with_aws_region(&region));
}

#[then("the backend is SnsSqs")]
async fn then_backend_is_sns_sqs(world: &mut DlqWorld) {
    let config = world.last_config.as_ref().expect("No config created");
    assert_eq!(config.backend, DlqBackend::SnsSqs);
}

#[then(expr = "aws_region is {string}")]
async fn then_aws_region_is(world: &mut DlqWorld, expected_region: String) {
    let config = world.last_config.as_ref().expect("No config created");
    assert_eq!(config.aws_region, Some(expected_region));
}
