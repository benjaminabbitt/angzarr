//! gRPC server utilities for running aggregate and saga services.
//!
//! This module provides helpers for starting gRPC servers with TCP or UDS transport.

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use tonic::transport::Server;
use tracing::info;

use crate::handler::{AggregateHandler, ProcessManagerGrpcHandler, ProjectorHandler, SagaHandler};
use crate::proto::aggregate_service_server::AggregateServiceServer;
use crate::proto::process_manager_service_server::ProcessManagerServiceServer;
use crate::proto::projector_service_server::ProjectorServiceServer;
use crate::proto::saga_service_server::SagaServiceServer;
use crate::router::{CommandRouter, EventRouter, ProcessManagerRouter};

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

/// Run an aggregate service with the given router.
///
/// Supports both TCP and Unix domain socket (UDS) transport.
/// UDS is used when `UDS_BASE_PATH`, `SERVICE_NAME`, and `DOMAIN` env vars are set.
///
/// # Example
///
/// ```rust,ignore
/// use angzarr_client::{run_aggregate_server, CommandRouter};
///
/// #[tokio::main]
/// async fn main() {
///     let router = CommandRouter::new("player", rebuild_state)
///         .on("RegisterPlayer", handle_register);
///
///     run_aggregate_server("player", 50001, router).await;
/// }
/// ```
pub async fn run_aggregate_server<S: Send + Sync + 'static>(
    domain: &str,
    default_port: u16,
    router: CommandRouter<S>,
) -> Result<(), tonic::transport::Error> {
    let config = ServerConfig::from_env(default_port);
    let handler = AggregateHandler::new(router);
    let service = AggregateServiceServer::new(handler);

    if let Some(uds_path) = &config.uds_path {
        // UDS mode (standalone)
        info!(
            domain = domain,
            path = %uds_path.display(),
            "Starting aggregate server (UDS)"
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
            "Starting aggregate server"
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
/// use angzarr_client::{run_saga_server, EventRouter};
///
/// #[tokio::main]
/// async fn main() {
///     let router = EventRouter::new("saga-order-fulfillment", "order")
///         .on("OrderCompleted", handle_order_completed);
///
///     run_saga_server("saga-order-fulfillment", 50010, router).await;
/// }
/// ```
pub async fn run_saga_server(
    name: &str,
    default_port: u16,
    router: EventRouter,
) -> Result<(), tonic::transport::Error> {
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
///         .subscribes("table")
///         .subscribes("hand")
///         .on("HandStarted", handle_hand_started);
///
///     run_process_manager_server("hand-flow", 9091, router).await;
/// }
/// ```
pub async fn run_process_manager_server<S: Send + Sync + 'static>(
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
