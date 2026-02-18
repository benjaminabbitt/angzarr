use super::*;
use crate::config::DEFAULT_SAGA_FALLBACK_DOMAIN;
use crate::proto::{CommandPage, MergeStrategy};

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
            command: Some(prost_types::Any {
                type_url: "test.AddPoints".to_string(),
                value: vec![],
            }),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
            external_payload: None,
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

#[test]
fn test_handle_business_response_with_events() {
    let context = make_context();
    let config = SagaCompensationConfig::default();

    let response = Ok(BusinessResponse {
        result: Some(business_response::Result::Events(EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: None,
                correlation_id: "corr-123".to_string(),
                edition: None,
            }),
            pages: vec![EventPage {
                sequence: Some(crate::proto::event_page::Sequence::Num(6)),
                created_at: None,
                external_payload: None,
                event: Some(prost_types::Any {
                    type_url: "test.Compensated".to_string(),
                    value: vec![],
                }),
            }],
            snapshot: None,
            ..Default::default()
        })),
    });

    let outcome = handle_business_response(response, &context, &config).unwrap();
    assert!(matches!(outcome, CompensationOutcome::Events(_)));
}

#[test]
fn test_handle_business_response_empty_events_uses_fallback() {
    let context = make_context();
    let config = SagaCompensationConfig {
        fallback_emit_system_revocation: true,
        ..Default::default()
    };

    let response = Ok(BusinessResponse {
        result: Some(business_response::Result::Events(EventBook {
            cover: None,
            pages: vec![], // Empty!
            snapshot: None,
            ..Default::default()
        })),
    });

    let outcome = handle_business_response(response, &context, &config).unwrap();
    assert!(matches!(
        outcome,
        CompensationOutcome::EmitSystemRevocation(_)
    ));
}

#[test]
fn test_handle_business_response_revocation_emit() {
    let context = make_context();
    let config = SagaCompensationConfig::default();

    let response = Ok(BusinessResponse {
        result: Some(business_response::Result::Revocation(RevocationResponse {
            emit_system_revocation: true,
            send_to_dead_letter_queue: false,
            escalate: false,
            abort: false,
            reason: "Cannot compensate".to_string(),
        })),
    });

    let outcome = handle_business_response(response, &context, &config).unwrap();
    assert!(matches!(
        outcome,
        CompensationOutcome::EmitSystemRevocation(_)
    ));
}

#[test]
fn test_handle_business_response_revocation_abort() {
    let context = make_context();
    let config = SagaCompensationConfig::default();

    let response = Ok(BusinessResponse {
        result: Some(business_response::Result::Revocation(RevocationResponse {
            emit_system_revocation: false,
            send_to_dead_letter_queue: false,
            escalate: false,
            abort: true,
            reason: "Critical failure".to_string(),
        })),
    });

    let result = handle_business_response(response, &context, &config);
    assert!(matches!(result, Err(CompensationError::Aborted(_))));
}

#[test]
fn test_handle_business_response_revocation_declined() {
    let context = make_context();
    let config = SagaCompensationConfig::default();

    let response = Ok(BusinessResponse {
        result: Some(business_response::Result::Revocation(RevocationResponse {
            emit_system_revocation: false,
            send_to_dead_letter_queue: false,
            escalate: false,
            abort: false,
            reason: "Already handled".to_string(),
        })),
    });

    let outcome = handle_business_response(response, &context, &config).unwrap();
    assert!(matches!(outcome, CompensationOutcome::Declined { .. }));
}

#[test]
fn test_handle_business_response_grpc_error_uses_fallback() {
    let context = make_context();
    let config = SagaCompensationConfig {
        fallback_emit_system_revocation: true,
        ..Default::default()
    };

    let response = Err(tonic::Status::unavailable("Service down"));

    let outcome = handle_business_response(response, &context, &config).unwrap();
    assert!(matches!(
        outcome,
        CompensationOutcome::EmitSystemRevocation(_)
    ));
}

#[test]
fn test_send_to_dlq_no_url_configured() {
    let context = make_context();
    let config = SagaCompensationConfig {
        dead_letter_queue_url: None,
        ..Default::default()
    };

    // Should succeed silently when DLQ not configured
    let result = send_to_dead_letter_queue(&context, "test", &config);
    assert!(result.is_ok());
}

#[test]
fn test_send_to_dlq_with_url() {
    let context = make_context();
    let config = SagaCompensationConfig {
        dead_letter_queue_url: Some("amqp://localhost:5672/dlq".to_string()),
        ..Default::default()
    };

    // Currently just logs - should succeed
    let result = send_to_dead_letter_queue(&context, "test", &config);
    assert!(result.is_ok());
}

#[test]
fn test_trigger_escalation_no_webhook() {
    let context = make_context();
    let config = SagaCompensationConfig {
        escalation_webhook_url: None,
        ..Default::default()
    };

    // Should succeed (logs at ERROR level)
    let result = trigger_escalation(&context, "test", &config);
    assert!(result.is_ok());
}

#[test]
fn test_trigger_escalation_with_webhook() {
    let context = make_context();
    let config = SagaCompensationConfig {
        escalation_webhook_url: Some("https://alerts.example.com/webhook".to_string()),
        ..Default::default()
    };

    // Currently just logs - should succeed
    let result = trigger_escalation(&context, "test", &config);
    assert!(result.is_ok());
}
