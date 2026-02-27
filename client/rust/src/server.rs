//! gRPC server utilities for running aggregate and saga services.
//!
//! This module provides helpers for starting gRPC servers with TCP or UDS transport.

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use tonic::transport::Server;
use tracing::info;

use crate::handler::{
    CloudEventsGrpcHandler, CommandHandlerGrpc, ProcessManagerGrpcHandler, ProjectorHandler,
    SagaHandler, UpcasterGrpcHandler,
};
use crate::proto::command_handler_service_server::CommandHandlerServiceServer;
use crate::proto::process_manager_service_server::ProcessManagerServiceServer;
use crate::proto::projector_service_server::ProjectorServiceServer;
use crate::proto::saga_service_server::SagaServiceServer;
use crate::proto::upcaster_service_server::UpcasterServiceServer;
use crate::router::{
    CloudEventsRouter, CommandHandlerDomainHandler, CommandHandlerRouter, ProcessManagerRouter,
    SagaDomainHandler, SagaRouter,
};

/// Configuration for a gRPC server.
pub struct ServerConfig {
    /// Port to listen on (TCP mode).
    pub port: u16,
    /// Unix domain socket path (UDS mode).
    pub uds_path: Option<PathBuf>,
}

impl ServerConfig {
    /// Create config from environment variables.
    ///
    /// UDS mode (standalone):
    /// - `UDS_BASE_PATH`: Base directory for UDS sockets
    /// - `SERVICE_NAME`: Service name (e.g., "business")
    /// - `DOMAIN`: Domain name (e.g., "player")
    ///   => Socket path: `{UDS_BASE_PATH}/{SERVICE_NAME}-{DOMAIN}.sock`
    ///
    /// TCP mode (distributed):
    /// - `PORT` or `GRPC_PORT`: TCP port (default: `default_port`)
    pub fn from_env(default_port: u16) -> Self {
        // Check for UDS mode first
        if let (Ok(base_path), Ok(service_name), Ok(domain)) = (
            env::var("UDS_BASE_PATH"),
            env::var("SERVICE_NAME"),
            env::var("DOMAIN"),
        ) {
            let socket_name = format!("{}-{}.sock", service_name, domain);
            let uds_path = PathBuf::from(base_path).join(socket_name);
            return Self {
                port: default_port,
                uds_path: Some(uds_path),
            };
        }

        // Fall back to TCP mode
        let port = env::var("PORT")
            .or_else(|_| env::var("GRPC_PORT"))
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(default_port);

        Self {
            port,
            uds_path: None,
        }
    }
}

/// Run a command handler service with the given router.
///
/// Supports both TCP and Unix domain socket (UDS) transport.
/// UDS is used when `UDS_BASE_PATH`, `SERVICE_NAME`, and `DOMAIN` env vars are set.
///
/// # Example
///
/// ```rust,ignore
/// use angzarr_client::{run_command_handler_server, CommandHandlerRouter};
///
/// #[tokio::main]
/// async fn main() {
///     let router = CommandHandlerRouter::new("player", "player", PlayerHandler::new());
///
///     run_command_handler_server("player", 50001, router).await;
/// }
/// ```
pub async fn run_command_handler_server<S, H>(
    domain: &str,
    default_port: u16,
    router: CommandHandlerRouter<S, H>,
) -> Result<(), tonic::transport::Error>
where
    S: Default + Send + Sync + 'static,
    H: CommandHandlerDomainHandler<State = S> + 'static,
{
    let config = ServerConfig::from_env(default_port);
    let handler = CommandHandlerGrpc::new(router);
    let service = CommandHandlerServiceServer::new(handler);

    if let Some(uds_path) = &config.uds_path {
        // UDS mode (standalone)
        info!(
            domain = domain,
            path = %uds_path.display(),
            "Starting command handler server (UDS)"
        );

        // Remove existing socket file if present
        let _ = std::fs::remove_file(uds_path);

        let uds = tokio::net::UnixListener::bind(uds_path).expect("Failed to bind UDS socket");
        let incoming = tokio_stream::wrappers::UnixListenerStream::new(uds);

        Server::builder()
            .add_service(service)
            .serve_with_incoming(incoming)
            .await
    } else {
        // TCP mode (distributed)
        let addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse().unwrap();

        info!(
            domain = domain,
            port = config.port,
            "Starting command handler server"
        );

        Server::builder().add_service(service).serve(addr).await
    }
}

