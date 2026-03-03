//! Tests for static service discovery.
//!
//! Static discovery provides service lookup without K8s dependencies:
//! - Services registered manually or via environment variables
//! - No background watching (unlike K8s discovery)
//! - Suitable for local development, Cloud Run, VMs
//!
//! Why this matters: Enables running angzarr outside K8s clusters. Local
//! development, Cloud Run, and VM deployments need service discovery
//! without requiring K8s infrastructure.
//!
//! Key behaviors verified:
//! - URL parsing handles UDS paths, HTTP/HTTPS, host:port formats
//! - Registration adds services to the appropriate cache
//! - Sync and async registration both work

use super::*;
use crate::discovery::ServiceDiscovery;

// ============================================================================
// URL Parsing Tests
// ============================================================================

/// UDS paths are recognized and parsed with port=0.
///
/// Unix Domain Socket paths start with `/` and don't have a port.
/// Port 0 indicates UDS transport, not TCP.
#[test]
fn test_parse_url_uds() {
    let (addr, port) = parse_url("/tmp/angzarr/test.sock").unwrap();
    assert_eq!(addr, "/tmp/angzarr/test.sock");
    assert_eq!(port, 0);
}

/// HTTP URLs are parsed correctly.
#[test]
fn test_parse_url_http() {
    let (addr, port) = parse_url("http://localhost:8080").unwrap();
    assert_eq!(addr, "localhost");
    assert_eq!(port, 8080);
}

/// HTTPS URLs default to port 443.
///
/// Cloud Run and other serverless platforms typically use HTTPS.
#[test]
fn test_parse_url_https() {
    let (addr, port) = parse_url("https://order-coordinator.run.app").unwrap();
    assert_eq!(addr, "order-coordinator.run.app");
    assert_eq!(port, 443);
}

/// HTTPS URLs with explicit port override default 443.
#[test]
fn test_parse_url_https_with_port() {
    let (addr, port) = parse_url("https://order-coordinator.run.app:8443").unwrap();
    assert_eq!(addr, "order-coordinator.run.app");
    assert_eq!(port, 8443);
}

/// Plain host:port format is parsed correctly.
///
/// Simple format for internal services without TLS.
#[test]
fn test_parse_url_host_port() {
    let (addr, port) = parse_url("localhost:50051").unwrap();
    assert_eq!(addr, "localhost");
    assert_eq!(port, 50051);
}

/// URL paths are stripped (only host:port used for gRPC).
///
/// gRPC uses host:port; URL paths are not part of the connection.
#[test]
fn test_parse_url_with_path() {
    let (addr, port) = parse_url("https://order-coordinator.run.app/api/v1").unwrap();
    assert_eq!(addr, "order-coordinator.run.app");
    assert_eq!(port, 443);
}

// ============================================================================
// Registration Tests
// ============================================================================

/// Aggregate registration adds service and updates has_aggregates().
#[tokio::test]
async fn test_static_discovery_register_aggregate() {
    let discovery = StaticServiceDiscovery::new();
    discovery
        .register_aggregate("order", "localhost", 50051)
        .await;

    assert!(discovery.has_aggregates().await);
    let domains = discovery.aggregate_domains().await;
    assert_eq!(domains, vec!["order"]);
}

/// Projector registration adds service and updates has_projectors().
#[tokio::test]
async fn test_static_discovery_register_projector() {
    let discovery = StaticServiceDiscovery::new();
    discovery
        .register_projector("web", "order", "localhost", 50052)
        .await;

    assert!(discovery.has_projectors().await);
}

/// Sync registration works for use in constructors (no async runtime).
///
/// Useful when initializing discovery from environment variables
/// before an async runtime is started.
#[test]
fn test_static_discovery_sync_registration() {
    let discovery = StaticServiceDiscovery::new();
    discovery.register_aggregate_sync("order", "localhost", 50051);
    discovery.register_projector_sync("web", "order", "localhost", 50052);

    assert!(!discovery.aggregates.blocking_read().is_empty());
    assert!(!discovery.projectors.blocking_read().is_empty());
}
