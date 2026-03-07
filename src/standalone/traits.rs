//! Handler traits for standalone mode.
//!
//! Users implement these traits to provide client logic, projectors, and sagas.

use async_trait::async_trait;
use tonic::Status;

use crate::proto::{
    BusinessResponse, CommandBook, ContextualCommand, Cover, EventBook, Notification,
    RejectionNotification, RevocationResponse, SagaResponse,
};

/// Context for fact event handling.
///
/// Contains the fact events to record and the aggregate's prior events.
#[derive(Debug, Clone)]
pub struct FactContext {
    /// The fact events to record (with ExternalDeferredSequence markers in PageHeader).
    pub facts: EventBook,
    /// Prior events for this aggregate root (for state reconstruction).
    pub prior_events: Option<EventBook>,
}

/// Result of process manager handle phase.
///
/// Contains commands, PM events, and facts to inject to other aggregates.
#[derive(Debug, Clone, Default)]
pub struct ProcessManagerHandleResult {
    /// Commands to send to other aggregates.
    pub commands: Vec<CommandBook>,
    /// Events to persist to the PM's own domain.
    pub process_events: Option<EventBook>,
    /// Facts to inject to other aggregates.
    pub facts: Vec<EventBook>,
}

/// Command handler for a domain aggregate.
///
/// Implement this trait to handle commands for a specific domain.
/// The handler receives a contextual command (command + prior events)
/// and returns new events to persist.
///
/// # Example
///
/// ```ignore
/// use angzarr::standalone::CommandHandler;
/// use angzarr::proto::{ContextualCommand, EventBook};
///
/// struct OrdersHandler;
///
/// #[async_trait::async_trait]
/// impl CommandHandler for OrdersHandler {
///     async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, tonic::Status> {
///         // Rebuild state from prior events
///         let state = rebuild_state(&ctx.existing);
///
///         // Validate command and produce new events
///         let events = match ctx.command.as_ref() {
///             Some(cmd) => process_command(cmd, &state)?,
///             None => return Err(tonic::Status::invalid_argument("missing command")),
///         };
///
///         Ok(events)
///     }
/// }
/// ```
#[async_trait]
pub trait CommandHandler: Send + Sync + 'static {
    /// Handle a contextual command and return new events.
    async fn handle(&self, command: ContextualCommand) -> Result<EventBook, Status>;

    /// Handle fact events from external systems.
    ///
    /// Called when fact events (with ExternalDeferredSequence markers) are injected into
    /// the aggregate. Facts represent external realities that cannot be rejected.
    /// The aggregate should update its state to reflect these facts.
    ///
    /// The coordinator will:
    /// 1. Check idempotency via `PageHeader.external_deferred.external_id`
    /// 2. Call this method to let aggregate update state
    /// 3. Assign real sequence numbers (replacing ExternalDeferredSequence markers)
    /// 4. Persist and publish the events
    ///
    /// Return events to persist. The events SHOULD have ExternalDeferredSequence markers
    /// (same as input) - the coordinator will replace them with real sequences.
    ///
    /// Default: Returns the input facts unchanged (pass-through).
    ///
    /// # Example
    ///
    /// ```ignore
    /// async fn handle_fact(&self, ctx: FactContext) -> Result<EventBook, Status> {
    ///     // Rebuild state from prior events
    ///     let state = rebuild_state(&ctx.prior_events);
    ///
    ///     // Process facts - update state, possibly emit additional events
    ///     let mut events = ctx.facts.clone();
    ///     for page in &ctx.facts.pages {
    ///         // Update internal state based on fact
    ///         // Optionally add derived events
    ///     }
    ///
    ///     Ok(events)
    /// }
    /// ```
    async fn handle_fact(&self, ctx: FactContext) -> Result<EventBook, Status> {
        // Default: pass through facts unchanged
        Ok(ctx.facts)
    }

    /// Replay events to compute state for COMMUTATIVE merge detection.
    ///
    /// Called by the coordinator when COMMUTATIVE strategy encounters a sequence
    /// mismatch. The coordinator compares states at different sequences to detect
    /// whether changes overlap (conflict) or are disjoint (commutative).
    ///
    /// Return the aggregate's internal state packed as a protobuf `Any` message.
    /// The state message should have fields that can be diffed for overlap detection.
    ///
    /// Default: Returns Unimplemented, causing COMMUTATIVE to degrade to STRICT behavior.
    ///
    /// # Example
    ///
    /// ```ignore
    /// async fn replay(&self, events: &EventBook) -> Result<prost_types::Any, Status> {
    ///     let state = rebuild_state_from_events(events);
    ///     let proto_state = state.to_proto();
    ///     let mut buf = Vec::new();
    ///     proto_state.encode(&mut buf).unwrap();
    ///     Ok(prost_types::Any {
    ///         type_url: "type.googleapis.com/examples.MyState".to_string(),
    ///         value: buf,
    ///     })
    /// }
    /// ```
    async fn replay(&self, _events: &EventBook) -> Result<prost_types::Any, Status> {
        Err(Status::unimplemented(
            "Replay not implemented. Override replay() to enable MERGE_COMMUTATIVE field detection.",
        ))
    }

    /// Handle a rejection notification.
    ///
    /// Called when a saga/PM command is rejected and compensation is needed.
    /// Override to provide custom compensation logic (emit compensation events).
    ///
    /// Default behavior: request framework to emit SagaCompensationFailed event.
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn handle_revocation(&self, notification: &Notification) -> BusinessResponse {
    ///     // Unpack rejection details
    ///     let rejection = RejectionNotification::decode(
    ///         notification.payload.as_ref().unwrap().value.as_slice()
    ///     ).unwrap();
    ///
    ///     // Custom compensation: emit events
    ///     let event = OrderCancelled {
    ///         reason: format!("{} failed: {}", rejection.issuer_name, rejection.rejection_reason),
    ///         ..Default::default()
    ///     };
    ///     BusinessResponse {
    ///         result: Some(business_response::Result::Events(pack_events(vec![event]))),
    ///     }
    /// }
    /// ```
    fn handle_revocation(&self, notification: &Notification) -> BusinessResponse {
        let source_domain = extract_source_domain(notification);
        BusinessResponse {
            result: Some(crate::proto::business_response::Result::Revocation(
                build_command_handler_revocation_response(&source_domain),
            )),
        }
    }
}

