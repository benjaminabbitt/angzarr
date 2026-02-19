//! gRPC service handlers for aggregates, sagas, and process managers.
//!
//! This module provides gRPC service implementations that wrap command/event routers.

use std::sync::Arc;

use prost_types::Any;
use tonic::{Request, Response, Status};

use crate::proto::{
    aggregate_service_server::AggregateService,
    process_manager_service_server::ProcessManagerService,
    projector_service_server::ProjectorService, saga_service_server::SagaService, BusinessResponse,
    ContextualCommand, EventBook, ProcessManagerHandleRequest, ProcessManagerHandleResponse,
    ProcessManagerPrepareRequest, ProcessManagerPrepareResponse, Projection, ReplayRequest,
    ReplayResponse, SagaExecuteRequest, SagaPrepareRequest, SagaPrepareResponse, SagaResponse,
};
use crate::router::{CommandRouter, EventRouter, ProcessManagerRouter};

/// Function type for packing state into protobuf Any.
///
/// Used by Replay RPC to return state as a serializable message.
pub type StatePacker<S> = fn(&S) -> Result<Any, Status>;

/// gRPC aggregate service implementation.
///
/// Wraps a `CommandRouter` to handle aggregate commands.
/// Optionally supports Replay RPC for MERGE_COMMUTATIVE conflict detection.
pub struct AggregateHandler<S: Send + Sync + 'static> {
    router: Arc<CommandRouter<S>>,
    /// Optional state packer for Replay RPC support.
    /// When set, enables computing state from events for commutative merge detection.
    state_packer: Option<StatePacker<S>>,
}

impl<S: Send + Sync + 'static> AggregateHandler<S> {
    /// Create a new aggregate handler from a command router.
    pub fn new(router: CommandRouter<S>) -> Self {
        Self {
            router: Arc::new(router),
            state_packer: None,
        }
    }

    /// Enable Replay RPC support by providing a state packer.
    ///
    /// The state packer converts the aggregate's internal state to a protobuf `Any`
    /// message. This is required for MERGE_COMMUTATIVE strategy, which uses Replay
    /// to compute state diffs for conflict detection.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn pack_player_state(state: &PlayerState) -> Result<Any, Status> {
    ///     let proto_state = state.to_proto();
    ///     let mut buf = Vec::new();
    ///     proto_state.encode(&mut buf).map_err(|e| Status::internal(e.to_string()))?;
    ///     Ok(Any {
    ///         type_url: "type.googleapis.com/examples.PlayerState".to_string(),
    ///         value: buf,
    ///     })
    /// }
    ///
    /// let handler = AggregateHandler::new(router)
    ///     .with_replay(pack_player_state);
    /// ```
    pub fn with_replay(mut self, packer: StatePacker<S>) -> Self {
        self.state_packer = Some(packer);
        self
    }

    /// Get the underlying router.
    pub fn router(&self) -> &CommandRouter<S> {
        &self.router
    }
}

#[tonic::async_trait]
impl<S: Send + Sync + 'static> AggregateService for AggregateHandler<S> {
    async fn handle(
        &self,
        request: Request<ContextualCommand>,
    ) -> Result<Response<BusinessResponse>, Status> {
        let cmd = request.into_inner();
        let response = self.router.dispatch(&cmd)?;
        Ok(Response::new(response))
    }

    async fn replay(
        &self,
        request: Request<ReplayRequest>,
    ) -> Result<Response<ReplayResponse>, Status> {
        // Replay is optional - only needed for MERGE_COMMUTATIVE conflict detection.
        let packer = self.state_packer.ok_or_else(|| {
            Status::unimplemented(
                "Replay not implemented. Call with_replay() to enable for MERGE_COMMUTATIVE strategy.",
            )
        })?;

        let req = request.into_inner();

        // Build EventBook from ReplayRequest
        let event_book = build_event_book_for_replay(&req);

        // Rebuild state using the router's state rebuilder
        let state = self.router.rebuild_state(&event_book);

        // Pack state to Any
        let state_any = packer(&state)?;

        Ok(Response::new(ReplayResponse {
            state: Some(state_any),
        }))
    }
}

/// Build an EventBook from a ReplayRequest for state reconstruction.
fn build_event_book_for_replay(req: &ReplayRequest) -> EventBook {
    // Convert ReplayRequest.events (Vec<EventPage>) to EventBook
    // Include base_snapshot if provided
    EventBook {
        cover: None,
        pages: req.events.clone(),
        snapshot: req.base_snapshot.clone(),
        next_sequence: 0,
    }
}

/// gRPC saga service implementation.
///
/// Wraps an `EventRouter` to handle saga events.
pub struct SagaHandler {
    router: Arc<EventRouter>,
}

impl SagaHandler {
    /// Create a new saga handler from an event router.
    pub fn new(router: EventRouter) -> Self {
        Self {
            router: Arc::new(router),
        }
    }

