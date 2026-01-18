//! Server bootstrap helpers for example services.
//!
//! Reduces boilerplate by providing generic wrappers and server runners
//! for aggregates, sagas, and projectors.

use std::error::Error;
use std::net::SocketAddr;

use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use angzarr::proto::aggregate_server::{Aggregate, AggregateServer};
use angzarr::proto::saga_server::{Saga as SagaService, SagaServer};
use angzarr::proto::{BusinessResponse, CommandBook, ContextualCommand, EventBook, SagaResponse};

/// Initialize JSON tracing subscriber.
pub fn init_tracing() {
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .json()
        .init();
}

/// Get port from PORT environment variable with a default.
pub fn get_port(default: &str) -> String {
    std::env::var("PORT").unwrap_or_else(|_| default.to_string())
}

// ============================================================================
// Aggregate Support
// ============================================================================

/// Trait for aggregate business logic implementations.
///
/// Business logic receives contextual commands (prior events + new command)
/// and returns a response with new events or rejection.
#[tonic::async_trait]
pub trait AggregateLogic: Send + Sync {
    /// Handle a contextual command and return a business response.
    async fn handle(&self, cmd: ContextualCommand) -> Result<BusinessResponse, Status>;
}

/// Wrapper that adapts an AggregateLogic to the Aggregate gRPC service.
pub struct AggregateWrapper<T> {
    logic: T,
}

impl<T> AggregateWrapper<T> {
    pub fn new(logic: T) -> Self {
        Self { logic }
    }
}

#[tonic::async_trait]
impl<T: AggregateLogic + 'static> Aggregate for AggregateWrapper<T> {
    async fn handle(
        &self,
        request: Request<ContextualCommand>,
    ) -> Result<Response<BusinessResponse>, Status> {
        let cmd = request.into_inner();
        let response = self.logic.handle(cmd).await?;
        Ok(Response::new(response))
    }
}

/// Run an aggregate server with the given business logic.
///
/// # Example
/// ```ignore
/// run_aggregate_server("cart", "50057", CartLogic::new()).await
/// ```
pub async fn run_aggregate_server<T: AggregateLogic + 'static>(
    domain: &str,
    default_port: &str,
    logic: T,
) -> Result<(), Box<dyn Error>> {
    init_tracing();

    let port = get_port(default_port);
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    let service = AggregateWrapper::new(logic);

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<AggregateServer<AggregateWrapper<T>>>()
        .await;

    info!(domain = domain, port = %port, "server_started");

    Server::builder()
        .add_service(health_service)
        .add_service(AggregateServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}

// ============================================================================
// Saga Support
// ============================================================================

/// Trait for saga implementations.
///
/// Sagas receive events and produce commands in response.
pub trait SagaLogic: Send + Sync {
    /// Handle an event book, returning commands to execute.
    fn handle(&self, book: &EventBook) -> Vec<CommandBook>;
}

/// Wrapper that adapts a SagaLogic to the Saga gRPC service.
pub struct SagaWrapper<T> {
    saga: T,
}

impl<T> SagaWrapper<T> {
    pub fn new(saga: T) -> Self {
        Self { saga }
    }
}

#[tonic::async_trait]
impl<T: SagaLogic + 'static> SagaService for SagaWrapper<T> {
    async fn handle(
        &self,
        request: Request<EventBook>,
    ) -> Result<Response<SagaResponse>, Status> {
        let event_book = request.into_inner();
        let commands = self.saga.handle(&event_book);
        Ok(Response::new(SagaResponse { commands }))
    }
}

/// Run a saga server with the given saga logic.
///
/// # Example
/// ```ignore
/// run_saga_server("fulfillment", "50061", FulfillmentSaga::new()).await
/// ```
pub async fn run_saga_server<T: SagaLogic + 'static>(
    saga_name: &str,
    default_port: &str,
    saga: T,
) -> Result<(), Box<dyn Error>> {
    init_tracing();

    let port = get_port(default_port);
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    let service = SagaWrapper::new(saga);

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<SagaServer<SagaWrapper<T>>>()
        .await;

    info!(saga = saga_name, port = %port, "saga_server_started");

    Server::builder()
        .add_service(health_service)
        .add_service(SagaServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
