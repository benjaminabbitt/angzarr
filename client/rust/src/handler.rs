//! gRPC service handlers for aggregates, sagas, and process managers.
//!
//! This module provides gRPC service implementations that wrap command/event routers.

use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::proto::{
    aggregate_service_server::AggregateService,
    process_manager_service_server::ProcessManagerService,
    projector_service_server::ProjectorService, saga_service_server::SagaService, BusinessResponse,
    ComponentDescriptor, ContextualCommand, EventBook, GetDescriptorRequest,
    ProcessManagerHandleRequest, ProcessManagerHandleResponse, ProcessManagerPrepareRequest,
    ProcessManagerPrepareResponse, Projection, SagaExecuteRequest, SagaPrepareRequest,
    SagaPrepareResponse, SagaResponse, Target,
};
use crate::router::{CommandRouter, EventRouter, ProcessManagerRouter};

/// gRPC aggregate service implementation.
///
/// Wraps a `CommandRouter` to handle aggregate commands.
pub struct AggregateHandler<S: Send + Sync + 'static> {
    router: Arc<CommandRouter<S>>,
}

impl<S: Send + Sync + 'static> AggregateHandler<S> {
    /// Create a new aggregate handler from a command router.
    pub fn new(router: CommandRouter<S>) -> Self {
        Self {
            router: Arc::new(router),
        }
    }

    /// Get the underlying router.
    pub fn router(&self) -> &CommandRouter<S> {
        &self.router
    }
}

#[tonic::async_trait]
impl<S: Send + Sync + 'static> AggregateService for AggregateHandler<S> {
    async fn get_descriptor(
        &self,
        _request: Request<GetDescriptorRequest>,
    ) -> Result<Response<ComponentDescriptor>, Status> {
        let descriptor = ComponentDescriptor {
            name: self.router.domain().to_string(),
            component_type: "aggregate".to_string(),
            inputs: vec![Target {
                domain: self.router.domain().to_string(),
                types: self.router.command_types(),
            }],
        };
        Ok(Response::new(descriptor))
    }

    async fn handle(
        &self,
        request: Request<ContextualCommand>,
    ) -> Result<Response<BusinessResponse>, Status> {
        let cmd = request.into_inner();
        let response = self.router.dispatch(&cmd)?;
        Ok(Response::new(response))
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
    async fn get_descriptor(
        &self,
        _request: Request<GetDescriptorRequest>,
    ) -> Result<Response<ComponentDescriptor>, Status> {
        let descriptor = ComponentDescriptor {
            name: self.router.name().to_string(),
            component_type: "saga".to_string(),
            inputs: vec![Target {
                domain: self.router.input_domain().to_string(),
                types: self.router.event_types(),
            }],
        };
        Ok(Response::new(descriptor))
    }

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

/// Handle function type for projectors.
pub type ProjectorHandleFn = fn(&EventBook) -> Result<Projection, Status>;

/// gRPC projector service implementation.
///
/// Wraps a handle function to process events and produce projections.
pub struct ProjectorHandler {
    name: String,
    domains: Vec<String>,
    handle_fn: Option<ProjectorHandleFn>,
}

impl ProjectorHandler {
    /// Create a new projector handler.
    pub fn new(name: impl Into<String>, domains: Vec<String>) -> Self {
        Self {
            name: name.into(),
            domains,
            handle_fn: None,
        }
    }

    /// Set the handle function.
    pub fn with_handle(mut self, handle_fn: ProjectorHandleFn) -> Self {
        self.handle_fn = Some(handle_fn);
        self
    }

    /// Get the projector name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[tonic::async_trait]
impl ProjectorService for ProjectorHandler {
    async fn get_descriptor(
        &self,
        _request: Request<GetDescriptorRequest>,
    ) -> Result<Response<ComponentDescriptor>, Status> {
        let inputs: Vec<Target> = self
            .domains
            .iter()
            .map(|d| Target {
                domain: d.clone(),
                types: vec![],
            })
            .collect();

        let descriptor = ComponentDescriptor {
            name: self.name.clone(),
            component_type: "projector".to_string(),
            inputs,
        };
        Ok(Response::new(descriptor))
    }

    async fn handle(&self, request: Request<EventBook>) -> Result<Response<Projection>, Status> {
        let event_book = request.into_inner();
        if let Some(handle_fn) = self.handle_fn {
            let projection = handle_fn(&event_book)?;
            Ok(Response::new(projection))
        } else {
            Ok(Response::new(Projection::default()))
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
    async fn get_descriptor(
        &self,
        _request: Request<GetDescriptorRequest>,
    ) -> Result<Response<ComponentDescriptor>, Status> {
        let inputs: Vec<Target> = self
            .router
            .input_domains()
            .iter()
            .map(|d| Target {
                domain: d.clone(),
                types: self.router.event_types(),
            })
            .collect();

        let descriptor = ComponentDescriptor {
            name: self.router.name().to_string(),
            component_type: "process_manager".to_string(),
            inputs,
        };
        Ok(Response::new(descriptor))
    }

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
