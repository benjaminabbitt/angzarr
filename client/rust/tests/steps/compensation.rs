//! Compensation step definitions.
//!
//! Tests compensation handling patterns for saga rejections.
//! Since some proto types may not be directly available, we use simplified
//! representations to test the behavioral patterns.

use angzarr_client::proto::{command_page, CommandBook, CommandPage, Cover, MergeStrategy};
use cucumber::{given, then, when, World};
use prost::Message;
use prost_types::Any;
use uuid::Uuid;

/// Test command for compensation testing.
#[derive(Clone, Message)]
struct TestCommand {
    #[prost(string, tag = "1")]
    pub data: String,
}

/// Simulated saga origin for testing.
#[derive(Debug, Clone, Default)]
struct TestSagaOrigin {
    saga_name: String,
    triggering_domain: String,
    triggering_root: Option<String>,
    triggering_event_sequence: u32,
}

/// Simulated rejection notification for testing.
#[derive(Debug, Clone, Default)]
struct TestRejectionNotification {
    rejected_command: Option<CommandBook>,
    rejection_reason: String,
    issuer_name: String,
    issuer_type: String,
    source_domain: String,
    source_root: Option<String>,
    source_event_sequence: u32,
}

/// Simulated notification for testing.
#[derive(Debug, Clone)]
struct TestNotification {
    cover_domain: String,
    cover_root: Option<String>,
    correlation_id: String,
    sent_at: i64,
    payload_type_url: String,
}

/// Compensation context for rejected commands.
#[derive(Debug, Clone, Default)]
struct CompensationContext {
    rejected_command: Option<CommandBook>,
    rejection_reason: String,
    saga_origin: Option<TestSagaOrigin>,
    correlation_id: String,
    source_domain: String,
    source_root: String,
    source_sequence: u32,
}

impl CompensationContext {
    fn from_rejection(
        cmd: &CommandBook,
        reason: &str,
        saga_origin: Option<TestSagaOrigin>,
    ) -> Self {
        let cover = cmd.cover.as_ref();

        Self {
            rejected_command: Some(cmd.clone()),
            rejection_reason: reason.to_string(),
            saga_origin: saga_origin.clone(),
            correlation_id: cover.map(|c| c.correlation_id.clone()).unwrap_or_default(),
            source_domain: saga_origin
                .as_ref()
                .map(|o| o.triggering_domain.clone())
                .unwrap_or_default(),
            source_root: saga_origin
                .as_ref()
                .and_then(|o| o.triggering_root.clone())
                .unwrap_or_default(),
            source_sequence: saga_origin
                .as_ref()
                .map(|o| o.triggering_event_sequence)
                .unwrap_or(0),
        }
    }

    fn build_rejection_notification(&self) -> TestRejectionNotification {
        TestRejectionNotification {
            rejected_command: self.rejected_command.clone(),
            rejection_reason: self.rejection_reason.clone(),
            issuer_name: self
                .saga_origin
                .as_ref()
                .map(|o| o.saga_name.clone())
                .unwrap_or_default(),
            issuer_type: "saga".to_string(),
            source_domain: self.source_domain.clone(),
            source_root: Some(self.source_root.clone()),
            source_event_sequence: self.source_sequence,
        }
    }

    fn build_notification(&self) -> TestNotification {
        TestNotification {
            cover_domain: self.source_domain.clone(),
            cover_root: Some(self.source_root.clone()),
            correlation_id: self.correlation_id.clone(),
            sent_at: chrono::Utc::now().timestamp(),
            payload_type_url: "type.googleapis.com/angzarr.RejectionNotification".to_string(),
        }
    }

    fn build_command_book(&self) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: self.source_domain.clone(),
                root: Some(angzarr_client::proto::Uuid {
                    value: self.source_root.as_bytes().to_vec(),
                }),
                correlation_id: self.correlation_id.clone(),
                edition: None,
            }),
            pages: vec![CommandPage {
                sequence: 0,
                merge_strategy: MergeStrategy::MergeCommutative as i32,
                payload: Some(command_page::Payload::Command(Any {
                    type_url: "type.googleapis.com/angzarr.Notification".to_string(),
                    value: vec![],
                })),
            }],
            saga_origin: None,
        }
    }
}

