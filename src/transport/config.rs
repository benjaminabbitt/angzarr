//! Transport configuration types.

use std::path::PathBuf;

use serde::Deserialize;

/// Environment variable for configuring gRPC message size limit (in KB).
pub const GRPC_MESSAGE_SIZE_KB_ENV: &str = "ANGZARR_GRPC_MESSAGE_SIZE_KB";

/// Default gRPC message size (10 MB = 10240 KB).
pub const DEFAULT_GRPC_MESSAGE_SIZE_KB: usize = 10 * 1024;

/// Get the configured gRPC message size in bytes.
///
/// Reads from `ANGZARR_GRPC_MESSAGE_SIZE_KB` environment variable (in KB).
/// Falls back to 10 MB (10240 KB) if not set or invalid.
pub fn max_grpc_message_size() -> usize {
    std::env::var(GRPC_MESSAGE_SIZE_KB_ENV)
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(DEFAULT_GRPC_MESSAGE_SIZE_KB)
        * 1024
}

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
    ///
    /// Default is `127.0.0.1` (localhost only) for security.
    /// Set to `0.0.0.0` explicitly to bind to all interfaces.
    pub host: String,
    /// Port to bind to.
    pub port: u16,
}

impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            // Default to localhost for security - external access requires explicit config
            host: "127.0.0.1".to_string(),
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
    ///
    /// Default uses `$XDG_RUNTIME_DIR/angzarr` if set, otherwise `/tmp/angzarr-{uid}`.
    /// The per-user directory prevents socket file conflicts and improves security.
    pub base_path: PathBuf,
}

impl Default for UdsConfig {
    fn default() -> Self {
        Self {
            base_path: default_uds_path(),
        }
    }
}

/// Get the default UDS socket base path.
///
/// Prefers `$XDG_RUNTIME_DIR/angzarr` (typically `/run/user/{uid}/angzarr`).
/// Falls back to `/tmp/angzarr-{uid}` for per-user isolation.
fn default_uds_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("angzarr")
    } else {
        // Fallback with user ID for isolation
        #[cfg(unix)]
        {
            let uid = nix::unistd::getuid();
            PathBuf::from(format!("/tmp/angzarr-{}", uid))
        }
        #[cfg(not(unix))]
        {
            PathBuf::from("/tmp/angzarr")
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
    /// Uses `{qualifier}-{service_name}` order to match K8s service naming convention.
    /// Example: `socket_path_qualified("aggregate", "orders")` -> `orders-aggregate.sock`
    pub fn socket_path_qualified(&self, service_name: &str, qualifier: &str) -> PathBuf {
        self.base_path
            .join(format!("{}-{}.sock", qualifier, service_name))
    }
}
