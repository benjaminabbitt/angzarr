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
