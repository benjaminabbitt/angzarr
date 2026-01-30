//! Transport layer abstraction for gRPC servers and clients.
//!
//! Supports:
//! - TCP: Standard network transport (default)
//! - UDS: Unix Domain Sockets for local IPC (embedded mode)

use std::future::Future;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use hyper_util::rt::TokioIo;
use serde::Deserialize;
use tokio::net::{UnixListener, UnixStream};
use tokio_stream::wrappers::UnixListenerStream;
use tonic::service::Routes;
use tonic::transport::server::Router;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;
use tower::Layer;
use tower::Service;
use tower_http::trace::TraceLayer;
use tracing::info;

/// Transport type discriminator.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportType {
    /// TCP transport (network).
    #[default]
    Tcp,
    /// Unix Domain Socket transport (local IPC).
    Uds,
}

/// Transport configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TransportConfig {
    /// Transport type discriminator.
    #[serde(rename = "type")]
    pub transport_type: TransportType,
    /// TCP-specific configuration.
    pub tcp: TcpConfig,
    /// UDS-specific configuration.
    pub uds: UdsConfig,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            transport_type: TransportType::Tcp,
            tcp: TcpConfig::default(),
            uds: UdsConfig::default(),
        }
    }
}

/// TCP transport configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TcpConfig {
    /// Host to bind to.
    pub host: String,
    /// Port to bind to.
    pub port: u16,
}

impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 50051,
        }
    }
}

impl TcpConfig {
    /// Get the socket address string.
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// UDS transport configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct UdsConfig {
    /// Base path for socket files.
    pub base_path: PathBuf,
}

impl Default for UdsConfig {
    fn default() -> Self {
        Self {
            base_path: PathBuf::from("/tmp/angzarr"),
        }
    }
}

impl UdsConfig {
    /// Get the socket path for a service.
    pub fn socket_path(&self, service_name: &str) -> PathBuf {
        self.base_path.join(format!("{}.sock", service_name))
    }

    /// Get the socket path for a service with a qualifier (e.g., domain name).
    ///
    /// Example: `socket_path_qualified("aggregate", "orders")` -> `aggregate-orders.sock`
    pub fn socket_path_qualified(&self, service_name: &str, qualifier: &str) -> PathBuf {
        self.base_path
            .join(format!("{}-{}.sock", service_name, qualifier))
    }
}

/// RAII guard for cleaning up UDS socket files.
pub struct UdsCleanupGuard {
    path: PathBuf,
}

impl UdsCleanupGuard {
    /// Create a new cleanup guard for the given socket path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Get the socket path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for UdsCleanupGuard {
    fn drop(&mut self) {
        if self.path.exists() {
            if let Err(e) = std::fs::remove_file(&self.path) {
                tracing::warn!(
                    path = %self.path.display(),
                    error = %e,
                    "Failed to clean up UDS socket"
                );
            } else {
                tracing::debug!(
                    path = %self.path.display(),
                    "Cleaned up UDS socket"
                );
            }
        }
    }
}