pub use crate::orchestration::projector::{ProjectionMode, ProjectorHandler};

/// Saga handler for stateless cross-domain translation.
///
/// Sagas are **pure translators**: they receive source events and produce commands
/// for target domains. They do NOT receive destination state — the framework handles
/// sequence stamping and delivery retries.
///
/// # Contract
///
/// - **Input**: Source EventBook (events from one domain)
/// - **Output**: SagaResponse with commands (for target domains) and facts (for injection)
/// - **Sequences**: Commands use `angzarr_deferred` (framework stamps explicit sequences on delivery)
/// - **Stateless**: Each event is processed independently with no memory of previous events
///
/// # Example
///
/// ```ignore
/// use angzarr::standalone::SagaHandler;
/// use angzarr::proto::{EventBook, SagaResponse};
///
/// struct FulfillmentSaga;
///
/// #[async_trait::async_trait]
/// impl SagaHandler for FulfillmentSaga {
///     async fn handle(&self, source: &EventBook) -> Result<SagaResponse, tonic::Status> {
///         let mut commands = Vec::new();
///         for page in &source.pages {
///             if is_order_placed(&page.event) {
///                 // Command will have angzarr_deferred set by framework
///                 commands.push(create_shipment_command(&page.event));
///             }
///         }
///         Ok(SagaResponse { commands, ..Default::default() })
///     }
/// }
/// ```
///
/// # Framework Responsibilities
///
/// The framework handles:
/// 1. **Sequence stamping**: Converts `angzarr_deferred` to explicit sequences on delivery
/// 2. **Delivery retry**: Retries command delivery on sequence conflict (not saga re-execution)
/// 3. **Idempotency**: Uses `angzarr_deferred` source info as idempotency key
/// 4. **Provenance tracking**: Links commands back to source event for compensation routing
#[async_trait]
pub trait SagaHandler: Send + Sync + 'static {
    /// Translate source events into commands for target domains.
    ///
    /// Commands should have `cover` set to identify the target aggregate.
    /// The framework will stamp `angzarr_deferred` with source info for:
    /// - Provenance tracking (which event triggered this command)
    /// - Compensation routing (where to send rejections)
    /// - Idempotency (prevent duplicate processing on retry)
    ///
    /// Return empty commands vec if saga doesn't act on this event (no-op).
    async fn handle(&self, source: &EventBook) -> Result<SagaResponse, Status>;
}

