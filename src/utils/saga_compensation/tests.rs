use super::*;
use crate::config::DEFAULT_SAGA_FALLBACK_DOMAIN;
use crate::proto::{command_page, event_page, CommandPage, MergeStrategy};

fn make_saga_origin() -> SagaCommandOrigin {
    SagaCommandOrigin {
        saga_name: "test_saga".to_string(),
        triggering_aggregate: Some(Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: Uuid::new_v4().as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        triggering_event_sequence: 5,
    }
}

fn make_test_command() -> CommandBook {
    CommandBook {
        cover: Some(Cover {
            domain: "customer".to_string(),
            root: Some(ProtoUuid {
                value: Uuid::new_v4().as_bytes().to_vec(),
            }),
            correlation_id: "corr-123".to_string(),
            edition: None,
        }),
        pages: vec![CommandPage {
            sequence: 0,
            payload: Some(command_page::Payload::Command(prost_types::Any {
                type_url: "test.AddPoints".to_string(),
                value: vec![],
            })),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
        }],
        saga_origin: Some(make_saga_origin()),
    }
}

fn make_context() -> CompensationContext {
    CompensationContext {
        saga_origin: make_saga_origin(),
        rejection_reason: "Customer not found".to_string(),
        rejected_command: make_test_command(),
        correlation_id: "corr-123".to_string(),
    }
}

#[test]
fn test_compensation_context_from_saga_command() {
    let command = make_test_command();
    let context =
        CompensationContext::from_rejected_command(&command, "rejection reason".to_string());

    assert!(context.is_some());
    let ctx = context.unwrap();
    assert_eq!(ctx.saga_origin.saga_name, "test_saga");
    assert_eq!(ctx.rejection_reason, "rejection reason");
}

#[test]
fn test_compensation_context_from_non_saga_command() {
    let mut command = make_test_command();
    command.saga_origin = None;

    let context =
        CompensationContext::from_rejected_command(&command, "rejection reason".to_string());

    assert!(context.is_none());
}

#[test]
fn test_build_rejection_notification() {
    let context = make_context();
    let notification = build_rejection_notification(&context);

    assert_eq!(notification.issuer_name, "test_saga");
    assert_eq!(notification.source_event_sequence, 5);
    assert_eq!(notification.rejection_reason, "Customer not found");
    assert!(notification.rejected_command.is_some());
}

#[test]
fn test_build_notification_command_book() {
    let context = make_context();
    let command_book = build_notification_command_book(&context).unwrap();

    assert!(command_book.cover.is_some());
    let cover = command_book.cover.unwrap();
    assert_eq!(cover.domain, "orders");
    assert!(command_book.saga_origin.is_none());
}

#[test]
fn test_build_notification_command_book_missing_aggregate() {
    let mut context = make_context();
    context.saga_origin.triggering_aggregate = None;

    let result = build_notification_command_book(&context);
    assert!(matches!(
        result,
        Err(CompensationError::MissingTriggeringAggregate)
    ));
}

#[test]
fn test_build_compensation_failed_event() {
    let context = make_context();
    let event = build_compensation_failed_event(&context, "Business declined");

    assert_eq!(event.saga_name, "test_saga");
    assert_eq!(event.triggering_event_sequence, 5);
    assert_eq!(event.rejection_reason, "Customer not found");
    assert_eq!(event.compensation_failure_reason, "Business declined");
    assert!(event.occurred_at.is_some());
}

#[test]
fn test_build_compensation_failed_event_book() {
    let context = make_context();
    let config = SagaCompensationConfig::default();
    let event_book = build_compensation_failed_event_book(&context, "failure", &config);

    assert!(event_book.cover.is_some());
    let cover = event_book.cover.unwrap();
    assert_eq!(cover.domain, DEFAULT_SAGA_FALLBACK_DOMAIN);
    assert_eq!(event_book.pages.len(), 1);
    assert_eq!(cover.correlation_id, "corr-123");
}

#[tokio::test]
async fn test_handle_business_response_with_events() {
    let context = make_context();
    let config = SagaCompensationConfig::default();
    let handler = NoopEscalationHandler;

    let response = Ok(BusinessResponse {
        result: Some(business_response::Result::Events(EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: None,
                correlation_id: "corr-123".to_string(),
                edition: None,
            }),
            pages: vec![EventPage {
                sequence: 6,
                created_at: None,
                payload: Some(event_page::Payload::Event(prost_types::Any {
                    type_url: "test.Compensated".to_string(),
                    value: vec![],
                })),
            }],
            snapshot: None,
            ..Default::default()
        })),
    });

    let outcome = handle_business_response(response, &context, &config, &handler)
        .await
        .unwrap();
    assert!(matches!(outcome, CompensationOutcome::Events(_)));
}