/// Prepare a UDS socket path for binding.
///
/// - Creates parent directories if needed
/// - Removes stale socket file if exists
/// - Returns a cleanup guard that removes the socket on drop
pub fn prepare_uds_socket(path: &Path) -> std::io::Result<UdsCleanupGuard> {
    // Create parent directories
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Remove stale socket file
    if path.exists() {
        info!(path = %path.display(), "Removing stale UDS socket");
        std::fs::remove_file(path)?;
    }

    Ok(UdsCleanupGuard::new(path))
}

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
    L::Service: Service<http::Request<tonic::body::BoxBody>, Response = http::Response<ResBody>>
        + Clone
        + Send
        + 'static,
    <L::Service as Service<http::Request<tonic::body::BoxBody>>>::Future: Send + 'static,
    <L::Service as Service<http::Request<tonic::body::BoxBody>>>::Error:
        Into<Box<dyn std::error::Error + Send + Sync>> + Send,
    ResBody: http_body::Body<Data = bytes::Bytes> + Send + 'static,
    ResBody::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let display_name = match qualifier {
        Some(q) => format!("{}-{}", service_name, q),
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
            router.serve(addr).await?;
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

            router.serve_with_incoming(stream).await?;
        }
    }

    Ok(())
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
    L::Service: Service<http::Request<tonic::body::BoxBody>, Response = http::Response<ResBody>>
        + Clone
        + Send
        + 'static,
    <L::Service as Service<http::Request<tonic::body::BoxBody>>>::Future: Send + 'static,
    <L::Service as Service<http::Request<tonic::body::BoxBody>>>::Error:
        Into<Box<dyn std::error::Error + Send + Sync>> + Send,
    ResBody: http_body::Body<Data = bytes::Bytes> + Send + 'static,
    ResBody::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    F: Future<Output = ()> + Send,
{
    let display_name = match qualifier {
        Some(q) => format!("{}-{}", service_name, q),
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

// ============================================================================
// Client Connection
// ============================================================================

/// Check if an address is a UDS path.
pub fn is_uds_address(address: &str) -> bool {
    address.starts_with('/') || address.starts_with("./")
}

/// Connect to a gRPC service by address.
///
/// Automatically detects whether the address is a UDS path or TCP address:
/// - Paths starting with `/` or `./` are treated as Unix domain sockets
/// - Everything else is treated as a TCP address (host:port)
///
/// This allows config files to use either:
/// - `address: /tmp/angzarr/business-orders.sock` (UDS)
/// - `address: localhost:50051` (TCP)
pub async fn connect_to_address(address: &str) -> Result<Channel, Box<dyn std::error::Error>> {
    if address.starts_with('/') || address.starts_with("./") {
        // UDS path
        let socket_path = PathBuf::from(address);
        info!(
            path = %socket_path.display(),
            transport = "uds",
            "Connecting to service"
        );

        let channel = Endpoint::try_from("http://[::]:50051")?
            .connect_with_connector(service_fn(move |_: Uri| {
                let path = socket_path.clone();
                async move {
                    let stream = UnixStream::connect(path).await?;
                    Ok::<_, std::io::Error>(TokioIo::new(stream))
                }
            }))
            .await?;

        Ok(channel)
    } else {
        // TCP address
        let uri = if address.starts_with("http://") || address.starts_with("https://") {
            address.to_string()
        } else {
            format!("http://{}", address)
        };

        info!(
            address = %address,
            transport = "tcp",
            "Connecting to service"
        );

        let channel = Channel::from_shared(uri)?.connect().await?;
        Ok(channel)
    }
}

/// Connect to a gRPC service using the configured transport.
///
/// # Arguments
/// * `config` - Transport configuration
/// * `service_name` - Base service name (e.g., "aggregate", "projector")
/// * `qualifier` - Optional qualifier for domain/name-specific sockets (e.g., "orders")
/// * `tcp_address` - TCP address to use when transport is TCP (e.g., "localhost:50051")
///
/// For TCP transport, uses the provided `tcp_address`.
/// For UDS transport, derives socket path from `service_name` and `qualifier`.
pub async fn connect_with_transport(
    config: &TransportConfig,
    service_name: &str,
    qualifier: Option<&str>,
    tcp_address: &str,
) -> Result<Channel, Box<dyn std::error::Error + Send + Sync>> {
    let display_name = match qualifier {
        Some(q) => format!("{}-{}", service_name, q),
        None => service_name.to_string(),
    };

    match config.transport_type {
        TransportType::Tcp => {
            let uri = format!("http://{}", tcp_address);
            info!(
                service = %display_name,
                address = %tcp_address,
                transport = "tcp",
                "Connecting to service"
            );
            let channel = Channel::from_shared(uri)?.connect().await?;
            Ok(channel)
        }
        TransportType::Uds => {
            let socket_path = match qualifier {
                Some(q) => config.uds.socket_path_qualified(service_name, q),
                None => config.uds.socket_path(service_name),
            };

            info!(
                service = %display_name,
                path = %socket_path.display(),
                transport = "uds",
                "Connecting to service"
            );

            // UDS requires a dummy URI and custom connector
            // TokioIo wraps UnixStream to implement hyper's io traits
            let channel = Endpoint::try_from("http://[::]:50051")?
                .connect_with_connector(service_fn(move |_: Uri| {
                    let path = socket_path.clone();
                    async move {
                        let stream = UnixStream::connect(path).await?;
                        Ok::<_, std::io::Error>(TokioIo::new(stream))
                    }
                }))
                .await?;

            Ok(channel)
        }
    }
}

/// Service endpoint configuration for embedded mode.
///
/// Allows specifying either a TCP address or deriving UDS path from name.
#[derive(Debug, Clone, Deserialize)]
pub struct ServiceEndpointConfig {
    /// Service name (used for UDS socket path).
    pub name: String,
    /// Optional qualifier (e.g., domain name).
    pub qualifier: Option<String>,
    /// TCP address (used when transport is TCP).
    pub address: Option<String>,
}

impl ServiceEndpointConfig {
    /// Create a new endpoint config for a named service.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            qualifier: None,
            address: None,
        }
    }

    /// Add a qualifier (e.g., domain name).
    pub fn with_qualifier(mut self, qualifier: impl Into<String>) -> Self {
        self.qualifier = Some(qualifier.into());
        self
    }

    /// Add a TCP address.
    pub fn with_address(mut self, address: impl Into<String>) -> Self {
        self.address = Some(address.into());
        self
    }

    /// Connect using the transport config.
    pub async fn connect(
        &self,
        transport: &TransportConfig,
    ) -> Result<Channel, Box<dyn std::error::Error + Send + Sync>> {
        let tcp_addr = self.address.as_deref().unwrap_or("localhost:50051");
        connect_with_transport(transport, &self.name, self.qualifier.as_deref(), tcp_addr).await
    }
}

