//! Tests for transport layer configuration.
//!
//! Transport abstracts TCP vs UDS (Unix Domain Socket) connections:
//! - TCP for distributed deployment (K8s, Cloud Run)
//! - UDS for local IPC (standalone mode, sidecar communication)
//!
//! Why this matters: Same code supports both deployment models.
//! Standalone mode uses UDS for zero-network-overhead IPC.
//! Distributed mode uses TCP for cross-node communication.
//!
//! Key behaviors verified:
//! - Default config is TCP on localhost:50051 (secure default)
//! - Socket path generation for UDS (domain-qualified naming)
//! - UDS cleanup guard removes socket files on drop

use super::*;

// ============================================================================
// TransportConfig Tests
// ============================================================================

/// Default transport is TCP bound to localhost.
///
/// Security: localhost binding prevents accidental network exposure.
/// Explicit config required for external access.
#[test]
fn test_transport_config_default() {
    let config = TransportConfig::default();
    assert_eq!(config.transport_type, TransportType::Tcp);
    // Default to localhost for security
    assert_eq!(config.tcp.host, "127.0.0.1");
    assert_eq!(config.tcp.port, 50051);
}

// ============================================================================
// TcpConfig Tests
// ============================================================================

/// addr() formats host:port string.
///
/// Used when binding server or connecting client.
#[test]
fn test_tcp_addr() {
    let tcp = TcpConfig {
        host: "127.0.0.1".to_string(),
        port: 8080,
    };
    assert_eq!(tcp.addr(), "127.0.0.1:8080");
}

// ============================================================================
// UdsConfig Tests
// ============================================================================

/// socket_path() generates path for unqualified service.
///
/// Used for singleton services like gateway.
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

/// socket_path_qualified() uses {qualifier}-{service} naming.
///
/// Matches K8s service naming convention for consistency:
/// `orders-aggregate.sock`, not `aggregate-orders.sock`.
#[test]
fn test_uds_socket_path_qualified() {
    let uds = UdsConfig {
        base_path: PathBuf::from("/tmp/angzarr"),
    };
    // Uses {qualifier}-{service_name} order to match K8s naming convention
    assert_eq!(
        uds.socket_path_qualified("aggregate", "orders"),
        PathBuf::from("/tmp/angzarr/orders-aggregate.sock")
    );
    assert_eq!(
        uds.socket_path_qualified("projector", "accounting"),
        PathBuf::from("/tmp/angzarr/accounting-projector.sock")
    );
}

/// UdsCleanupGuard removes socket file on drop.
///
/// Prevents stale socket files from blocking server startup.
/// RAII pattern ensures cleanup even on panic.
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