/// Configuration for a projector.
#[derive(Debug, Clone, Default)]
pub struct ProjectorConfig {
    /// Whether this projector should block command response.
    ///
    /// Synchronous projectors must complete before the command returns.
    /// This is useful for projectors that produce data needed by the client.
    pub synchronous: bool,

    /// Domains to subscribe to.
    ///
    /// If empty, subscribes to all domains.
    pub domains: Vec<String>,
}

impl ProjectorConfig {
    /// Create a synchronous projector config.
    pub fn sync() -> Self {
        Self {
            synchronous: true,
            ..Default::default()
        }
    }

    /// Create an asynchronous projector config.
    pub fn async_() -> Self {
        Self {
            synchronous: false,
            ..Default::default()
        }
    }

    /// Subscribe to specific domains.
    pub fn with_domains(mut self, domains: Vec<String>) -> Self {
        self.domains = domains;
        self
    }
}

/// Configuration for a saga.
#[derive(Debug, Clone)]
pub struct SagaConfig {
    /// Domain to subscribe to for events.
    pub input_domain: String,
    /// Domains that saga commands may target.
    pub output_domains: Vec<String>,
}

impl SagaConfig {
    /// Create a saga config with a single output domain.
    pub fn new(input_domain: impl Into<String>, output_domain: impl Into<String>) -> Self {
        Self {
            input_domain: input_domain.into(),
            output_domains: vec![output_domain.into()],
        }
    }

    /// Add an additional output domain.
    pub fn with_output(mut self, domain: impl Into<String>) -> Self {
        self.output_domains.push(domain.into());
        self
    }
}

/// Process manager handler for stateful cross-domain coordination.
///
/// Process managers ARE aggregates — they have their own domain, event-sourced state,
/// and storage. The runtime triggers PM logic when matching events arrive on the bus,
/// persists PM events to the PM's aggregate domain, and executes resulting commands.
///
/// # Two-Phase Protocol
///
/// Phase 1 (`prepare`): PM examines trigger event + its own state, declares
/// additional destination aggregates it needs. Return empty if PM only uses its own state.
///
/// Phase 2 (`handle`): PM receives trigger + PM state + fetched destinations,
/// returns commands to issue and PM events to persist.
///
/// # Compensation
///
/// When a PM's command is rejected by the target aggregate, `handle_revocation()`
/// is called. The PM can emit compensation events to its own domain or delegate
/// to framework handling. Override to provide custom compensation logic.
///
/// # Example
///
/// ```ignore
/// use angzarr::standalone::ProcessManagerHandler;
/// use angzarr::proto::{CommandBook, Cover, EventBook};
///
/// struct OrderFulfillmentPM;
///
/// impl ProcessManagerHandler for OrderFulfillmentPM {
///     fn prepare(&self, _trigger: &EventBook, _state: Option<&EventBook>) -> Vec<Cover> {
///         vec![] // Only needs PM state
///     }
///
///     fn handle(
///         &self,
///         trigger: &EventBook,
///         process_state: Option<&EventBook>,
///         _destinations: &[EventBook],
///     ) -> ProcessManagerHandleResult {
///         // Pure computation: examine trigger + state, produce commands + PM events + facts
///         ProcessManagerHandleResult::default()
///     }
/// }
/// ```
pub trait ProcessManagerHandler: Send + Sync + 'static {
    /// Phase 1: Declare additional destinations needed beyond trigger + PM state.
    ///
    /// Returns destinations to fetch. Most PMs return empty (only need PM state).
    fn prepare(&self, trigger: &EventBook, process_state: Option<&EventBook>) -> Vec<Cover>;

    /// Phase 2: Produce commands, PM events, and facts given trigger, PM state, and destinations.
    ///
    /// Returns commands to execute, optional PM events to persist, and facts to inject.
    fn handle(
        &self,
        trigger: &EventBook,
        process_state: Option<&EventBook>,
        destinations: &[EventBook],
    ) -> ProcessManagerHandleResult;

    /// Handle a rejection notification for commands this PM issued.
    ///
    /// Called when a command produced by this PM is rejected by the target aggregate.
    /// Override to provide custom compensation logic (emit PM events to record
    /// the failed workflow step).
    ///
    /// Default behavior: request framework to emit SagaCompensationFailed event.
    ///
    /// # Arguments
    ///
    /// * `notification` - The notification with RejectionNotification payload
    /// * `process_state` - Current PM state for this correlation_id
    ///
    /// # Returns
    ///
    /// Tuple of (optional PM events to persist, RevocationResponse for framework).
    /// Return events to record compensation in PM state. Return RevocationResponse
    /// to delegate to framework handling.
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn handle_revocation(
    ///     &self,
    ///     notification: &Notification,
    ///     process_state: Option<&EventBook>,
    /// ) -> (Option<EventBook>, RevocationResponse) {
    ///     // Unpack rejection details
    ///     let rejection = RejectionNotification::decode(
    ///         notification.payload.as_ref().unwrap().value.as_slice()
    ///     ).unwrap();
    ///
    ///     // Record the failure in PM state
    ///     let event = WorkflowStepFailed {
    ///         issuer_name: rejection.issuer_name.clone(),
    ///         reason: rejection.rejection_reason.clone(),
    ///     };
    ///     let events = pack_events(vec![event]);
    ///
    ///     // Also delegate to framework for system tracking
    ///     (Some(events), RevocationResponse {
    ///         emit_system_revocation: true,
    ///         reason: format!("PM recorded failure for {}", rejection.issuer_name),
    ///         ..Default::default()
    ///     })
    /// }
    /// ```
    fn handle_revocation(
        &self,
        notification: &Notification,
        _process_state: Option<&EventBook>,
    ) -> (Option<EventBook>, RevocationResponse) {
        let source_domain = extract_source_domain(notification);
        // Default: no PM events, delegate to framework
        (None, build_pm_revocation_response(&source_domain))
    }
}

