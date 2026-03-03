//! Integration tests for angzarr binary entry points.
//!
//! These tests verify that binaries:
//! - Exit with errors for missing required configuration
//! - Exit with errors for invalid configuration files
//! - Parse command-line arguments correctly
//!
//! Why this matters: Binary startup errors should be clear and helpful.
//! A user running a binary without proper config should get a useful
//! error message, not a cryptic panic or silent failure.

use std::fs;
use std::process::Command;
use tempfile::tempdir;

/// Helper to get the path to a built binary.
fn binary_path(name: &str) -> String {
    format!("./target/debug/{}", name)
}

/// Helper to run a binary and capture output.
fn run_binary(name: &str, args: &[&str]) -> std::process::Output {
    Command::new(binary_path(name))
        .args(args)
        .output()
        .expect("Failed to execute binary")
}

// ============================================================================
// angzarr-aggregate Tests
// ============================================================================

/// angzarr-aggregate fails when config file doesn't exist.
///
/// Clear error message helps users fix configuration issues.
#[test]
fn test_aggregate_fails_for_missing_config() {
    let output = run_binary("angzarr-aggregate", &["-c", "/nonexistent/config.yaml"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found")
            || stderr.contains("No such file")
            || stderr.contains("Failed to load"),
        "Should report config file not found, got: {}",
        stderr
    );
}

/// angzarr-aggregate fails when target config is missing.
///
/// Aggregate sidecar requires a target domain to be configured.
#[test]
fn test_aggregate_fails_without_target() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("minimal.yaml");

    // Config with server and valid storage but no target
    let config_content = r#"
server:
  ch_port: 1313
storage:
  type: "sqlite"
"#;
    fs::write(&config_path, config_content).unwrap();

    let output = run_binary("angzarr-aggregate", &["-c", config_path.to_str().unwrap()]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("target") || stderr.contains("Target"),
        "Should report missing target config, got: {}",
        stderr
    );
}

// ============================================================================
// angzarr-projector Tests
// ============================================================================

/// angzarr-projector fails when config file doesn't exist.
#[test]
fn test_projector_fails_for_missing_config() {
    let output = run_binary("angzarr-projector", &["-c", "/nonexistent/config.yaml"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found")
            || stderr.contains("No such file")
            || stderr.contains("Failed to load"),
        "Should report config file not found, got: {}",
        stderr
    );
}

/// angzarr-projector fails without target config.
#[test]
fn test_projector_fails_without_target() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("minimal.yaml");

    let config_content = r#"
server:
  ch_port: 1313
storage:
  type: "sqlite"
"#;
    fs::write(&config_path, config_content).unwrap();

    let output = run_binary("angzarr-projector", &["-c", config_path.to_str().unwrap()]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("target") || stderr.contains("Target"),
        "Should report missing target config, got: {}",
        stderr
    );
}

// ============================================================================
// angzarr-saga Tests
// ============================================================================

/// angzarr-saga fails when config file doesn't exist.
#[test]
fn test_saga_fails_for_missing_config() {
    let output = run_binary("angzarr-saga", &["-c", "/nonexistent/config.yaml"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found")
            || stderr.contains("No such file")
            || stderr.contains("Failed to load"),
        "Should report config file not found, got: {}",
        stderr
    );
}

/// angzarr-saga fails without target config.
#[test]
fn test_saga_fails_without_target() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("minimal.yaml");

    let config_content = r#"
server:
  ch_port: 1313
storage:
  type: "sqlite"
"#;
    fs::write(&config_path, config_content).unwrap();

    let output = run_binary("angzarr-saga", &["-c", config_path.to_str().unwrap()]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("target") || stderr.contains("Target"),
        "Should report missing target config, got: {}",
        stderr
    );
}

// ============================================================================
// angzarr-process-manager Tests
// ============================================================================

/// angzarr-process-manager fails when config file doesn't exist.
#[test]
fn test_process_manager_fails_for_missing_config() {
    let output = run_binary(
        "angzarr-process-manager",
        &["-c", "/nonexistent/config.yaml"],
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found")
            || stderr.contains("No such file")
            || stderr.contains("Failed to load"),
        "Should report config file not found, got: {}",
        stderr
    );
}

/// angzarr-process-manager fails without target config.
#[test]
fn test_process_manager_fails_without_target() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("minimal.yaml");

    let config_content = r#"
server:
  ch_port: 1313
storage:
  type: "sqlite"
"#;
    fs::write(&config_path, config_content).unwrap();

    let output = run_binary(
        "angzarr-process-manager",
        &["-c", config_path.to_str().unwrap()],
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("target") || stderr.contains("Target"),
        "Should report missing target config, got: {}",
        stderr
    );
}

// ============================================================================
// angzarr-stream Tests
// ============================================================================

/// angzarr-stream fails when config file doesn't exist.
#[test]
fn test_stream_fails_for_missing_config() {
    let output = run_binary("angzarr-stream", &["-c", "/nonexistent/config.yaml"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found")
            || stderr.contains("No such file")
            || stderr.contains("Failed to load"),
        "Should report config file not found, got: {}",
        stderr
    );
}

// ============================================================================
// angzarr-log Tests
// ============================================================================

/// angzarr-log fails when config file doesn't exist.
#[test]
fn test_log_fails_for_missing_config() {
    let output = run_binary("angzarr-log", &["-c", "/nonexistent/config.yaml"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found")
            || stderr.contains("No such file")
            || stderr.contains("Failed to load"),
        "Should report config file not found, got: {}",
        stderr
    );
}

// ============================================================================
// angzarr-upcaster Tests
// ============================================================================

/// angzarr-upcaster fails when config file doesn't exist.
#[test]
fn test_upcaster_fails_for_missing_config() {
    let output = run_binary("angzarr-upcaster", &["-c", "/nonexistent/config.yaml"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found")
            || stderr.contains("No such file")
            || stderr.contains("Failed to load"),
        "Should report config file not found, got: {}",
        stderr
    );
}

// ============================================================================
// Invalid YAML Tests
// ============================================================================

/// All binaries fail gracefully with invalid YAML syntax.
#[test]
fn test_aggregate_fails_for_invalid_yaml() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("invalid.yaml");

    let invalid_yaml = "server:\n  ch_port: [invalid";
    fs::write(&config_path, invalid_yaml).unwrap();

    let output = run_binary("angzarr-aggregate", &["-c", config_path.to_str().unwrap()]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Failed to load") || stderr.contains("error") || stderr.contains("invalid"),
        "Should report invalid YAML, got: {}",
        stderr
    );
}
