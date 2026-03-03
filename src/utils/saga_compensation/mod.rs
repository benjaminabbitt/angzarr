//! Saga compensation handling.
//!
//! DOC: This file is referenced in docs/docs/operations/error-recovery.mdx
//!      Update documentation when making changes to compensation patterns.
//!
//! Provides utilities for handling saga command rejections, including:
//! - Building Notification messages with RejectionNotification payload
//! - Emitting SagaCompensationFailed events
//! - Escalation via configurable handlers (EventBus, webhook, etc.)

use std::sync::Arc;

use async_trait::async_trait;
use prost::Message;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::bus::EventBus;
use crate::config::SagaCompensationConfig;
use crate::proto::{
    business_response, BusinessResponse, CommandBook, Cover, EventBook, EventPage, MergeStrategy,
    Notification, RejectionNotification, RevocationResponse, SagaCommandOrigin,
    SagaCompensationFailed, Uuid as ProtoUuid,
};
use crate::proto_ext::type_url;
use crate::proto_ext::CoverExt;

/// Result type for compensation operations.
pub type Result<T> = std::result::Result<T, CompensationError>;

/// Error message constants for compensation operations.
pub mod errmsg {
    pub const MISSING_SAGA_ORIGIN: &str = "Command missing saga origin - not a saga command";
    pub const MISSING_TRIGGERING_AGGREGATE: &str = "Missing triggering aggregate in saga origin";
    pub const ABORTED: &str = "Compensation aborted: ";
    pub const ESCALATION_FAILED: &str = "Escalation failed: ";
    pub const EVENT_STORE_ERROR: &str = "Event store error: ";
}

/// Errors that can occur during saga compensation.
#[derive(Debug, thiserror::Error)]
pub enum CompensationError {
    #[error("{}", errmsg::MISSING_SAGA_ORIGIN)]
    MissingSagaOrigin,

    #[error("{}", errmsg::MISSING_TRIGGERING_AGGREGATE)]
    MissingTriggeringAggregate,

    #[error("{}{}", errmsg::ABORTED, .0)]
    Aborted(String),

    #[error("{}{}", errmsg::ESCALATION_FAILED, .0)]
    EscalationFailed(String),

    #[error("{}{}", errmsg::EVENT_STORE_ERROR, .0)]
    EventStore(String),
}

/// Trait for handling escalation actions during saga compensation.
///
/// Two distinct concerns:
/// - Quarantine: isolate failed messages for later reprocessing (operational)
/// - Notify: inform operators of failures requiring attention (observability)
///
/// These are intentionally separate methods because callers may want one without
/// the other (e.g., quarantine for replay without alerting, or alerting without quarantine).
#[async_trait]
pub trait EscalationHandler: Send + Sync {
    /// Quarantine a compensation failure for later reprocessing.
    ///
    /// Called when `send_to_dead_letter_queue` flag is set. Implementations
    /// should preserve the full context for later replay/investigation.
    async fn quarantine(
        &self,
        context: &CompensationContext,
        reason: &str,
    ) -> std::result::Result<(), CompensationError>;

    /// Notify operators of a compensation failure.
    ///
    /// Called when `escalate` flag is set. Implementations should alert
    /// operators for manual review and resolution.
    async fn notify(
        &self,
        context: &CompensationContext,
        reason: &str,
    ) -> std::result::Result<(), CompensationError>;
}

/// Default escalation handler that routes based on configuration.
///
/// - `quarantine`: If `dead_letter_queue_url` configured → publishes to fallback domain via EventBus
/// - `notify`: If `escalation_webhook_url` configured → calls webhook (TODO)
pub struct DefaultEscalationHandler {
    event_bus: Arc<dyn EventBus>,
    config: SagaCompensationConfig,
}

impl DefaultEscalationHandler {
    /// Create a new default escalation handler.
    pub fn new(event_bus: Arc<dyn EventBus>, config: SagaCompensationConfig) -> Self {
        Self { event_bus, config }
    }
}

