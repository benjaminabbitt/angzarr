//! Tests for process management utilities.
//!
//! Process management handles spawning child processes for business logic:
//! - ProcessEnv converts transport config to environment variables
//! - ManagedProcess wraps child process lifecycle
//! - wait_for_ready polls until a service accepts connections
//!
//! Why this matters: In standalone mode, business logic runs as separate processes
//! (Python, Go, etc.) that communicate via gRPC. If env var translation is wrong,
//! processes can't find their sockets. If wait_for_ready fails, the coordinator
//! starts routing commands before handlers are listening.
//!
//! Key behaviors verified:
//! - UDS transport produces correct env vars (TRANSPORT_TYPE, UDS_BASE_PATH)
//! - TCP transport produces PORT instead of UDS_BASE_PATH
//! - Domain and service name are always included

use std::path::PathBuf;

use super::*;
use crate::transport::{TcpConfig, TransportConfig, TransportType, UdsConfig};

// ============================================================================
// ProcessEnv::to_env_vars Tests
// ============================================================================

/// UDS transport produces TRANSPORT_TYPE=uds and UDS_BASE_PATH.
///
/// UDS is the default for standalone mode - processes connect via Unix domain
/// sockets in a shared directory. The base path tells spawned processes where
/// to create/connect to sockets.
#[test]
fn test_process_env_to_env_vars() {
    let env = ProcessEnv {
        transport_type: "uds".to_string(),
        uds_base_path: Some("/tmp/angzarr".to_string()),
        service_name: "business".to_string(),
        domain: Some("customer".to_string()),
        port: None,
    };

    let vars = env.to_env_vars();
    assert_eq!(vars.get("TRANSPORT_TYPE"), Some(&"uds".to_string()));
    assert_eq!(vars.get("UDS_BASE_PATH"), Some(&"/tmp/angzarr".to_string()));
    assert_eq!(vars.get("SERVICE_NAME"), Some(&"business".to_string()));
    assert_eq!(vars.get("DOMAIN"), Some(&"customer".to_string()));
}

/// TCP transport produces TRANSPORT_TYPE=tcp and PORT, omits UDS_BASE_PATH.
///
/// TCP mode is used when processes can't share a filesystem (containers,
/// different hosts). PORT tells the process which port to listen on.
#[test]
fn test_process_env_tcp() {
    let env = ProcessEnv {
        transport_type: "tcp".to_string(),
        uds_base_path: None,
        service_name: "business".to_string(),
        domain: Some("order".to_string()),
        port: Some(50051),
    };

    let vars = env.to_env_vars();
    assert_eq!(vars.get("TRANSPORT_TYPE"), Some(&"tcp".to_string()));
    assert_eq!(vars.get("PORT"), Some(&"50051".to_string()));
    assert!(!vars.contains_key("UDS_BASE_PATH"));
}

/// ProcessEnv without domain omits DOMAIN from env vars.
///
/// Some services (like global projectors) don't belong to a specific domain.
/// Omitting DOMAIN lets the process know it's domain-agnostic.
#[test]
fn test_process_env_without_domain() {
    let env = ProcessEnv {
        transport_type: "uds".to_string(),
        uds_base_path: Some("/tmp/angzarr".to_string()),
        service_name: "projector".to_string(),
        domain: None,
        port: None,
    };

    let vars = env.to_env_vars();
    assert!(!vars.contains_key("DOMAIN"));
    assert!(vars.contains_key("SERVICE_NAME"));
}

/// ProcessEnv without port omits PORT from env vars.
///
/// UDS mode doesn't use ports - processes derive socket paths from
/// UDS_BASE_PATH + SERVICE_NAME + DOMAIN.
#[test]
fn test_process_env_without_port() {
    let env = ProcessEnv {
        transport_type: "uds".to_string(),
        uds_base_path: Some("/tmp/sockets".to_string()),
        service_name: "saga".to_string(),
        domain: Some("orders".to_string()),
        port: None,
    };

    let vars = env.to_env_vars();
    assert!(!vars.contains_key("PORT"));
}

// ============================================================================
// ProcessEnv::from_transport Tests
// ============================================================================

/// from_transport creates UDS ProcessEnv correctly.
///
/// This is the primary constructor - takes TransportConfig and extracts
/// the fields spawned processes need to connect back to the coordinator.
#[test]
fn test_process_env_from_transport_uds() {
    let config = TransportConfig {
        transport_type: TransportType::Uds,
        tcp: TcpConfig::default(),
        uds: UdsConfig {
            base_path: PathBuf::from("/var/run/angzarr"),
        },
    };

    let env = ProcessEnv::from_transport(&config, "business", Some("orders"));

    assert_eq!(env.transport_type, "uds");
    assert_eq!(env.uds_base_path, Some("/var/run/angzarr".to_string()));
    assert_eq!(env.service_name, "business");
    assert_eq!(env.domain, Some("orders".to_string()));
}

/// from_transport creates TCP ProcessEnv correctly.
///
/// TCP mode sets transport_type but doesn't set port - that's determined
/// by the spawning code based on port allocation strategy.
#[test]
fn test_process_env_from_transport_tcp() {
    let config = TransportConfig {
        transport_type: TransportType::Tcp,
        tcp: TcpConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
        },
        uds: UdsConfig::default(),
    };

    let env = ProcessEnv::from_transport(&config, "saga", None);

    assert_eq!(env.transport_type, "tcp");
    assert_eq!(env.service_name, "saga");
    assert_eq!(env.domain, None);
}

/// from_transport with no domain passes None through.
///
/// Domain-agnostic services (global projectors, topology) don't need
/// a domain qualifier in their socket/port configuration.
#[test]
fn test_process_env_from_transport_no_domain() {
    let config = TransportConfig::default();

    let env = ProcessEnv::from_transport(&config, "projector", None);

    assert!(env.domain.is_none());
}

// ============================================================================
// Error Message Constants Tests
// ============================================================================

/// Error constants must be non-empty for meaningful error messages.
#[test]
fn test_errmsg_constants_are_non_empty() {
    assert!(!errmsg::COMMAND_EMPTY.is_empty());
    assert!(!errmsg::TIMEOUT_WAITING.is_empty());
}

/// TIMEOUT_WAITING is a prefix that gets an address appended.
///
/// Format: "Timeout waiting for service at <address>: <error>"
#[test]
fn test_errmsg_timeout_is_prefix() {
    assert!(errmsg::TIMEOUT_WAITING.ends_with(' '));
}