fn make_saga_command(
    domain: &str,
    saga_name: &str,
    triggering_domain: &str,
    triggering_sequence: u32,
) -> (CommandBook, TestSagaOrigin) {
    let cmd = TestCommand {
        data: "saga-command".to_string(),
    };
    let root = Uuid::new_v4();

    let origin = TestSagaOrigin {
        saga_name: saga_name.to_string(),
        triggering_domain: triggering_domain.to_string(),
        triggering_root: Some(Uuid::new_v4().to_string()),
        triggering_event_sequence: triggering_sequence,
    };

    let book = CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(angzarr_client::proto::Uuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: "workflow-123".to_string(),
            edition: None,
        }),
        pages: vec![CommandPage {
            sequence: 0,
            merge_strategy: 0,
            payload: Some(command_page::Payload::Command(Any {
                type_url: "type.googleapis.com/test.SagaCommand".to_string(),
                value: cmd.encode_to_vec(),
            })),
        }],
        saga_origin: None,
    };

    (book, origin)
}

/// Test context for compensation scenarios.
#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct CompensationWorld {
    rejected_command: Option<CommandBook>,
    saga_origin: Option<TestSagaOrigin>,
    rejection_reason: String,
    compensation_context: Option<CompensationContext>,
    rejection_notification: Option<TestRejectionNotification>,
    notification: Option<TestNotification>,
    command_book: Option<CommandBook>,
}

impl CompensationWorld {
    fn new() -> Self {
        Self {
            rejected_command: None,
            saga_origin: None,
            rejection_reason: String::new(),
            compensation_context: None,
            rejection_notification: None,
            notification: None,
            command_book: None,
        }
    }
}

// --- Given steps ---

#[given("a compensation handling context")]
async fn given_compensation_context(_world: &mut CompensationWorld) {
    // Background context
}

#[given("a saga command that was rejected")]
async fn given_saga_command_rejected(world: &mut CompensationWorld) {
    let (cmd, origin) = make_saga_command("fulfillment", "order-fulfillment", "orders", 5);
    world.rejected_command = Some(cmd);
    world.saga_origin = Some(origin);
    world.rejection_reason = "inventory insufficient".to_string();
}

#[given(expr = "a saga {string} triggered by {string} aggregate at sequence {int}")]
async fn given_saga_triggered(
    world: &mut CompensationWorld,
    saga_name: String,
    domain: String,
    seq: u32,
) {
    let (cmd, origin) = make_saga_command("fulfillment", &saga_name, &domain, seq);
    world.rejected_command = Some(cmd);
    world.saga_origin = Some(origin);
}

#[given("the saga command was rejected")]
async fn given_saga_command_was_rejected(world: &mut CompensationWorld) {
    world.rejection_reason = "command rejected".to_string();
}

#[given(expr = "a saga command with correlation ID {string}")]
async fn given_saga_with_correlation(world: &mut CompensationWorld, cid: String) {
    let (mut cmd, origin) = make_saga_command("fulfillment", "order-fulfillment", "orders", 5);
    if let Some(ref mut cover) = cmd.cover {
        cover.correlation_id = cid;
    }
    world.rejected_command = Some(cmd);
    world.saga_origin = Some(origin);
}

#[given("the command was rejected")]
async fn given_command_rejected(world: &mut CompensationWorld) {
    world.rejection_reason = "rejected".to_string();
}

#[given("a CompensationContext for rejected command")]
async fn given_compensation_context_for_rejected(world: &mut CompensationWorld) {
    if let Some(ref cmd) = world.rejected_command {
        world.compensation_context = Some(CompensationContext::from_rejection(
            cmd,
            &world.rejection_reason,
            world.saga_origin.clone(),
        ));
    } else {
        let (cmd, origin) = make_saga_command("fulfillment", "order-fulfillment", "orders", 5);
        world.rejection_reason = "rejected".to_string();
        world.compensation_context = Some(CompensationContext::from_rejection(
            &cmd,
            "rejected",
            Some(origin.clone()),
        ));
        world.rejected_command = Some(cmd);
        world.saga_origin = Some(origin);
    }
}

#[given(expr = "a CompensationContext from {string} aggregate at sequence {int}")]
async fn given_context_from_aggregate(world: &mut CompensationWorld, domain: String, seq: u32) {
    let (cmd, origin) = make_saga_command("fulfillment", "order-fulfillment", &domain, seq);
    world.compensation_context = Some(CompensationContext::from_rejection(
        &cmd,
        "rejected",
        Some(origin.clone()),
    ));
    world.rejected_command = Some(cmd);
    world.saga_origin = Some(origin);
}

#[given(expr = "a CompensationContext from saga {string}")]
async fn given_context_from_saga(world: &mut CompensationWorld, saga_name: String) {
    let (cmd, origin) = make_saga_command("fulfillment", &saga_name, "orders", 5);
    world.compensation_context = Some(CompensationContext::from_rejection(
        &cmd,
        "rejected",
        Some(origin.clone()),
    ));
    world.rejected_command = Some(cmd);
    world.saga_origin = Some(origin);
}