/// Tower trace layer that extracts `x-correlation-id` from gRPC request headers.
///
/// Creates a tracing span per request with the correlation_id, enabling
/// all downstream tracing to inherit it automatically. This works at the HTTP
/// layer — before tonic deserializes the protobuf body.
///
/// When the `otel` feature is enabled, also extracts W3C `traceparent` header
/// and sets it as the parent context on the span for distributed tracing.
pub fn grpc_trace_layer() -> TraceLayer<
    tower_http::classify::SharedClassifier<tower_http::classify::GrpcErrorsAsFailures>,
    impl Fn(&http::Request<tonic::body::BoxBody>) -> tracing::Span + Clone,
> {
    TraceLayer::new_for_grpc().make_span_with(|request: &http::Request<tonic::body::BoxBody>| {
        let correlation_id = request
            .headers()
            .get("x-correlation-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let path = request.uri().path();
        let span = tracing::info_span!("grpc", %correlation_id, %path);

        #[cfg(feature = "otel")]
        {
            extract_trace_context(request.headers(), &span);
        }

        span
    })
}

/// Extract W3C trace context from HTTP headers and set as parent on the span.
#[cfg(feature = "otel")]
fn extract_trace_context(headers: &http::HeaderMap, span: &tracing::Span) {
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    let parent_cx = opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(headers))
    });
    span.set_parent(parent_cx);
}

/// Adapter to extract OTel context from HTTP headers.
#[cfg(feature = "otel")]
struct HeaderExtractor<'a>(&'a http::HeaderMap);

#[cfg(feature = "otel")]
impl opentelemetry::propagation::Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_config_default() {
        let config = TransportConfig::default();
        assert_eq!(config.transport_type, TransportType::Tcp);
        assert_eq!(config.tcp.host, "0.0.0.0");
        assert_eq!(config.tcp.port, 50051);
    }

    #[test]
    fn test_tcp_addr() {
        let tcp = TcpConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
        };
        assert_eq!(tcp.addr(), "127.0.0.1:8080");
    }

    #[test]
    fn test_uds_socket_path() {
        let uds = UdsConfig {
            base_path: PathBuf::from("/tmp/test"),
        };
        assert_eq!(
            uds.socket_path("gateway"),
            PathBuf::from("/tmp/test/gateway.sock")
        );
    }

    #[test]
    fn test_uds_socket_path_qualified() {
        let uds = UdsConfig {
            base_path: PathBuf::from("/tmp/angzarr"),
        };
        assert_eq!(
            uds.socket_path_qualified("aggregate", "orders"),
            PathBuf::from("/tmp/angzarr/aggregate-orders.sock")
        );
        assert_eq!(
            uds.socket_path_qualified("projector", "accounting"),
            PathBuf::from("/tmp/angzarr/projector-accounting.sock")
        );
    }

    #[test]
    fn test_uds_cleanup_guard() {
        let temp_dir = std::env::temp_dir();
        let socket_path = temp_dir.join("test_cleanup.sock");

        // Create a dummy file
        std::fs::write(&socket_path, "test").unwrap();
        assert!(socket_path.exists());

        // Guard should clean up on drop
        {
            let _guard = UdsCleanupGuard::new(&socket_path);
        }

        assert!(!socket_path.exists());
    }
}