#[tokio::test]
async fn test_handle_business_response_empty_events_uses_fallback() {
    let context = make_context();
    let config = SagaCompensationConfig {
        fallback_emit_system_revocation: true,
        ..Default::default()
    };
    let handler = NoopEscalationHandler;

    let response = Ok(BusinessResponse {
        result: Some(business_response::Result::Events(EventBook {
            cover: None,
            pages: vec![], // Empty!
            snapshot: None,
            ..Default::default()
        })),
    });

    let outcome = handle_business_response(response, &context, &config, &handler)
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        CompensationOutcome::EmitSystemRevocation(_)
    ));
}

#[tokio::test]
async fn test_handle_business_response_revocation_emit() {
    let context = make_context();
    let config = SagaCompensationConfig::default();
    let handler = NoopEscalationHandler;

    let response = Ok(BusinessResponse {
        result: Some(business_response::Result::Revocation(RevocationResponse {
            emit_system_revocation: true,
            send_to_dead_letter_queue: false,
            escalate: false,
            abort: false,
            reason: "Cannot compensate".to_string(),
        })),
    });

    let outcome = handle_business_response(response, &context, &config, &handler)
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        CompensationOutcome::EmitSystemRevocation(_)
    ));
}

#[tokio::test]
async fn test_handle_business_response_revocation_abort() {
    let context = make_context();
    let config = SagaCompensationConfig::default();
    let handler = NoopEscalationHandler;

    let response = Ok(BusinessResponse {
        result: Some(business_response::Result::Revocation(RevocationResponse {
            emit_system_revocation: false,
            send_to_dead_letter_queue: false,
            escalate: false,
            abort: true,
            reason: "Critical failure".to_string(),
        })),
    });

    let result = handle_business_response(response, &context, &config, &handler).await;
    assert!(matches!(result, Err(CompensationError::Aborted(_))));
}

#[tokio::test]
async fn test_handle_business_response_revocation_declined() {
    let context = make_context();
    let config = SagaCompensationConfig::default();
    let handler = NoopEscalationHandler;

    let response = Ok(BusinessResponse {
        result: Some(business_response::Result::Revocation(RevocationResponse {
            emit_system_revocation: false,
            send_to_dead_letter_queue: false,
            escalate: false,
            abort: false,
            reason: "Already handled".to_string(),
        })),
    });

    let outcome = handle_business_response(response, &context, &config, &handler)
        .await
        .unwrap();
    assert!(matches!(outcome, CompensationOutcome::Declined { .. }));
}

#[tokio::test]
async fn test_handle_business_response_grpc_error_uses_fallback() {
    let context = make_context();
    let config = SagaCompensationConfig {
        fallback_emit_system_revocation: true,
        ..Default::default()
    };
    let handler = NoopEscalationHandler;

    let response = Err(tonic::Status::unavailable("Service down"));

    let outcome = handle_business_response(response, &context, &config, &handler)
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        CompensationOutcome::EmitSystemRevocation(_)
    ));
}

#[tokio::test]
async fn test_noop_escalation_handler_quarantine() {
    let context = make_context();
    let handler = NoopEscalationHandler;

    // NoopEscalationHandler always succeeds (logs only)
    let result = handler.quarantine(&context, "test reason").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_noop_escalation_handler_notify() {
    let context = make_context();
    let handler = NoopEscalationHandler;

    // NoopEscalationHandler always succeeds (logs only)
    let result = handler.notify(&context, "test reason").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_handle_business_response_with_notify_flag() {
    let context = make_context();
    let config = SagaCompensationConfig::default();
    let handler = NoopEscalationHandler;

    // When escalate flag is true, handler.notify() is called
    let response = Ok(BusinessResponse {
        result: Some(business_response::Result::Revocation(RevocationResponse {
            emit_system_revocation: true,
            send_to_dead_letter_queue: false,
            escalate: true, // This triggers notify()
            abort: false,
            reason: "Notification needed".to_string(),
        })),
    });

    let outcome = handle_business_response(response, &context, &config, &handler)
        .await
        .unwrap();
    // Should still return EmitSystemRevocation since that flag is also set
    assert!(matches!(
        outcome,
        CompensationOutcome::EmitSystemRevocation(_)
    ));
}

#[tokio::test]
async fn test_handle_business_response_with_quarantine_flag() {
    let context = make_context();
    let config = SagaCompensationConfig::default();
    let handler = NoopEscalationHandler;

    // When send_to_dead_letter_queue flag is true, handler.quarantine() is called
    let response = Ok(BusinessResponse {
        result: Some(business_response::Result::Revocation(RevocationResponse {
            emit_system_revocation: false,
            send_to_dead_letter_queue: true, // This triggers quarantine()
            escalate: false,
            abort: false,
            reason: "Quarantine requested".to_string(),
        })),
    });

    let outcome = handle_business_response(response, &context, &config, &handler)
        .await
        .unwrap();
    // Should return Declined since emit_system_revocation is false
    assert!(matches!(outcome, CompensationOutcome::Declined { .. }));
}
