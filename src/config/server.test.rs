//! Tests for server and networking configuration.
//!
//! Server config controls how angzarr binds to ports and where services
//! connect. ServiceConfig supports both inline definitions and file
//! references for modular configuration.
//!
//! Why this matters: Server binding affects security (localhost vs all
//! interfaces) and connectivity (port conflicts). ServiceConfig resolution
//! enables modular config files for complex deployments.
//!
//! Security: Default host is localhost (127.0.0.1), not 0.0.0.0.
//! External access requires explicit configuration.

use std::path::PathBuf;

use super::*;

// ============================================================================
// ServerConfig Tests
// ============================================================================

/// Server defaults to standard ports and localhost binding.
///
/// Security: localhost binding prevents accidental network exposure.
/// Must explicitly set host = "0.0.0.0" for external access.
#[test]
fn test_server_config_default() {
    let server = ServerConfig::default();
    assert_eq!(server.ch_port, 1313);
    assert_eq!(server.event_query_port, 1314);
    assert_eq!(server.host, "127.0.0.1");
}

// ============================================================================
// ServiceConfigRef Tests
// ============================================================================

/// Inline service config parses domain and command.
///
/// Most services are defined inline in the main config file.
#[test]
fn test_service_config_inline_deserialization() {
    let yaml = r#"
        domain: cart
        command: ["python", "server.py"]
    "#;
    let config: ServiceConfigRef = serde_yaml::from_str(yaml).unwrap();
    match config {
        ServiceConfigRef::Inline(svc) => {
            assert_eq!(svc.domain, "cart");
            assert_eq!(svc.command, vec!["python", "server.py"]);
        }
        ServiceConfigRef::File { .. } => panic!("Expected inline config"),
    }
}

/// File reference config supports overrides.
///
/// Large service definitions can be extracted to separate files.
/// Overrides (like storage) are merged at load time.
#[test]
fn test_service_config_file_ref_deserialization() {
    let yaml = r#"
        file: config/cart.yaml
        storage:
          type: sqlite
    "#;
    let config: ServiceConfigRef = serde_yaml::from_str(yaml).unwrap();
    match config {
        ServiceConfigRef::File { file, overrides } => {
            assert_eq!(file, PathBuf::from("config/cart.yaml"));
            assert!(overrides.storage.is_some());
        }
        ServiceConfigRef::Inline(_) => panic!("Expected file reference"),
    }
}

// ============================================================================
// ServiceConfig::resolve_address Tests
// ============================================================================

/// Explicit address is returned as-is (no derivation).
///
/// Users can override the derived address for special cases.
#[test]
fn test_resolve_address_explicit_address() {
    use crate::transport::{TcpConfig, TransportConfig, TransportType, UdsConfig};

    let config = ServiceConfig {
        domain: "cart".to_string(),
        name: None,
        address: Some("http://custom:8080".to_string()),
        port: None,
        socket: None,
        working_dir: None,
        command: vec![],
        listen_domain: None,
        subscriptions: None,
        env: std::collections::HashMap::new(),
        storage: None,
    };

    let transport = TransportConfig {
        transport_type: TransportType::Uds,
        tcp: TcpConfig::default(),
        uds: UdsConfig {
            base_path: PathBuf::from("/tmp/sockets"),
        },
    };

    let result = config.resolve_address(&transport, "agg");
    assert_eq!(result.unwrap(), "http://custom:8080");
}

/// UDS transport derives socket path from domain and service type.
#[test]
fn test_resolve_address_uds_no_name() {
    use crate::transport::{TcpConfig, TransportConfig, TransportType, UdsConfig};

    let config = ServiceConfig {
        domain: "cart".to_string(),
        name: None,
        address: None,
        port: None,
        socket: None,
        working_dir: None,
        command: vec![],
        listen_domain: None,
        subscriptions: None,
        env: std::collections::HashMap::new(),
        storage: None,
    };

    let transport = TransportConfig {
        transport_type: TransportType::Uds,
        tcp: TcpConfig::default(),
        uds: UdsConfig {
            base_path: PathBuf::from("/tmp/sockets"),
        },
    };

    let result = config.resolve_address(&transport, "agg");
    assert_eq!(result.unwrap(), "/tmp/sockets/agg-cart.sock");
}