    /// Get the underlying router.
    pub fn router(&self) -> &EventRouter {
        &self.router
    }
}

#[tonic::async_trait]
impl SagaService for SagaHandler {
    async fn prepare(
        &self,
        request: Request<SagaPrepareRequest>,
    ) -> Result<Response<SagaPrepareResponse>, Status> {
        let req = request.into_inner();

        // Get destinations from the router (for now, return empty)
        // TODO: Allow handlers to declare destinations
        let destinations = self.router.prepare_destinations(&req.source);

        Ok(Response::new(SagaPrepareResponse { destinations }))
    }

    async fn execute(
        &self,
        request: Request<SagaExecuteRequest>,
    ) -> Result<Response<SagaResponse>, Status> {
        let req = request.into_inner();

        let source = req
            .source
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing source event book"))?;

        let commands = self.router.dispatch(source, &req.destinations)?;
        Ok(Response::new(SagaResponse {
            commands,
            events: vec![],
        }))
    }
}

/// Handle function type for projectors (function pointer).
pub type ProjectorHandleFn = fn(&EventBook) -> Result<Projection, Status>;

/// Handle closure type for projectors.
pub type ProjectorHandleClosureFn =
    Arc<dyn Fn(&EventBook) -> Result<Projection, Status> + Send + Sync>;

/// Internal handle type - either fn pointer or closure.
enum ProjectorHandleType {
    Fn(ProjectorHandleFn),
    Closure(ProjectorHandleClosureFn),
}

/// gRPC projector service implementation.
///
/// Wraps a handle function to process events and produce projections.
pub struct ProjectorHandler {
    name: String,
    handle_type: Option<ProjectorHandleType>,
}

impl ProjectorHandler {
    /// Create a new projector handler.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            handle_type: None,
        }
    }

    /// Set the handle function (function pointer).
    pub fn with_handle(mut self, handle_fn: ProjectorHandleFn) -> Self {
        self.handle_type = Some(ProjectorHandleType::Fn(handle_fn));
        self
    }

    /// Set the handle function (closure).
    pub fn with_handle_fn<H>(mut self, handle_fn: H) -> Self
    where
        H: Fn(&EventBook) -> Result<Projection, Status> + Send + Sync + 'static,
    {
        self.handle_type = Some(ProjectorHandleType::Closure(Arc::new(handle_fn)));
        self
    }

    /// Get the projector name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[tonic::async_trait]
impl ProjectorService for ProjectorHandler {
    async fn handle(&self, request: Request<EventBook>) -> Result<Response<Projection>, Status> {
        let event_book = request.into_inner();
        match &self.handle_type {
            Some(ProjectorHandleType::Fn(handle_fn)) => {
                let projection = handle_fn(&event_book)?;
                Ok(Response::new(projection))
            }
            Some(ProjectorHandleType::Closure(handle_fn)) => {
                let projection = handle_fn(&event_book)?;
                Ok(Response::new(projection))
            }
            None => Ok(Response::new(Projection::default())),
        }
    }

    async fn handle_speculative(
        &self,
        request: Request<EventBook>,
    ) -> Result<Response<Projection>, Status> {
        self.handle(request).await
    }
}

/// gRPC process manager service implementation.
///
/// Wraps a `ProcessManagerRouter` to handle PM events.
pub struct ProcessManagerGrpcHandler<S: Send + Sync + 'static> {
    router: Arc<ProcessManagerRouter<S>>,
}

impl<S: Send + Sync + 'static> ProcessManagerGrpcHandler<S> {
    /// Create a new process manager handler from a router.
    pub fn new(router: ProcessManagerRouter<S>) -> Self {
        Self {
            router: Arc::new(router),
        }
    }

    /// Get the underlying router.
    pub fn router(&self) -> &ProcessManagerRouter<S> {
        &self.router
    }
}

#[tonic::async_trait]
impl<S: Send + Sync + 'static> ProcessManagerService for ProcessManagerGrpcHandler<S> {
    async fn prepare(
        &self,
        request: Request<ProcessManagerPrepareRequest>,
    ) -> Result<Response<ProcessManagerPrepareResponse>, Status> {
        let req = request.into_inner();
        let destinations = self
            .router
            .prepare_destinations(&req.trigger, &req.process_state);

        Ok(Response::new(ProcessManagerPrepareResponse {
            destinations,
        }))
    }

    async fn handle(
        &self,
        request: Request<ProcessManagerHandleRequest>,
    ) -> Result<Response<ProcessManagerHandleResponse>, Status> {
        let req = request.into_inner();

        let trigger = req
            .trigger
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing trigger event book"))?;

        let process_state = req.process_state.as_ref().cloned().unwrap_or_default();

        let response = self
            .router
            .dispatch(trigger, &process_state, &req.destinations)?;

        Ok(Response::new(ProcessManagerHandleResponse {
            commands: response.commands,
            process_events: response.process_events,
        }))
    }
}