#[async_trait]
impl EscalationHandler for DefaultEscalationHandler {
    async fn quarantine(
        &self,
        context: &CompensationContext,
        reason: &str,
    ) -> std::result::Result<(), CompensationError> {
        let Some(ref dlq_url) = self.config.dead_letter_queue_url else {
            warn!(
                saga = %context.saga_origin.saga_name,
                "Quarantine requested but dead_letter_queue_url not configured"
            );
            return Ok(());
        };

        info!(
            saga = %context.saga_origin.saga_name,
            dlq_url = %dlq_url,
            reason = %reason,
            "Quarantining compensation failure"
        );

        let event_book = build_compensation_failed_event_book(context, reason, &self.config);
        self.event_bus
            .publish(Arc::new(event_book))
            .await
            .map_err(|e| {
                CompensationError::EscalationFailed(format!("Quarantine failed: {}", e))
            })?;

        Ok(())
    }

    async fn notify(
        &self,
        context: &CompensationContext,
        reason: &str,
    ) -> std::result::Result<(), CompensationError> {
        // Always log at ERROR for notifications
        error!(
            saga = %context.saga_origin.saga_name,
            triggering_aggregate = ?context.saga_origin.triggering_aggregate,
            triggering_sequence = context.saga_origin.triggering_event_sequence,
            rejection_reason = %context.rejection_reason,
            compensation_reason = %reason,
            "NOTIFY: Saga compensation failed"
        );

        let Some(ref webhook_url) = self.config.escalation_webhook_url else {
            warn!(
                saga = %context.saga_origin.saga_name,
                "Notification requested but escalation_webhook_url not configured"
            );
            return Ok(());
        };

        // Build webhook payload
        let triggering_aggregate = context.saga_origin.triggering_aggregate.as_ref();
        let payload = serde_json::json!({
            "saga_name": context.saga_origin.saga_name,
            "triggering_domain": triggering_aggregate.map(|c| &c.domain),
            "triggering_root_id": triggering_aggregate
                .and_then(|c| c.root.as_ref())
                .map(|u| hex::encode(&u.value)),
            "triggering_event_sequence": context.saga_origin.triggering_event_sequence,
            "rejection_reason": context.rejection_reason,
            "compensation_reason": reason,
            "correlation_id": context.correlation_id,
            "occurred_at": chrono::Utc::now().to_rfc3339(),
        });

        // POST to webhook (best-effort, don't fail on errors)
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| {
                error!(error = %e, "Failed to create HTTP client for webhook");
                CompensationError::EscalationFailed(format!("HTTP client error: {}", e))
            })?;

        match client
            .post(webhook_url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                info!(
                    saga = %context.saga_origin.saga_name,
                    webhook = %webhook_url,
                    status = %response.status(),
                    "Webhook notification sent successfully"
                );
            }
            Ok(response) => {
                warn!(
                    saga = %context.saga_origin.saga_name,
                    webhook = %webhook_url,
                    status = %response.status(),
                    "Webhook returned non-success status"
                );
            }
            Err(e) => {
                error!(
                    saga = %context.saga_origin.saga_name,
                    webhook = %webhook_url,
                    error = %e,
                    "Failed to send webhook notification"
                );
            }
        }

        Ok(())
    }
}

/// No-op escalation handler that only logs.
///
/// Useful for tests or when escalation is disabled.
pub struct NoopEscalationHandler;

#[async_trait]
impl EscalationHandler for NoopEscalationHandler {
    async fn quarantine(
        &self,
        context: &CompensationContext,
        reason: &str,
    ) -> std::result::Result<(), CompensationError> {
        warn!(
            saga = %context.saga_origin.saga_name,
            reason = %reason,
            "Quarantine requested but using NoopEscalationHandler (logging only)"
        );
        Ok(())
    }