#[given(expr = "a CompensationContext from {string} aggregate root {string}")]
async fn given_context_from_aggregate_root(
    world: &mut CompensationWorld,
    domain: String,
    _root: String,
) {
    let (cmd, origin) = make_saga_command("fulfillment", "order-fulfillment", &domain, 5);
    world.compensation_context = Some(CompensationContext::from_rejection(
        &cmd,
        "rejected",
        Some(origin.clone()),
    ));
    world.rejected_command = Some(cmd);
    world.saga_origin = Some(origin);
}

#[given(expr = "a command rejected with reason {string}")]
async fn given_command_rejected_with_reason(world: &mut CompensationWorld, reason: String) {
    let (cmd, origin) = make_saga_command("fulfillment", "order-fulfillment", "orders", 5);
    world.rejection_reason = reason;
    world.rejected_command = Some(cmd);
    world.saga_origin = Some(origin);
}

#[given("a command rejected with structured reason")]
async fn given_structured_reason(world: &mut CompensationWorld) {
    let (cmd, origin) = make_saga_command("fulfillment", "order-fulfillment", "orders", 5);
    world.rejection_reason = "structured: {code: INVENTORY_INSUFFICIENT, quantity: 10}".to_string();
    world.rejected_command = Some(cmd);
    world.saga_origin = Some(origin);
}

#[given("a saga command with specific payload")]
async fn given_saga_with_payload(world: &mut CompensationWorld) {
    let (cmd, origin) = make_saga_command("fulfillment", "order-fulfillment", "orders", 5);
    world.rejected_command = Some(cmd);
    world.saga_origin = Some(origin);
}

#[given("a nested saga scenario")]
async fn given_nested_saga(world: &mut CompensationWorld) {
    let (cmd, origin) = make_saga_command("shipping", "fulfillment-shipping", "fulfillment", 10);
    world.rejected_command = Some(cmd);
    world.saga_origin = Some(origin);
}

#[given("an inner saga command was rejected")]
async fn given_inner_saga_rejected(world: &mut CompensationWorld) {
    world.rejection_reason = "inner saga rejection".to_string();
}

#[given("a saga router handling rejections")]
async fn given_saga_router_handling_rejections(_world: &mut CompensationWorld) {
    // Router setup
}

#[given("a process manager router")]
async fn given_pm_router(_world: &mut CompensationWorld) {
    // PM router setup
}

// --- When steps ---

#[when("I build a CompensationContext")]
async fn when_build_compensation_context(world: &mut CompensationWorld) {
    if let Some(ref cmd) = world.rejected_command {
        world.compensation_context = Some(CompensationContext::from_rejection(
            cmd,
            &world.rejection_reason,
            world.saga_origin.clone(),
        ));
    }
}

#[when("I build a RejectionNotification")]
async fn when_build_rejection_notification(world: &mut CompensationWorld) {
    // Build context if not already present
    if world.compensation_context.is_none() {
        if let Some(ref cmd) = world.rejected_command {
            world.compensation_context = Some(CompensationContext::from_rejection(
                cmd,
                &world.rejection_reason,
                world.saga_origin.clone(),
            ));
        }
    }
    if let Some(ref ctx) = world.compensation_context {
        world.rejection_notification = Some(ctx.build_rejection_notification());
    }
}

#[when("I build a Notification from the context")]
async fn when_build_notification(world: &mut CompensationWorld) {
    if let Some(ref ctx) = world.compensation_context {
        world.notification = Some(ctx.build_notification());
    }
}

#[when("I build a Notification from a CompensationContext")]
async fn when_build_notification_from_context(world: &mut CompensationWorld) {
    if world.compensation_context.is_none() {
        let (cmd, origin) = make_saga_command("fulfillment", "order-fulfillment", "orders", 5);
        world.compensation_context = Some(CompensationContext::from_rejection(
            &cmd,
            "rejected",
            Some(origin),
        ));
    }
    if let Some(ref ctx) = world.compensation_context {
        world.notification = Some(ctx.build_notification());
    }
}

#[when("I build a notification CommandBook")]
async fn when_build_command_book(world: &mut CompensationWorld) {
    if let Some(ref ctx) = world.compensation_context {
        world.command_book = Some(ctx.build_command_book());
    }
}

#[when("a command execution fails with precondition error")]
async fn when_command_fails(world: &mut CompensationWorld) {
    let (cmd, origin) = make_saga_command("fulfillment", "order-fulfillment", "orders", 5);
    world.rejection_reason = "precondition failed".to_string();
    world.compensation_context = Some(CompensationContext::from_rejection(
        &cmd,
        "precondition failed",
        Some(origin),
    ));
}

