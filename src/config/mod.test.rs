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