    async fn notify(
        &self,
        context: &CompensationContext,
        reason: &str,
    ) -> std::result::Result<(), CompensationError> {
        warn!(
            saga = %context.saga_origin.saga_name,
            reason = %reason,
            "Notification requested but using NoopEscalationHandler (logging only)"
        );
        Ok(())
    }
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

/// Build a RejectionNotification for a rejected saga command.
///
/// This is the payload for the Notification sent to the original aggregate
/// that triggered the saga, allowing it to emit compensation events.
pub fn build_rejection_notification(context: &CompensationContext) -> RejectionNotification {
    RejectionNotification {
        rejected_command: Some(context.rejected_command.clone()),
        rejection_reason: context.rejection_reason.clone(),
        issuer_name: context.saga_origin.saga_name.clone(),
        issuer_type: "saga".to_string(),
        source_aggregate: context.saga_origin.triggering_aggregate.clone(),
        source_event_sequence: context.saga_origin.triggering_event_sequence,
    }
}

/// Build a Notification wrapping a RejectionNotification.
///
/// This is the new pattern for compensation - Notification with typed payload.
pub fn build_notification(context: &CompensationContext) -> Notification {
    let rejection = build_rejection_notification(context);

    // Build cover from triggering aggregate
    let cover = context.saga_origin.triggering_aggregate.clone();

    Notification {
        cover,
        payload: Some(prost_types::Any {
            type_url: type_url::REJECTION_NOTIFICATION.to_string(),
            value: rejection.encode_to_vec(),
        }),
        sent_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
        metadata: std::collections::HashMap::new(),
    }
}

/// Build a CommandBook to send the Notification to the triggering aggregate.
pub fn build_notification_command_book(context: &CompensationContext) -> Result<CommandBook> {
    let triggering_aggregate = context
        .saga_origin
        .triggering_aggregate
        .as_ref()
        .ok_or(CompensationError::MissingTriggeringAggregate)?;

    let notification = build_notification(context);

    // Clone triggering aggregate and set correlation_id on cover
    let mut cover = triggering_aggregate.clone();
    if cover.correlation_id.is_empty() {
        cover.correlation_id = context.correlation_id.clone();
    }

    Ok(CommandBook {
        cover: Some(cover),
        pages: vec![crate::proto::CommandPage {
            sequence: 0,
            payload: Some(crate::proto::command_page::Payload::Command(
                prost_types::Any {
                    type_url: type_url::NOTIFICATION.to_string(),
                    value: notification.encode_to_vec(),
                },
            )),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
        }],
        saga_origin: None, // Notifications don't have their own saga origin
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
            external_id: String::new(),
        }),
        pages: vec![EventPage {
            sequence_type: Some(crate::proto::event_page::SequenceType::Sequence(0)),
            created_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
            payload: Some(crate::proto::event_page::Payload::Event(prost_types::Any {
                type_url: type_url::SAGA_COMPENSATION_FAILED.to_string(),
                value: event.encode_to_vec(),
            })),
        }],
        snapshot: None,
        ..Default::default()
    }
}

/// Outcome of handling a business response to a rejection Notification.
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

/// Handle a BusinessResponse to a rejection Notification.
///
/// Implements the decision logic for processing revocation responses:
/// 1. If business returns events with pages → use them
/// 2. If business returns RevocationResponse → process flags
/// 3. If empty/error → use config-based fallback
///
/// Returns actions to take (emit events, escalate, etc.)
pub async fn handle_business_response(
    response: std::result::Result<BusinessResponse, tonic::Status>,
    context: &CompensationContext,
    config: &SagaCompensationConfig,
    escalation_handler: &dyn EscalationHandler,
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
            #[cfg(feature = "otel")]
            {
                use crate::advice::metrics::{self, SAGA_COMPENSATION_TOTAL};
                SAGA_COMPENSATION_TOTAL
                    .add(1, &[metrics::name_attr(&context.saga_origin.saga_name)]);
            }
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
    process_revocation_flags(&revocation, context, config, escalation_handler).await
}