/// Run a saga service with the given router.
///
/// Supports both TCP and Unix domain socket (UDS) transport.
///
/// # Example
///
/// ```rust,ignore
/// use angzarr_client::{run_saga_server, SagaRouter};
///
/// #[tokio::main]
/// async fn main() {
///     let router = SagaRouter::new("saga-order-fulfillment", "order", OrderHandler::new());
///
///     run_saga_server("saga-order-fulfillment", 50010, router).await;
/// }
/// ```
pub async fn run_saga_server<H>(
    name: &str,
    default_port: u16,
    router: SagaRouter<H>,
) -> Result<(), tonic::transport::Error>
where
    H: SagaDomainHandler + 'static,
{
    let config = ServerConfig::from_env(default_port);
    let handler = SagaHandler::new(router);
    let service = SagaServiceServer::new(handler);

    if let Some(uds_path) = &config.uds_path {
        // UDS mode
        info!(
            name = name,
            path = %uds_path.display(),
            "Starting saga server (UDS)"
        );

        let _ = std::fs::remove_file(uds_path);

        let uds = tokio::net::UnixListener::bind(uds_path).expect("Failed to bind UDS socket");
        let incoming = tokio_stream::wrappers::UnixListenerStream::new(uds);

        Server::builder()
            .add_service(service)
            .serve_with_incoming(incoming)
            .await
    } else {
        // TCP mode
        let addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse().unwrap();

        info!(name = name, port = config.port, "Starting saga server");

        Server::builder().add_service(service).serve(addr).await
    }
}

/// Run a projector service with the given handler.
///
/// Supports both TCP and Unix domain socket (UDS) transport.
///
/// # Example
///
/// ```rust,ignore
/// use angzarr_client::{run_projector_server, ProjectorHandler};
///
/// #[tokio::main]
/// async fn main() {
///     let handler = ProjectorHandler::new("output").with_handle(handle_events);
///
///     run_projector_server("output", 9090, handler).await;
/// }
/// ```
pub async fn run_projector_server(
    name: &str,
    default_port: u16,
    handler: ProjectorHandler,
) -> Result<(), tonic::transport::Error> {
    let config = ServerConfig::from_env(default_port);
    let service = ProjectorServiceServer::new(handler);

    if let Some(uds_path) = &config.uds_path {
        // UDS mode
        info!(
            name = name,
            path = %uds_path.display(),
            "Starting projector server (UDS)"
        );

        let _ = std::fs::remove_file(uds_path);

        let uds = tokio::net::UnixListener::bind(uds_path).expect("Failed to bind UDS socket");
        let incoming = tokio_stream::wrappers::UnixListenerStream::new(uds);

        Server::builder()
            .add_service(service)
            .serve_with_incoming(incoming)
            .await
    } else {
        // TCP mode
        let addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse().unwrap();

        info!(name = name, port = config.port, "Starting projector server");

        Server::builder().add_service(service).serve(addr).await
    }
}

/// Run a process manager service with the given router.
///
/// Supports both TCP and Unix domain socket (UDS) transport.
///
/// # Example
///
/// ```rust,ignore
/// use angzarr_client::{run_process_manager_server, ProcessManagerRouter};
///
/// #[tokio::main]
/// async fn main() {
///     let router = ProcessManagerRouter::new("hand-flow", "hand-flow", rebuild_state)
///         .domain("table", TablePmHandler::new());
///
///     run_process_manager_server("hand-flow", 9091, router).await;
/// }
/// ```
pub async fn run_process_manager_server<S: Default + Send + Sync + 'static>(
    name: &str,
    default_port: u16,
    router: ProcessManagerRouter<S>,
) -> Result<(), tonic::transport::Error> {
    let config = ServerConfig::from_env(default_port);
    let handler = ProcessManagerGrpcHandler::new(router);
    let service = ProcessManagerServiceServer::new(handler);

    if let Some(uds_path) = &config.uds_path {
        // UDS mode
        info!(
            name = name,
            path = %uds_path.display(),
            "Starting process manager server (UDS)"
        );

        let _ = std::fs::remove_file(uds_path);

        let uds = tokio::net::UnixListener::bind(uds_path).expect("Failed to bind UDS socket");
        let incoming = tokio_stream::wrappers::UnixListenerStream::new(uds);

        Server::builder()
            .add_service(service)
            .serve_with_incoming(incoming)
            .await
    } else {
        // TCP mode
        let addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse().unwrap();

        info!(
            name = name,
            port = config.port,
            "Starting process manager server"
        );

        Server::builder().add_service(service).serve(addr).await
    }
}