/// UDS transport with name includes it in socket path.
///
/// Used for projectors with multiple instances per domain.
#[test]
fn test_resolve_address_uds_with_name() {
    use crate::transport::{TcpConfig, TransportConfig, TransportType, UdsConfig};

    let config = ServiceConfig {
        domain: "inventory".to_string(),
        name: Some("stock".to_string()),
        address: None,
        port: None,
        socket: None,
        working_dir: None,
        command: vec![],
        listen_domain: None,
        subscriptions: None,
        env: std::collections::HashMap::new(),
        storage: None,
    };

    let transport = TransportConfig {
        transport_type: TransportType::Uds,
        tcp: TcpConfig::default(),
        uds: UdsConfig {
            base_path: PathBuf::from("/var/run/angzarr"),
        },
    };

    let result = config.resolve_address(&transport, "prj");
    assert_eq!(result.unwrap(), "/var/run/angzarr/prj-stock-inventory.sock");
}

/// TCP transport without explicit address returns error.
///
/// TCP requires explicit host:port - cannot be derived.
#[test]
fn test_resolve_address_tcp_requires_explicit() {
    use crate::transport::{TcpConfig, TransportConfig, TransportType, UdsConfig};

    let config = ServiceConfig {
        domain: "cart".to_string(),
        name: None,
        address: None,
        port: None,
        socket: None,
        working_dir: None,
        command: vec![],
        listen_domain: None,
        subscriptions: None,
        env: std::collections::HashMap::new(),
        storage: None,
    };

    let transport = TransportConfig {
        transport_type: TransportType::Tcp,
        tcp: TcpConfig::default(),
        uds: UdsConfig::default(),
    };

    let result = config.resolve_address(&transport, "agg");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err
        .to_string()
        .contains("TCP transport requires explicit address"));
}

// ============================================================================
// ConfigError Tests
// ============================================================================

/// ConfigError::FileRead formats correctly.
#[test]
fn test_config_error_file_read_display() {
    let err = ConfigError::FileRead("/path/to/file.yaml".to_string(), "not found".to_string());
    assert_eq!(
        err.to_string(),
        "Failed to read '/path/to/file.yaml': not found"
    );
}

/// ConfigError::Parse formats correctly.
#[test]
fn test_config_error_parse_display() {
    let err = ConfigError::Parse("config.yaml".to_string(), "invalid YAML".to_string());
    assert_eq!(
        err.to_string(),
        "Failed to parse 'config.yaml': invalid YAML"
    );
}

// ============================================================================
// RegistrationConfig Tests
// ============================================================================

/// RegistrationConfig default uses fixed strategy.
#[test]
fn test_registration_config_default() {
    let config = RegistrationConfig::default();
    assert_eq!(config.strategy, "fixed");
    assert_eq!(config.interval_secs, 30);
    assert_eq!(config.initial_secs, 1);
    assert_eq!(config.max_secs, 60);
    assert_eq!(config.multiplier, 2.0);
    assert!(config.jitter);
}

/// build_strategy creates FixedInterval for "fixed" strategy.
#[test]
fn test_registration_config_build_fixed_strategy() {
    let config = RegistrationConfig {
        strategy: "fixed".to_string(),
        interval_secs: 10,
        ..Default::default()
    };

    let strategy = config.build_strategy();

    // Fixed interval should return Some for any attempt
    assert!(strategy.next_delay(0).is_some());
    assert!(strategy.next_delay(100).is_some());
}

/// build_strategy creates ExponentialBackoff for "exponential" strategy.
#[test]
fn test_registration_config_build_exponential_strategy() {
    let config = RegistrationConfig {
        strategy: "exponential".to_string(),
        initial_secs: 2,
        max_secs: 30,
        multiplier: 3.0,
        jitter: false,
        ..Default::default()
    };

    let strategy = config.build_strategy();

    // Exponential should return increasing delays
    let d0 = strategy.next_delay(0).unwrap();
    let d1 = strategy.next_delay(1).unwrap();
    assert!(d1 > d0, "Exponential backoff should increase");
}
