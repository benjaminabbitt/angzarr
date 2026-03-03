//! Tests for configuration loading and defaults.
//!
//! The Config struct aggregates all angzarr configuration from YAML files
//! and environment variables. These tests verify:
//! - Default values are sensible and secure
//! - Test configuration helpers work correctly
//!
//! Why this matters: Configuration errors are runtime failures that can
//! cause silent misbehavior. Defaults must be safe (localhost binding)
//! and explicit configuration required for production features.
//!
//! Integration tests for file loading are in Gherkin tests.

use super::*;

/// Default config has expected ports and no optional services.
///
/// Verifies baseline config is minimal and requires explicit
/// configuration for optional features.
#[test]
fn test_config_default() {
    let config = Config::default();
    assert_eq!(config.server.ch_port, 1313);
    assert!(config.messaging.is_none());
    assert!(config.target.is_none());
}

/// Test config defaults to localhost for security.
///
/// Prevents accidental exposure to network when running tests.
#[test]
fn test_config_for_test() {
    let config = Config::for_test();
    assert_eq!(config.server.host, "127.0.0.1");
}

// ============================================================================
// config_base_dir Tests
// ============================================================================

/// config_base_dir returns current dir when CONFIG_ENV_VAR is not set.
#[test]
fn test_config_base_dir_no_env() {
    // Ensure env var is not set for this test
    std::env::remove_var(CONFIG_ENV_VAR);
    let base = config_base_dir();
    assert_eq!(base, std::path::PathBuf::from("."));
}

/// config_base_dir returns parent directory when CONFIG_ENV_VAR is set.
#[test]
fn test_config_base_dir_with_env() {
    // Set env var to a path with a parent
    let original = std::env::var(CONFIG_ENV_VAR).ok();
    std::env::set_var(CONFIG_ENV_VAR, "/some/path/to/config.yaml");

    let base = config_base_dir();
    assert_eq!(base, std::path::PathBuf::from("/some/path/to"));

    // Restore original value
    match original {
        Some(val) => std::env::set_var(CONFIG_ENV_VAR, val),
        None => std::env::remove_var(CONFIG_ENV_VAR),
    }
}

// ============================================================================
// Constant Tests
// ============================================================================

/// Environment variable constants have expected values.
#[test]
fn test_env_var_constants() {
    assert_eq!(CONFIG_ENV_VAR, "ANGZARR_CONFIG");
    assert_eq!(CONFIG_ENV_PREFIX, "ANGZARR");
    assert_eq!(DEFAULT_CONFIG_FILE, "config.yaml");
    assert_eq!(LOG_ENV_VAR, "ANGZARR_LOG");
    assert_eq!(DISCOVERY_STATIC, "static");
}