/// Run an upcaster service with the given handler.
///
/// Supports both TCP and Unix domain socket (UDS) transport.
///
/// # Example
///
/// ```rust,ignore
/// use angzarr_client::{run_upcaster_server, UpcasterGrpcHandler, UpcasterRouter};
///
/// fn upcast_events(events: &[EventPage]) -> Vec<EventPage> {
///     let router = UpcasterRouter::new("player")
///         .on("PlayerRegisteredV1", |old| {
///             // Transform old event to new version
///             old.clone()
///         });
///     router.upcast(events)
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let handler = UpcasterGrpcHandler::new("upcaster-player", "player")
///         .with_handle(upcast_events);
///
///     run_upcaster_server("upcaster-player", 50401, handler).await;
/// }
/// ```
pub async fn run_upcaster_server(
    name: &str,
    default_port: u16,
    handler: UpcasterGrpcHandler,
) -> Result<(), tonic::transport::Error> {
    let config = ServerConfig::from_env(default_port);
    let service = UpcasterServiceServer::new(handler);

    if let Some(uds_path) = &config.uds_path {
        // UDS mode
        info!(
            name = name,
            path = %uds_path.display(),
            "Starting upcaster server (UDS)"
        );

        let _ = std::fs::remove_file(uds_path);

        let uds = tokio::net::UnixListener::bind(uds_path).expect("Failed to bind UDS socket");
        let incoming = tokio_stream::wrappers::UnixListenerStream::new(uds);

        Server::builder()
            .add_service(service)
            .serve_with_incoming(incoming)
            .await
    } else {
        // TCP mode
        let addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse().unwrap();

        info!(name = name, port = config.port, "Starting upcaster server");

        Server::builder().add_service(service).serve(addr).await
    }
}

/// Run a CloudEvents projector service with the given router.
///
/// CloudEvents projectors transform domain events into CloudEvents 1.0 format
/// for external consumption via HTTP webhooks or Kafka.
///
/// Supports both TCP and Unix domain socket (UDS) transport.
///
/// # Example
///
/// ```rust,ignore
/// use angzarr_client::{run_cloudevents_projector, CloudEventsRouter};
/// use angzarr_client::proto::angzarr::CloudEvent;
/// use angzarr_client::proto::examples::PlayerRegistered;
///
/// fn handle_player_registered(event: &PlayerRegistered) -> Option<CloudEvent> {
///     Some(CloudEvent {
///         r#type: "com.poker.player.registered".into(),
///         ..Default::default()
///     })
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let router = CloudEventsRouter::new("prj-player-cloudevents", "player")
///         .on::<PlayerRegistered>(handle_player_registered);
///
///     run_cloudevents_projector("prj-player-cloudevents", 50091, router).await;
/// }
/// ```
pub async fn run_cloudevents_projector(
    name: &str,
    default_port: u16,
    router: CloudEventsRouter,
) -> Result<(), tonic::transport::Error> {
    let config = ServerConfig::from_env(default_port);
    let handler = CloudEventsGrpcHandler::new(router);
    let service = ProjectorServiceServer::new(handler);

    if let Some(uds_path) = &config.uds_path {
        // UDS mode
        info!(
            name = name,
            path = %uds_path.display(),
            "Starting CloudEvents projector server (UDS)"
        );

        let _ = std::fs::remove_file(uds_path);

        let uds = tokio::net::UnixListener::bind(uds_path).expect("Failed to bind UDS socket");
        let incoming = tokio_stream::wrappers::UnixListenerStream::new(uds);

        Server::builder()
            .add_service(service)
            .serve_with_incoming(incoming)
            .await
    } else {
        // TCP mode
        let addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse().unwrap();

        info!(
            name = name,
            port = config.port,
            "Starting CloudEvents projector server"
        );

        Server::builder().add_service(service).serve(addr).await
    }
}
