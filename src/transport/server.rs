//! gRPC server functions for transport abstraction.

use std::future::Future;
use std::net::SocketAddr;

use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::service::Routes;
use tonic::transport::server::Router;
use tower::Layer;
use tower::Service;
use tracing::info;

use super::config::{TransportConfig, TransportType};
use super::uds::prepare_uds_socket;

/// Serve a gRPC router using the configured transport.
///
/// Returns a future that completes when the server shuts down.
/// For UDS, also returns a cleanup guard that removes the socket on drop.
///
/// # Arguments
/// * `router` - The gRPC router to serve
/// * `config` - Transport configuration
/// * `service_name` - Base service name (e.g., "aggregate", "projector")
/// * `qualifier` - Optional qualifier for domain/name-specific sockets (e.g., "orders")
///
/// For UDS transport:
/// - Without qualifier: `/tmp/angzarr/{service_name}.sock`
/// - With qualifier: `/tmp/angzarr/{service_name}-{qualifier}.sock`
pub async fn serve_with_transport<L, ResBody>(
    router: Router<L>,
    config: &TransportConfig,
    service_name: &str,
    qualifier: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>>
where
    L: Layer<Routes> + Clone,
    L::Service: Service<http::Request<tonic::body::Body>, Response = http::Response<ResBody>>
        + Clone
        + Send
        + 'static,
    <L::Service as Service<http::Request<tonic::body::Body>>>::Future: Send + 'static,
    <L::Service as Service<http::Request<tonic::body::Body>>>::Error:
        Into<Box<dyn std::error::Error + Send + Sync>> + Send,
    ResBody: http_body::Body<Data = bytes::Bytes> + Send + 'static,
    ResBody::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    // Use shutdown_signal() for graceful shutdown with telemetry flush
    serve_with_transport_and_shutdown(
        router,
        config,
        service_name,
        qualifier,
        crate::utils::bootstrap::shutdown_signal(),
    )
    .await
}

/// Serve a gRPC router using the configured transport with a shutdown signal.
///
/// The server will gracefully shut down when the signal future completes.
pub async fn serve_with_transport_and_shutdown<L, ResBody, F>(
    router: Router<L>,
    config: &TransportConfig,
    service_name: &str,
    qualifier: Option<&str>,
    signal: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    L: Layer<Routes> + Clone,
    L::Service: Service<http::Request<tonic::body::Body>, Response = http::Response<ResBody>>
        + Clone
        + Send
        + 'static,
    <L::Service as Service<http::Request<tonic::body::Body>>>::Future: Send + 'static,
    <L::Service as Service<http::Request<tonic::body::Body>>>::Error:
        Into<Box<dyn std::error::Error + Send + Sync>> + Send,
    ResBody: http_body::Body<Data = bytes::Bytes> + Send + 'static,
    ResBody::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    F: Future<Output = ()> + Send,
{
    // Use {qualifier}-{service_name} order to match K8s naming convention
    let display_name = match qualifier {
        Some(q) => format!("{}-{}", q, service_name),
        None => service_name.to_string(),
    };

    match config.transport_type {
        TransportType::Tcp => {
            let addr: SocketAddr = config.tcp.addr().parse()?;
            info!(
                service = %display_name,
                address = %addr,
                transport = "tcp",
                "Server listening"
            );
            router.serve_with_shutdown(addr, signal).await?;
        }
        TransportType::Uds => {
            let socket_path = match qualifier {
                Some(q) => config.uds.socket_path_qualified(service_name, q),
                None => config.uds.socket_path(service_name),
            };
            let _guard = prepare_uds_socket(&socket_path)?;

            let uds = UnixListener::bind(&socket_path)?;
            let stream = UnixListenerStream::new(uds);

            info!(
                service = %display_name,
                path = %socket_path.display(),
                transport = "uds",
                "Server listening"
            );

            router.serve_with_incoming_shutdown(stream, signal).await?;
        }
    }

    Ok(())
}