/// Configuration for a process manager.
#[derive(Debug, Clone)]
pub struct ProcessManagerConfig {
    /// The PM's own aggregate domain for state storage.
    pub domain: String,
    /// Subscriptions: which domains/event types this PM listens to.
    pub subscriptions: Vec<crate::descriptor::Target>,
}

impl ProcessManagerConfig {
    /// Create a process manager config with the PM's domain.
    pub fn new(domain: impl Into<String>) -> Self {
        Self {
            domain: domain.into(),
            subscriptions: vec![],
        }
    }

    /// Add subscriptions for this process manager.
    pub fn with_subscriptions(mut self, subscriptions: Vec<crate::descriptor::Target>) -> Self {
        self.subscriptions = subscriptions;
        self
    }
}

// ============================================================================
// Pure Helper Functions (testable without infrastructure)
// ============================================================================

/// Extract source domain from a notification payload.
///
/// Returns the source domain from the rejected command's angzarr_deferred header,
/// or "unknown" if the notification payload cannot be decoded.
pub(crate) fn extract_source_domain(notification: &Notification) -> String {
    use crate::proto::page_header::SequenceType;

    notification
        .payload
        .as_ref()
        .and_then(|p| {
            use prost::Message;
            RejectionNotification::decode(p.value.as_slice()).ok()
        })
        .and_then(|r| r.rejected_command)
        .and_then(|cmd| cmd.pages.first().cloned())
        .and_then(|page| page.header)
        .and_then(|h| h.sequence_type)
        .and_then(|st| match st {
            SequenceType::AngzarrDeferred(ad) => ad.source.map(|c| c.domain),
            _ => None,
        })
        .unwrap_or_else(|| "unknown".to_string())
}

/// Build a default revocation response for command handlers.
///
/// Returns a RevocationResponse requesting framework to emit system revocation event.
pub(crate) fn build_command_handler_revocation_response(issuer_name: &str) -> RevocationResponse {
    RevocationResponse {
        emit_system_revocation: true,
        reason: format!(
            "CommandHandler has no custom compensation for {}",
            issuer_name
        ),
        ..Default::default()
    }
}

/// Build a default revocation response for process managers.
///
/// Returns a RevocationResponse requesting framework to emit system revocation event.
pub(crate) fn build_pm_revocation_response(issuer_name: &str) -> RevocationResponse {
    RevocationResponse {
        emit_system_revocation: true,
        reason: format!(
            "ProcessManager has no custom compensation for {}",
            issuer_name
        ),
        ..Default::default()
    }
}
