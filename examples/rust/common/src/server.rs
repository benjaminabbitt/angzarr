//! Server bootstrap helpers for example services.
//!
//! Reduces boilerplate by providing generic wrappers and server runners
//! for aggregates, sagas, and projectors.
//!
//! Supports both TCP and UDS transports based on environment variables:
//! - TRANSPORT_TYPE: "tcp" or "uds" (default: tcp)
//! - UDS_BASE_PATH: Base directory for UDS sockets (e.g., /tmp/angzarr)
//! - SERVICE_NAME: Service type for socket naming (e.g., "business", "saga")
//! - DOMAIN: Domain name for socket naming (e.g., "customer")
//! - PORT: TCP port (used when TRANSPORT_TYPE=tcp)

use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;

use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use angzarr::proto::aggregate_server::{Aggregate, AggregateServer};
use angzarr::proto::projector_server::{Projector as ProjectorService, ProjectorServer};
use angzarr::proto::saga_server::{Saga as SagaService, SagaServer};
use angzarr::proto::{
    BusinessResponse, CommandBook, ContextualCommand, EventBook, Projection, SagaResponse,
};

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

/// Transport configuration from environment.
pub enum Transport {
    Tcp(SocketAddr),
    Uds(PathBuf),
}

/// Get transport configuration from environment variables.
pub fn get_transport(service_name: &str, domain: &str, default_port: &str) -> Transport {
    let transport_type = std::env::var("TRANSPORT_TYPE").unwrap_or_else(|_| "tcp".to_string());

    if transport_type == "uds" {
        let base_path = std::env::var("UDS_BASE_PATH").unwrap_or_else(|_| "/tmp/angzarr".to_string());
        let svc = std::env::var("SERVICE_NAME").unwrap_or_else(|_| service_name.to_string());
        let dom = std::env::var("DOMAIN").unwrap_or_else(|_| domain.to_string());
        let socket_path = PathBuf::from(format!("{}/{}-{}.sock", base_path, svc, dom));
        Transport::Uds(socket_path)
    } else {
        let port = get_port(default_port);
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().expect("valid address");
        Transport::Tcp(addr)
    }
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
/// run_aggregate_server("cart", "50093", CartLogic::new()).await
/// ```
pub async fn run_aggregate_server<T: AggregateLogic + 'static>(
    domain: &str,
    default_port: &str,
    logic: T,
) -> Result<(), Box<dyn Error>> {
    init_tracing();

    let transport = get_transport("business", domain, default_port);
    let service = AggregateWrapper::new(logic);

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<AggregateServer<AggregateWrapper<T>>>()
        .await;

    match transport {
        Transport::Tcp(addr) => {
            info!(domain = domain, port = %addr.port(), transport = "tcp", "server_started");
            Server::builder()
                .add_service(health_service)
                .add_service(AggregateServer::new(service))
                .serve(addr)
                .await?;
        }
        Transport::Uds(socket_path) => {
            // Remove stale socket if exists
            if socket_path.exists() {
                std::fs::remove_file(&socket_path)?;
            }
            let uds = UnixListener::bind(&socket_path)?;
            let uds_stream = UnixListenerStream::new(uds);
            info!(domain = domain, path = %socket_path.display(), transport = "uds", "server_started");
            Server::builder()
                .add_service(health_service)
                .add_service(AggregateServer::new(service))
                .serve_with_incoming(uds_stream)
                .await?;
        }
    }

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
/// run_saga_server("fulfillment", "50123", FulfillmentSaga::new()).await
/// ```
pub async fn run_saga_server<T: SagaLogic + 'static>(
    saga_name: &str,
    default_port: &str,
    saga: T,
) -> Result<(), Box<dyn Error>> {
    init_tracing();

    let transport = get_transport("saga", saga_name, default_port);
    let service = SagaWrapper::new(saga);

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<SagaServer<SagaWrapper<T>>>()
        .await;

    match transport {
        Transport::Tcp(addr) => {
            info!(saga = saga_name, port = %addr.port(), transport = "tcp", "saga_server_started");
            Server::builder()
                .add_service(health_service)
                .add_service(SagaServer::new(service))
                .serve(addr)
                .await?;
        }
        Transport::Uds(socket_path) => {
            if socket_path.exists() {
                std::fs::remove_file(&socket_path)?;
            }
            let uds = UnixListener::bind(&socket_path)?;
            let uds_stream = UnixListenerStream::new(uds);
            info!(saga = saga_name, path = %socket_path.display(), transport = "uds", "saga_server_started");
            Server::builder()
                .add_service(health_service)
                .add_service(SagaServer::new(service))
                .serve_with_incoming(uds_stream)
                .await?;
        }
    }

    Ok(())
}

// ============================================================================
// Projector Support
// ============================================================================

/// Trait for projector implementations.
///
/// Projectors receive events and produce projections (read model updates).
#[tonic::async_trait]
pub trait ProjectorLogic: Send + Sync {
    /// Handle an event book, returning an optional projection.
    async fn handle(&self, book: &EventBook) -> Result<Option<Projection>, Status>;
}

/// Wrapper that adapts a ProjectorLogic to the Projector gRPC service.
pub struct ProjectorWrapper<T> {
    projector: T,
}

impl<T> ProjectorWrapper<T> {
    pub fn new(projector: T) -> Self {
        Self { projector }
    }
}

#[tonic::async_trait]
impl<T: ProjectorLogic + 'static> ProjectorService for ProjectorWrapper<T> {
    async fn handle(
        &self,
        request: Request<EventBook>,
    ) -> Result<Response<Projection>, Status> {
        let event_book = request.into_inner();
        let projection = self.projector.handle(&event_book).await?;
        Ok(Response::new(projection.unwrap_or_default()))
    }
}

/// Run a projector server with the given projector logic.
///
/// # Example
/// ```ignore
/// run_projector_server("logging", "customer", "50163", LoggingProjector::new("customer")).await
/// ```
pub async fn run_projector_server<T: ProjectorLogic + 'static>(
    projector_name: &str,
    domain: &str,
    default_port: &str,
    projector: T,
) -> Result<(), Box<dyn Error>> {
    init_tracing();

    // Socket name combines projector name and domain for uniqueness
    let socket_name = format!("{}-{}", projector_name, domain);
    let transport = get_transport("projector", &socket_name, default_port);
    let service = ProjectorWrapper::new(projector);

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<ProjectorServer<ProjectorWrapper<T>>>()
        .await;

    match transport {
        Transport::Tcp(addr) => {
            info!(projector = projector_name, domain = domain, port = %addr.port(), transport = "tcp", "projector_server_started");
            Server::builder()
                .add_service(health_service)
                .add_service(ProjectorServer::new(service))
                .serve(addr)
                .await?;
        }
        Transport::Uds(socket_path) => {
            if socket_path.exists() {
                std::fs::remove_file(&socket_path)?;
            }
            let uds = UnixListener::bind(&socket_path)?;
            let uds_stream = UnixListenerStream::new(uds);
            info!(projector = projector_name, domain = domain, path = %socket_path.display(), transport = "uds", "projector_server_started");
            Server::builder()
                .add_service(health_service)
                .add_service(ProjectorServer::new(service))
                .serve_with_incoming(uds_stream)
                .await?;
        }
    }

    Ok(())
}