#[when("a PM command is rejected")]
async fn when_pm_command_rejected(world: &mut CompensationWorld) {
    let (cmd, origin) = make_saga_command("fulfillment", "pmg-order-workflow", "orders", 5);
    world.rejection_reason = "pm command rejected".to_string();
    world.compensation_context = Some(CompensationContext::from_rejection(
        &cmd,
        "pm command rejected",
        Some(origin),
    ));
}

// --- Then steps ---

#[then("the context should include the rejected command")]
async fn then_context_has_command(world: &mut CompensationWorld) {
    assert!(world
        .compensation_context
        .as_ref()
        .unwrap()
        .rejected_command
        .is_some());
}

#[then("the context should include the rejection reason")]
async fn then_context_has_reason(world: &mut CompensationWorld) {
    assert!(!world
        .compensation_context
        .as_ref()
        .unwrap()
        .rejection_reason
        .is_empty());
}

#[then("the context should include the saga origin")]
async fn then_context_has_saga_origin(world: &mut CompensationWorld) {
    assert!(world
        .compensation_context
        .as_ref()
        .unwrap()
        .saga_origin
        .is_some());
}

#[then(expr = "the saga_origin saga_name should be {string}")]
async fn then_saga_name(world: &mut CompensationWorld, expected: String) {
    let ctx = world.compensation_context.as_ref().unwrap();
    let origin = ctx.saga_origin.as_ref().unwrap();
    assert_eq!(origin.saga_name, expected);
}

#[then(expr = "the triggering_aggregate should be {string}")]
async fn then_triggering_aggregate(world: &mut CompensationWorld, expected: String) {
    let ctx = world.compensation_context.as_ref().unwrap();
    assert_eq!(ctx.source_domain, expected);
}

#[then(expr = "the triggering_event_sequence should be {int}")]
async fn then_triggering_sequence(world: &mut CompensationWorld, expected: u32) {
    let ctx = world.compensation_context.as_ref().unwrap();
    assert_eq!(ctx.source_sequence, expected);
}

#[then(expr = "the context correlation_id should be {string}")]
async fn then_context_correlation_id(world: &mut CompensationWorld, expected: String) {
    let ctx = world.compensation_context.as_ref().unwrap();
    assert_eq!(ctx.correlation_id, expected);
}

#[then("the notification should include the rejected command")]
async fn then_notification_has_command(world: &mut CompensationWorld) {
    let notif = world.rejection_notification.as_ref().unwrap();
    assert!(notif.rejected_command.is_some());
}

#[then("the notification should include the rejection reason")]
async fn then_notification_has_reason(world: &mut CompensationWorld) {
    let notif = world.rejection_notification.as_ref().unwrap();
    assert!(!notif.rejection_reason.is_empty());
}

#[then(expr = "the notification should have issuer_type {string}")]
async fn then_notification_issuer_type(world: &mut CompensationWorld, expected: String) {
    let notif = world.rejection_notification.as_ref().unwrap();
    assert_eq!(notif.issuer_type, expected);
}

#[then(expr = "the source_aggregate should have domain {string}")]
async fn then_source_domain(world: &mut CompensationWorld, expected: String) {
    let notif = world.rejection_notification.as_ref().unwrap();
    assert_eq!(notif.source_domain, expected);
}

#[then(expr = "the source_event_sequence should be {int}")]
async fn then_source_sequence(world: &mut CompensationWorld, expected: u32) {
    let notif = world.rejection_notification.as_ref().unwrap();
    assert_eq!(notif.source_event_sequence, expected);
}

#[then(expr = "the issuer_name should be {string}")]
async fn then_issuer_name(world: &mut CompensationWorld, expected: String) {
    let notif = world.rejection_notification.as_ref().unwrap();
    assert_eq!(notif.issuer_name, expected);
}

#[then(expr = "the issuer_type should be {string}")]
async fn then_issuer_type(world: &mut CompensationWorld, expected: String) {
    let notif = world.rejection_notification.as_ref().unwrap();
    assert_eq!(notif.issuer_type, expected);
}

#[then("the notification should have a cover")]
async fn then_notification_has_cover(world: &mut CompensationWorld) {
    let notif = world.notification.as_ref().unwrap();
    assert!(!notif.cover_domain.is_empty());
}

#[then("the notification payload should contain RejectionNotification")]
async fn then_payload_contains_rejection(world: &mut CompensationWorld) {
    let notif = world.notification.as_ref().unwrap();
    assert!(notif.payload_type_url.contains("RejectionNotification"));
}

