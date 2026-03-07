//! gRPC service handlers for aggregates, sagas, and process managers.
//!
//! This module provides gRPC service implementations that wrap routers.

use std::sync::Arc;

use prost_types::Any;
use tonic::{Request, Response, Status};

use crate::proto::{
    command_handler_service_server::CommandHandlerService,
    process_manager_service_server::ProcessManagerService,
    projector_service_server::ProjectorService, saga_service_server::SagaService,
    upcaster_service_server::UpcasterService, BusinessResponse, ContextualCommand, EventBook,
    ProcessManagerHandleRequest, ProcessManagerHandleResponse, ProcessManagerPrepareRequest,
    ProcessManagerPrepareResponse, Projection, ReplayRequest, ReplayResponse, SagaHandleRequest,
    SagaResponse, UpcastRequest, UpcastResponse,
};
use crate::router::{
    CloudEventsRouter, CommandHandlerDomainHandler, CommandHandlerRouter, ProcessManagerRouter,
    SagaDomainHandler, SagaRouter,
};

/// Function type for packing state into protobuf Any.
///
/// Used by Replay RPC to return state as a serializable message.
pub type StatePacker<S> = fn(&S) -> Result<Any, Status>;

/// gRPC command handler service implementation.
///
/// Wraps a `CommandHandlerRouter` to handle commands.
/// Optionally supports Replay RPC for MERGE_COMMUTATIVE conflict detection.
pub struct CommandHandlerGrpc<S, H>
where
    S: Default + Send + Sync + 'static,
    H: CommandHandlerDomainHandler<State = S> + 'static,
{
    router: Arc<CommandHandlerRouter<S, H>>,
    /// Optional state packer for Replay RPC support.
    state_packer: Option<StatePacker<S>>,
}

impl<S, H> CommandHandlerGrpc<S, H>
where
    S: Default + Send + Sync + 'static,
    H: CommandHandlerDomainHandler<State = S> + 'static,
{
    /// Create a new command handler from a router.
    pub fn new(router: CommandHandlerRouter<S, H>) -> Self {
        Self {
            router: Arc::new(router),
            state_packer: None,
        }
    }

    /// Enable Replay RPC support by providing a state packer.
    pub fn with_replay(mut self, packer: StatePacker<S>) -> Self {
        self.state_packer = Some(packer);
        self
    }

    /// Get the underlying router.
    pub fn router(&self) -> &CommandHandlerRouter<S, H> {
        &self.router
    }
}

#[tonic::async_trait]
impl<S, H> CommandHandlerService for CommandHandlerGrpc<S, H>
where
    S: Default + Send + Sync + 'static,
    H: CommandHandlerDomainHandler<State = S> + 'static,
{
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
        let packer = self.state_packer.ok_or_else(|| {
            Status::unimplemented(
                "Replay not implemented. Call with_replay() to enable for MERGE_COMMUTATIVE strategy.",
            )
        })?;

        let req = request.into_inner();
        let event_book = build_event_book_for_replay(&req);
        let state = self.router.rebuild_state(&event_book);
        let state_any = packer(&state)?;

        Ok(Response::new(ReplayResponse {
            state: Some(state_any),
        }))
    }
}

/// Build an EventBook from a ReplayRequest for state reconstruction.
fn build_event_book_for_replay(req: &ReplayRequest) -> EventBook {
    EventBook {
        cover: None,
        pages: req.events.clone(),
        snapshot: req.base_snapshot.clone(),
        next_sequence: 0,
    }
}

/// gRPC saga service implementation.
///
/// Wraps a `SagaRouter` to handle saga events.
pub struct SagaHandler<H>
where
    H: SagaDomainHandler + 'static,
{
    router: Arc<SagaRouter<H>>,
}

impl<H: SagaDomainHandler + 'static> SagaHandler<H> {
    /// Create a new saga handler from a router.
    pub fn new(router: SagaRouter<H>) -> Self {
        Self {
            router: Arc::new(router),
        }
    }

    /// Get the underlying router.
    pub fn router(&self) -> &SagaRouter<H> {
        &self.router
    }
}

#[tonic::async_trait]
impl<H: SagaDomainHandler + 'static> SagaService for SagaHandler<H> {
    async fn handle(
        &self,
        request: Request<SagaHandleRequest>,
    ) -> Result<Response<SagaResponse>, Status> {
        let req = request.into_inner();
        let source = req
            .source
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing source event book"))?;

        let response = self.router.dispatch(source)?;
        Ok(Response::new(response))
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
pub struct ProcessManagerGrpcHandler<S: Default + Send + Sync + 'static> {
    router: Arc<ProcessManagerRouter<S>>,
}

