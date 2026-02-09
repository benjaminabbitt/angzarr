//! Saga compensation handling.
//!
//! Provides utilities for handling saga command rejections, including:
//! - Building RevokeEventCommand messages
//! - Emitting SagaCompensationFailed events
//! - Dead letter queue routing
//! - Escalation triggers

use prost::Message;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::config::SagaCompensationConfig;
use crate::proto::{
    business_response, BusinessResponse, CommandBook, Cover, EventBook, EventPage,
    RevocationResponse, RevokeEventCommand, SagaCommandOrigin, SagaCompensationFailed,
    Uuid as ProtoUuid,
};
use crate::proto_ext::CoverExt;

/// Result type for compensation operations.
pub type Result<T> = std::result::Result<T, CompensationError>;

/// Errors that can occur during saga compensation.
#[derive(Debug, thiserror::Error)]
pub enum CompensationError {
    #[error("Command missing saga origin - not a saga command")]
    MissingSagaOrigin,

    #[error("Missing triggering aggregate in saga origin")]
    MissingTriggeringAggregate,

    #[error("Compensation aborted: {0}")]
    Aborted(String),

    #[error("DLQ send failed: {0}")]
    DlqFailed(String),

    #[error("Escalation failed: {0}")]
    EscalationFailed(String),

    #[error("Event store error: {0}")]
    EventStore(String),
}

/// Context for compensation operations.
///
/// Contains all information needed to build compensation events
/// and route failures.
#[derive(Debug, Clone)]
pub struct CompensationContext {
    /// The saga origin from the rejected command.
    pub saga_origin: SagaCommandOrigin,
    /// Why the command was rejected.
    pub rejection_reason: String,
    /// The rejected command.
    pub rejected_command: CommandBook,
    /// Correlation ID for tracing.
    pub correlation_id: String,
}

impl CompensationContext {
    /// Create a new compensation context from a rejected command.
    ///
    /// Returns None if the command doesn't have a saga origin
    /// (indicating it's not a saga-issued command).
    pub fn from_rejected_command(command: &CommandBook, rejection_reason: String) -> Option<Self> {
        let saga_origin = command.saga_origin.as_ref()?.clone();
        let correlation_id = command.correlation_id().to_string();

        Some(Self {
            saga_origin,
            rejection_reason,
            rejected_command: command.clone(),
            correlation_id,
        })
    }
}

/// Build a RevokeEventCommand for a rejected saga command.
///
/// This command will be sent to the original aggregate that triggered
/// the saga, allowing it to emit compensation events.
pub fn build_revoke_command(context: &CompensationContext) -> RevokeEventCommand {
    RevokeEventCommand {
        triggering_event_sequence: context.saga_origin.triggering_event_sequence,
        saga_name: context.saga_origin.saga_name.clone(),
        rejection_reason: context.rejection_reason.clone(),
        rejected_command: Some(context.rejected_command.clone()),
    }
}

/// Build a CommandBook to send the RevokeEventCommand to the triggering aggregate.
pub fn build_revoke_command_book(context: &CompensationContext) -> Result<CommandBook> {
    let triggering_aggregate = context
        .saga_origin
        .triggering_aggregate
        .as_ref()
        .ok_or(CompensationError::MissingTriggeringAggregate)?;

    let revoke_cmd = build_revoke_command(context);

    // Clone triggering aggregate and set correlation_id on cover
    let mut cover = triggering_aggregate.clone();
    if cover.correlation_id.is_empty() {
        cover.correlation_id = context.correlation_id.clone();
    }

    Ok(CommandBook {
        cover: Some(cover),
        pages: vec![crate::proto::CommandPage {
            sequence: 0,
            command: Some(prost_types::Any {
                type_url: "type.angzarr/angzarr.RevokeEventCommand".to_string(),
                value: revoke_cmd.encode_to_vec(),
            }),
        }],
        saga_origin: None, // Revoke commands don't have their own saga origin
    })
}

/// Build a SagaCompensationFailed event.
///
/// This is emitted when client logic cannot handle the revocation
/// or explicitly requests system revocation.
pub fn build_compensation_failed_event(
    context: &CompensationContext,
    compensation_failure_reason: &str,
) -> SagaCompensationFailed {
    SagaCompensationFailed {
        triggering_aggregate: context.saga_origin.triggering_aggregate.clone(),
        triggering_event_sequence: context.saga_origin.triggering_event_sequence,
        saga_name: context.saga_origin.saga_name.clone(),
        rejection_reason: context.rejection_reason.clone(),
        compensation_failure_reason: compensation_failure_reason.to_string(),
        rejected_command: Some(context.rejected_command.clone()),
        occurred_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
    }
}

/// Build an EventBook containing the SagaCompensationFailed event.
///
/// Uses the fallback domain from config as the target domain.
pub fn build_compensation_failed_event_book(
    context: &CompensationContext,
    compensation_failure_reason: &str,
    config: &SagaCompensationConfig,
) -> EventBook {
    let event = build_compensation_failed_event(context, compensation_failure_reason);
    let fallback_root = Uuid::new_v4();

    EventBook {
        cover: Some(Cover {
            domain: config.fallback_domain.clone(),
            root: Some(ProtoUuid {
                value: fallback_root.as_bytes().to_vec(),
            }),
            correlation_id: context.correlation_id.clone(),
            edition: None,
        }),
        pages: vec![EventPage {
            sequence: Some(crate::proto::event_page::Sequence::Num(0)),
            created_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
            event: Some(prost_types::Any {
                type_url: "type.angzarr/angzarr.SagaCompensationFailed".to_string(),
                value: event.encode_to_vec(),
            }),
        }],
        snapshot: None,
        ..Default::default()
    }
}