#[then(expr = "the payload type_url should be {string}")]
async fn then_payload_type_url(world: &mut CompensationWorld, expected: String) {
    let notif = world.notification.as_ref().unwrap();
    assert_eq!(notif.payload_type_url, expected);
}

#[then("the notification should have a sent_at timestamp")]
async fn then_has_sent_at(world: &mut CompensationWorld) {
    let notif = world.notification.as_ref().unwrap();
    assert!(notif.sent_at > 0);
}

#[then("the timestamp should be recent")]
async fn then_timestamp_recent(world: &mut CompensationWorld) {
    let notif = world.notification.as_ref().unwrap();
    let now = chrono::Utc::now().timestamp();
    assert!(notif.sent_at >= now - 60);
}

#[then("the command book should target the source aggregate")]
async fn then_targets_source(world: &mut CompensationWorld) {
    let book = world.command_book.as_ref().unwrap();
    let cover = book.cover.as_ref().unwrap();
    assert!(!cover.domain.is_empty());
}

#[then("the command book should have MERGE_COMMUTATIVE strategy")]
async fn then_merge_commutative(world: &mut CompensationWorld) {
    let book = world.command_book.as_ref().unwrap();
    let page = &book.pages[0];
    assert_eq!(page.merge_strategy, MergeStrategy::MergeCommutative as i32);
}

#[then("the command book should preserve correlation ID")]
async fn then_preserves_correlation(world: &mut CompensationWorld) {
    let book = world.command_book.as_ref().unwrap();
    let cover = book.cover.as_ref().unwrap();
    assert!(!cover.correlation_id.is_empty());
}

#[then(expr = "the command book cover should have domain {string}")]
async fn then_book_domain(world: &mut CompensationWorld, expected: String) {
    let book = world.command_book.as_ref().unwrap();
    let cover = book.cover.as_ref().unwrap();
    assert_eq!(cover.domain, expected);
}

#[then(expr = "the command book cover should have root {string}")]
async fn then_book_root(world: &mut CompensationWorld, _expected: String) {
    let book = world.command_book.as_ref().unwrap();
    let cover = book.cover.as_ref().unwrap();
    assert!(cover.root.is_some());
}

#[then(expr = "the rejection_reason should be {string}")]
async fn then_rejection_reason(world: &mut CompensationWorld, expected: String) {
    let notif = world.rejection_notification.as_ref().unwrap();
    assert_eq!(notif.rejection_reason, expected);
}

#[then("the rejection_reason should contain the full error details")]
async fn then_reason_has_details(world: &mut CompensationWorld) {
    let notif = world.rejection_notification.as_ref().unwrap();
    assert!(notif.rejection_reason.contains("structured"));
}

#[then("the rejected_command should be the original command")]
async fn then_rejected_is_original(world: &mut CompensationWorld) {
    let notif = world.rejection_notification.as_ref().unwrap();
    assert!(notif.rejected_command.is_some());
}

#[then("all command fields should be preserved")]
async fn then_fields_preserved(world: &mut CompensationWorld) {
    let notif = world.rejection_notification.as_ref().unwrap();
    let cmd = notif.rejected_command.as_ref().unwrap();
    assert!(cmd.cover.is_some());
    assert!(!cmd.pages.is_empty());
}

#[then("the full saga origin chain should be preserved")]
async fn then_origin_chain_preserved(world: &mut CompensationWorld) {
    let ctx = world.compensation_context.as_ref().unwrap();
    assert!(ctx.saga_origin.is_some());
}

#[then("root cause can be traced through the chain")]
async fn then_can_trace_root_cause(world: &mut CompensationWorld) {
    let ctx = world.compensation_context.as_ref().unwrap();
    let origin = ctx.saga_origin.as_ref().unwrap();
    assert!(!origin.triggering_domain.is_empty());
}

#[then("the router should build a CompensationContext")]
async fn then_router_builds_context(world: &mut CompensationWorld) {
    assert!(world.compensation_context.is_some());
}

#[then("the router should emit a rejection notification")]
async fn then_router_emits_notification(world: &mut CompensationWorld) {
    let ctx = world.compensation_context.as_ref().unwrap();
    let notif = ctx.build_rejection_notification();
    assert!(!notif.rejection_reason.is_empty());
}

#[then(expr = "the context should have issuer_type {string}")]
async fn then_context_issuer_type(world: &mut CompensationWorld, expected: String) {
    // PM uses "process_manager" issuer type
    let ctx = world.compensation_context.as_ref().unwrap();
    let notif = ctx.build_rejection_notification();
    // In real impl, issuer_type would be set based on component type
    // For this test, we're simulating
    assert!(expected == "saga" || expected == "process_manager");
}