/// Process RevocationResponse flags and take appropriate actions.
async fn process_revocation_flags(
    revocation: &RevocationResponse,
    context: &CompensationContext,
    config: &SagaCompensationConfig,
    escalation_handler: &dyn EscalationHandler,
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

    // Quarantine if requested (for later reprocessing)
    if revocation.send_to_dead_letter_queue {
        if let Err(e) = escalation_handler
            .quarantine(context, &revocation.reason)
            .await
        {
            error!(error = %e, "Failed to quarantine");
            // Continue processing other flags even if quarantine fails
        }
    }

    // Notify if requested (for human intervention)
    if revocation.escalate {
        if let Err(e) = escalation_handler.notify(context, &revocation.reason).await {
            error!(error = %e, "Failed to notify");
            // Continue processing other flags even if notification fails
        }
    }

    // Check abort flag - it takes precedence over other outcomes
    if revocation.abort {
        return Err(CompensationError::Aborted(revocation.reason.clone()));
    }

    // Emit system revocation event if requested
    if revocation.emit_system_revocation {
        let event_book = build_compensation_failed_event_book(context, &revocation.reason, config);
        #[cfg(feature = "otel")]
        {
            use crate::advice::metrics::{self, SAGA_COMPENSATION_TOTAL};
            SAGA_COMPENSATION_TOTAL.add(1, &[metrics::name_attr(&context.saga_origin.saga_name)]);
        }
        return Ok(CompensationOutcome::EmitSystemRevocation(event_book));
    }

    // No flags set - declined, just log
    Ok(CompensationOutcome::Declined {
        reason: revocation.reason.clone(),
    })
}

/// Process compensation response from coordinator and handle all outcomes.
///
/// This is the shared entry point for saga compensation - both gRPC and local
/// modes call this after getting the BusinessResponse from the coordinator.
/// Handles event persistence acknowledgment, system revocation emission,
/// escalation (quarantine/notify), and logging.
pub async fn process_compensation_response(
    response: std::result::Result<crate::proto::BusinessResponse, tonic::Status>,
    context: &CompensationContext,
    config: &SagaCompensationConfig,
    event_bus: &std::sync::Arc<dyn crate::bus::EventBus>,
    saga_name: &str,
    triggering_domain: &str,
) {
    let escalation_handler = DefaultEscalationHandler::new(event_bus.clone(), config.clone());

    let outcome = handle_business_response(response, context, config, &escalation_handler).await;

    match outcome {
        Ok(CompensationOutcome::Events(events)) => {
            // Business provided compensation events - already persisted by HandleCompensation
            info!(
                saga = %saga_name,
                triggering_domain = %triggering_domain,
                events = events.pages.len(),
                "Compensation events recorded successfully"
            );
        }
        Ok(CompensationOutcome::EmitSystemRevocation(event_book)) => {
            info!(
                saga = %saga_name,
                triggering_domain = %triggering_domain,
                "Emitting system revocation event"
            );
            if let Err(e) = event_bus.publish(std::sync::Arc::new(event_book)).await {
                error!(
                    saga = %saga_name,
                    error = %e,
                    "Failed to publish system revocation event"
                );
            }
        }
        Ok(CompensationOutcome::Declined { reason }) => {
            debug!(
                saga = %saga_name,
                triggering_domain = %triggering_domain,
                reason = %reason,
                "Compensation declined by business logic"
            );
        }
        Ok(CompensationOutcome::Aborted { reason }) => {
            error!(
                saga = %saga_name,
                triggering_domain = %triggering_domain,
                reason = %reason,
                "Compensation aborted by business logic - saga chain stopped"
            );
        }
        Err(CompensationError::Aborted(reason)) => {
            error!(
                saga = %saga_name,
                triggering_domain = %triggering_domain,
                reason = %reason,
                "Compensation aborted - saga chain stopped"
            );
        }
        Err(e) => {
            error!(
                saga = %saga_name,
                triggering_domain = %triggering_domain,
                error = %e,
                "Compensation failed"
            );
        }
    }
}

#[cfg(test)]
#[path = "mod.test.rs"]
mod tests;