impl<S: Default + Send + Sync + 'static> ProcessManagerGrpcHandler<S> {
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
impl<S: Default + Send + Sync + 'static> ProcessManagerService for ProcessManagerGrpcHandler<S> {
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

        Ok(Response::new(response))
    }
}

/// Handle function type for upcasters (function pointer).
pub type UpcasterHandleFn = fn(&[crate::proto::EventPage]) -> Vec<crate::proto::EventPage>;

/// Handle closure type for upcasters.
pub type UpcasterHandleClosureFn =
    Arc<dyn Fn(&[crate::proto::EventPage]) -> Vec<crate::proto::EventPage> + Send + Sync>;

/// Internal handle type - either fn pointer or closure.
enum UpcasterHandleType {
    Fn(UpcasterHandleFn),
    Closure(UpcasterHandleClosureFn),
}

/// gRPC upcaster service implementation.
///
/// Wraps a handle function to transform events to current versions.
pub struct UpcasterGrpcHandler {
    name: String,
    domain: String,
    handle_type: Option<UpcasterHandleType>,
}

impl UpcasterGrpcHandler {
    /// Create a new upcaster handler.
    pub fn new(name: impl Into<String>, domain: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            domain: domain.into(),
            handle_type: None,
        }
    }

    /// Set the handle function (function pointer).
    pub fn with_handle(mut self, handle_fn: UpcasterHandleFn) -> Self {
        self.handle_type = Some(UpcasterHandleType::Fn(handle_fn));
        self
    }

    /// Set the handle function (closure).
    pub fn with_handle_fn<H>(mut self, handle_fn: H) -> Self
    where
        H: Fn(&[crate::proto::EventPage]) -> Vec<crate::proto::EventPage> + Send + Sync + 'static,
    {
        self.handle_type = Some(UpcasterHandleType::Closure(Arc::new(handle_fn)));
        self
    }

    /// Get the upcaster name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the domain this upcaster handles.
    pub fn domain(&self) -> &str {
        &self.domain
    }
}

#[tonic::async_trait]
impl UpcasterService for UpcasterGrpcHandler {
    async fn upcast(
        &self,
        request: Request<UpcastRequest>,
    ) -> Result<Response<UpcastResponse>, Status> {
        let req = request.into_inner();
        let events = req.events;

        let result = match &self.handle_type {
            Some(UpcasterHandleType::Fn(handle_fn)) => handle_fn(&events),
            Some(UpcasterHandleType::Closure(handle_fn)) => handle_fn(&events),
            None => events, // Passthrough if no handler
        };

        Ok(Response::new(UpcastResponse { events: result }))
    }
}

/// gRPC CloudEvents projector service implementation.
///
/// Wraps a `CloudEventsRouter` to transform events into CloudEvents.
/// Uses the standard ProjectorService protocol but returns CloudEventsResponse
/// packed into Projection.projection.
pub struct CloudEventsGrpcHandler {
    router: Arc<CloudEventsRouter>,
}

impl CloudEventsGrpcHandler {
    /// Create a new CloudEvents handler from a router.
    pub fn new(router: CloudEventsRouter) -> Self {
        Self {
            router: Arc::new(router),
        }
    }

    /// Get the underlying router.
    pub fn router(&self) -> &CloudEventsRouter {
        &self.router
    }
}

#[tonic::async_trait]
impl ProjectorService for CloudEventsGrpcHandler {
    async fn handle(&self, request: Request<EventBook>) -> Result<Response<Projection>, Status> {
        let event_book = request.into_inner();
        let response = self.router.project(&event_book);

        // Pack CloudEventsResponse into Projection.projection
        let projection_any =
            Any::from_msg(&response).map_err(|e| Status::internal(format!("Pack error: {}", e)))?;

        Ok(Response::new(Projection {
            cover: event_book.cover,
            projector: self.router.name().to_string(),
            sequence: event_book.next_sequence,
            projection: Some(projection_any),
        }))
    }

    async fn handle_speculative(
        &self,
        request: Request<EventBook>,
    ) -> Result<Response<Projection>, Status> {
        self.handle(request).await
    }
}