/// Outcome of handling a business response to a RevokeEventCommand.
#[derive(Debug)]
pub enum CompensationOutcome {
    /// Business provided compensation events - use them.
    Events(EventBook),
    /// Emit SagaCompensationFailed event to fallback domain.
    EmitSystemRevocation(EventBook),
    /// Compensation declined, just log.
    Declined { reason: String },
    /// Abort saga chain, propagate error to caller.
    Aborted { reason: String },
}

/// Handle a BusinessResponse to a RevokeEventCommand.
///
/// Implements the decision logic for processing revocation responses:
/// 1. If business returns events with pages → use them
/// 2. If business returns RevocationResponse → process flags
/// 3. If empty/error → use config-based fallback
///
/// Returns actions to take (emit events, DLQ, escalate, etc.)
pub fn handle_business_response(
    response: std::result::Result<BusinessResponse, tonic::Status>,
    context: &CompensationContext,
    config: &SagaCompensationConfig,
) -> Result<CompensationOutcome> {
    let revocation = match response {
        Ok(BusinessResponse {
            result: Some(business_response::Result::Events(book)),
        }) if !book.pages.is_empty() => {
            // Business provided compensation events - use them
            info!(
                saga = %context.saga_origin.saga_name,
                events = book.pages.len(),
                "Business provided compensation events"
            );
            return Ok(CompensationOutcome::Events(book));
        }
        Ok(BusinessResponse {
            result: Some(business_response::Result::Revocation(r)),
        }) => r,
        Ok(_) => {
            // Empty events → use config-based fallback flags
            warn!(
                saga = %context.saga_origin.saga_name,
                "Business returned empty response, using fallback"
            );
            RevocationResponse {
                emit_system_revocation: config.fallback_emit_system_revocation,
                send_to_dead_letter_queue: config.fallback_send_to_dlq,
                escalate: config.fallback_escalate,
                abort: false, // Don't abort on fallback
                reason: "client logic returned empty response".to_string(),
            }
        }
        Err(status) => {
            // gRPC error → use config-based fallback flags
            error!(
                saga = %context.saga_origin.saga_name,
                error = %status,
                "gRPC error from client logic, using fallback"
            );
            RevocationResponse {
                emit_system_revocation: config.fallback_emit_system_revocation,
                send_to_dead_letter_queue: config.fallback_send_to_dlq,
                escalate: config.fallback_escalate,
                abort: false, // Don't abort on fallback
                reason: format!("gRPC error: {}", status),
            }
        }
    };

    // Process revocation flags
    process_revocation_flags(&revocation, context, config)
}

/// Process RevocationResponse flags and take appropriate actions.
fn process_revocation_flags(
    revocation: &RevocationResponse,
    context: &CompensationContext,
    config: &SagaCompensationConfig,
) -> Result<CompensationOutcome> {
    info!(
        saga = %context.saga_origin.saga_name,
        emit = revocation.emit_system_revocation,
        dlq = revocation.send_to_dead_letter_queue,
        escalate = revocation.escalate,
        abort = revocation.abort,
        reason = %revocation.reason,
        "Processing revocation response"
    );

    // Send to DLQ if requested
    if revocation.send_to_dead_letter_queue {
        if let Err(e) = send_to_dead_letter_queue(context, &revocation.reason, config) {
            error!(error = %e, "Failed to send to DLQ");
            // Continue processing other flags even if DLQ fails
        }
    }

    // Trigger escalation if requested
    if revocation.escalate {
        if let Err(e) = trigger_escalation(context, &revocation.reason, config) {
            error!(error = %e, "Failed to trigger escalation");
            // Continue processing other flags even if escalation fails
        }
    }

    // Check abort flag first - it takes precedence
    if revocation.abort {
        return Err(CompensationError::Aborted(revocation.reason.clone()));
    }

    // Emit system revocation event if requested
    if revocation.emit_system_revocation {
        let event_book = build_compensation_failed_event_book(context, &revocation.reason, config);
        return Ok(CompensationOutcome::EmitSystemRevocation(event_book));
    }

    // No flags set - declined, just log
    Ok(CompensationOutcome::Declined {
        reason: revocation.reason.clone(),
    })
}

/// Send compensation failure context to dead letter queue.
///
/// Currently a stub - will be implemented with AMQP integration.
pub fn send_to_dead_letter_queue(
    context: &CompensationContext,
    reason: &str,
    config: &SagaCompensationConfig,
) -> Result<()> {
    let Some(dlq_url) = &config.dead_letter_queue_url else {
        warn!("DLQ requested but not configured");
        return Ok(());
    };

    info!(
        saga = %context.saga_origin.saga_name,
        dlq_url = %dlq_url,
        reason = %reason,
        "Sending to dead letter queue"
    );

    // TODO: Implement actual DLQ send via AMQP
    // For now, just log the intent
    Ok(())
}

/// Trigger escalation for compensation failure.
///
/// Currently logs at ERROR level. Will integrate with webhook when configured.
pub fn trigger_escalation(
    context: &CompensationContext,
    reason: &str,
    config: &SagaCompensationConfig,
) -> Result<()> {
    // Always log at ERROR for escalations
    error!(
        saga = %context.saga_origin.saga_name,
        triggering_aggregate = ?context.saga_origin.triggering_aggregate,
        triggering_sequence = context.saga_origin.triggering_event_sequence,
        rejection_reason = %context.rejection_reason,
        compensation_reason = %reason,
        "ESCALATION: Saga compensation failed"
    );

    // Send to webhook if configured
    if let Some(webhook_url) = &config.escalation_webhook_url {
        info!(
            webhook = %webhook_url,
            "Sending escalation to webhook"
        );
        // TODO: Implement actual webhook call
        // For now, just log the intent
    }

    Ok(())
}

#[cfg(test)]
mod tests;
