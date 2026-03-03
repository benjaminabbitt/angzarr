//! Tests for K8s service discovery.
//!
//! K8s discovery watches Service resources with specific labels:
//! - app.kubernetes.io/component: aggregate|projector
//! - angzarr.io/domain: {domain-name}
//!
//! Key behaviors verified:
//! - Service extraction parses labels and ports correctly
//! - gRPC URL construction for cluster-local DNS
//! - Default port fallback when grpc port not specified
//!
//! Note: Full K8s integration requires a running cluster.
//! Unit tests verify parsing logic without K8s API calls.

use super::*;
use k8s_openapi::api::core::v1::{ServicePort, ServiceSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use std::collections::BTreeMap;

/// Helper to create K8s Service objects for testing.
fn make_test_service(name: &str, component: &str, domain: Option<&str>, port: i32) -> Service {
    let mut labels = BTreeMap::new();
    labels.insert(COMPONENT_LABEL.to_string(), component.to_string());
    if let Some(d) = domain {
        labels.insert(DOMAIN_LABEL.to_string(), d.to_string());
    }

    Service {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some("test-ns".to_string()),
            labels: Some(labels),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            ports: Some(vec![ServicePort {
                name: Some("grpc".to_string()),
                port,
                ..Default::default()
            }]),
            ..Default::default()
        }),
        status: None,
    }
}

// ============================================================================
// Service Extraction Tests
// ============================================================================

/// Aggregate service extraction parses labels and builds cluster DNS.
#[test]
fn test_extract_aggregate_service() {
    let svc = make_test_service("cart-agg", COMPONENT_AGGREGATE, Some("cart"), 50051);
    let discovered = K8sServiceDiscovery::extract_service_with_namespace(&svc, "test-ns").unwrap();

    assert_eq!(discovered.name, "cart-agg");
    assert_eq!(
        discovered.service_address,
        "cart-agg.test-ns.svc.cluster.local"
    );
    assert_eq!(discovered.port, 50051);
    assert_eq!(discovered.domain, Some("cart".to_string()));
}

/// Projector service extraction works similarly.
#[test]
fn test_extract_projector_service() {
    let svc = make_test_service("cart-proj", COMPONENT_PROJECTOR, Some("cart"), 50052);
    let discovered = K8sServiceDiscovery::extract_service_with_namespace(&svc, "test-ns").unwrap();

    assert_eq!(discovered.name, "cart-proj");
    assert_eq!(discovered.domain, Some("cart".to_string()));
}

// ============================================================================
// URL Construction Tests
// ============================================================================

/// grpc_url() builds correct HTTP URL for gRPC connections.
#[test]
fn test_grpc_url() {
    let service = DiscoveredService {
        name: "test-svc".to_string(),
        service_address: "test-svc.ns.svc.cluster.local".to_string(),
        port: 50051,
        domain: None,
    };

    assert_eq!(
        service.grpc_url(),
        "http://test-svc.ns.svc.cluster.local:50051"
    );
}

/// Missing grpc port falls back to DEFAULT_GRPC_PORT.
#[test]
fn test_extract_service_without_grpc_port_uses_default() {
    let svc = Service {
        metadata: ObjectMeta {
            name: Some("test-svc".to_string()),
            namespace: Some("test-ns".to_string()),
            labels: Some({
                let mut l = BTreeMap::new();
                l.insert(COMPONENT_LABEL.to_string(), COMPONENT_AGGREGATE.to_string());
                l
            }),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            ports: None,
            ..Default::default()
        }),
        status: None,
    };

    let discovered = K8sServiceDiscovery::extract_service_with_namespace(&svc, "test-ns").unwrap();
    assert_eq!(discovered.port, DEFAULT_GRPC_PORT);
}
