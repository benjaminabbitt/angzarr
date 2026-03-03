//! Tests for RuntimeBuilder fluent configuration API.
//!
//! RuntimeBuilder provides a type-safe way to configure and construct
//! the standalone runtime. These tests verify:
//! - Default configuration is sensible (SQLite in-memory, channel bus)
//! - Builder methods correctly modify configuration
//! - Handler registration tracks registered domains/names
//! - Config accessors return correct values
//!
//! Why this matters: The builder pattern ensures invalid configurations
//! are caught at compile time rather than runtime. Tests verify that the
//! fluent API produces expected storage/messaging/transport settings.

use async_trait::async_trait;
use tonic::Status;

use super::*;
use crate::proto::{ContextualCommand, Cover, EventBook, Projection, SagaResponse};
use crate::standalone::traits::ProcessManagerHandleResult;
use crate::standalone::traits::ProjectionMode;

// ============================================================================
// Mock Handlers
// ============================================================================

struct MockCommandHandler;

#[async_trait]
impl CommandHandler for MockCommandHandler {
    async fn handle(&self, _command: ContextualCommand) -> Result<EventBook, Status> {
        Ok(EventBook::default())
    }
}

struct MockProjectorHandler;

#[async_trait]
impl ProjectorHandler for MockProjectorHandler {
    async fn handle(
        &self,
        _events: &EventBook,
        _mode: ProjectionMode,
    ) -> Result<Projection, Status> {
        Ok(Projection::default())
    }
}

struct MockSagaHandler;

#[async_trait]
impl SagaHandler for MockSagaHandler {
    async fn prepare(&self, _source: &EventBook) -> Result<Vec<Cover>, Status> {
        Ok(vec![])
    }

    async fn handle(
        &self,
        _source: &EventBook,
        _destinations: &[EventBook],
    ) -> Result<SagaResponse, Status> {
        Ok(SagaResponse::default())
    }
}

struct MockPMHandler;

impl ProcessManagerHandler for MockPMHandler {
    fn prepare(&self, _trigger: &EventBook, _process_state: Option<&EventBook>) -> Vec<Cover> {
        vec![]
    }

    fn handle(
        &self,
        _trigger: &EventBook,
        _process_state: Option<&EventBook>,
        _destinations: &[EventBook],
    ) -> ProcessManagerHandleResult {
        ProcessManagerHandleResult::default()
    }
}

// ============================================================================
// Default Configuration Tests
// ============================================================================

/// Builder defaults to SQLite in-memory, channel bus, UDS transport.
///
/// These defaults enable immediate local development without external
/// infrastructure (no database server, no message broker).
#[test]
fn test_builder_defaults() {
    let builder = RuntimeBuilder::new();

    assert_eq!(builder.storage.storage_type, "sqlite");
    assert!(builder.storage.sqlite.path.is_none());
    assert_eq!(builder.messaging.messaging_type, "channel".to_string());
    assert_eq!(builder.transport.transport_type, TransportType::Uds);
}

// ============================================================================
// Storage Configuration Tests
// ============================================================================

/// SQLite file storage is configured correctly.
#[test]
fn test_builder_sqlite_file() {
    let builder = RuntimeBuilder::new().with_sqlite_file("./data/events.db");

    assert_eq!(builder.storage.storage_type, "sqlite");
    assert_eq!(
        builder.storage.sqlite.path,
        Some("./data/events.db".to_string())
    );
}

/// PostgreSQL storage is configured correctly.
#[test]
fn test_builder_postgres() {
    let builder = RuntimeBuilder::new().with_postgres("postgres://localhost:5432/test");

    assert_eq!(builder.storage.storage_type, "postgres");
    assert_eq!(
        builder.storage.postgres.uri,
        "postgres://localhost:5432/test"
    );
}

// ============================================================================
// Messaging Configuration Tests
// ============================================================================

/// AMQP messaging is configured correctly.
#[test]
fn test_builder_amqp() {
    let builder = RuntimeBuilder::new().with_amqp("amqp://localhost:5672");

    assert_eq!(builder.messaging.messaging_type, "amqp".to_string());
    assert_eq!(builder.messaging.amqp.url, "amqp://localhost:5672");
}

// ============================================================================
// Handler Registration Accessor Tests
// ============================================================================

/// No command handlers registered by default.
#[test]
fn test_command_handler_domains_empty_by_default() {
    let builder = RuntimeBuilder::new();
    assert!(builder.command_handler_domains().is_empty());
}

/// Registered command handler domains are tracked.
#[test]
fn test_command_handler_domains_returns_registered() {
    let builder = RuntimeBuilder::new()
        .register_command_handler("domain1", MockCommandHandler)
        .register_command_handler("domain2", MockCommandHandler);

    let domains = builder.command_handler_domains();
    assert_eq!(domains.len(), 2);
    assert!(domains.contains(&"domain1"));
    assert!(domains.contains(&"domain2"));
}

/// Unregistered domains are not reported.
#[test]
fn test_command_handler_domains_excludes_unregistered() {
    let builder = RuntimeBuilder::new().register_command_handler("registered", MockCommandHandler);

    let domains = builder.command_handler_domains();
    assert!(!domains.contains(&"unregistered"));
}

/// No projectors registered by default.
#[test]
fn test_projector_names_empty_by_default() {
    let builder = RuntimeBuilder::new();
    assert!(builder.projector_names().is_empty());
}

/// Registered projector names are tracked.
#[test]
fn test_projector_names_returns_registered() {
    let builder = RuntimeBuilder::new()
        .register_projector("proj1", MockProjectorHandler, ProjectorConfig::default())
        .register_projector("proj2", MockProjectorHandler, ProjectorConfig::default());

    let names = builder.projector_names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"proj1"));
    assert!(names.contains(&"proj2"));
}

/// No sagas registered by default.
#[test]
fn test_saga_names_empty_by_default() {
    let builder = RuntimeBuilder::new();
    assert!(builder.saga_names().is_empty());
}

/// Registered saga names are tracked.
#[test]
fn test_saga_names_returns_registered() {
    let builder = RuntimeBuilder::new()
        .register_saga("saga1", MockSagaHandler, SagaConfig::new("input", "output"))
        .register_saga("saga2", MockSagaHandler, SagaConfig::new("input", "output"));

    let names = builder.saga_names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"saga1"));
    assert!(names.contains(&"saga2"));
}

/// No process managers registered by default.
#[test]
fn test_process_manager_names_empty_by_default() {
    let builder = RuntimeBuilder::new();
    assert!(builder.process_manager_names().is_empty());
}

/// Registered process manager names are tracked.
#[test]
fn test_process_manager_names_returns_registered() {
    let builder = RuntimeBuilder::new()
        .register_process_manager("pm1", MockPMHandler, ProcessManagerConfig::new("pm-domain"))
        .register_process_manager("pm2", MockPMHandler, ProcessManagerConfig::new("pm-domain"));

    let names = builder.process_manager_names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"pm1"));
    assert!(names.contains(&"pm2"));
}

// ============================================================================
// Config Accessor Tests
// ============================================================================

/// storage_config() returns the configured storage settings.
#[test]
fn test_storage_config_returns_configured() {
    let builder = RuntimeBuilder::new().with_postgres("postgres://test");
    let config = builder.storage_config();
    assert_eq!(config.storage_type, "postgres");
    assert_eq!(config.postgres.uri, "postgres://test");
}

/// messaging_config() returns the configured messaging settings.
#[test]
fn test_messaging_config_returns_configured() {
    let builder = RuntimeBuilder::new().with_amqp("amqp://test");
    let config = builder.messaging_config();
    assert_eq!(config.messaging_type, "amqp");
    assert_eq!(config.amqp.url, "amqp://test");
}

/// transport_config() returns the configured transport settings.
#[test]
fn test_transport_config_returns_configured() {
    let builder = RuntimeBuilder::new().with_tcp();
    let config = builder.transport_config();
    assert_eq!(config.transport_type, TransportType::Tcp);
}

// ============================================================================
// Builder Chaining Tests
// ============================================================================

/// with_sqlite_memory() returns self for chaining.
#[test]
fn test_with_sqlite_memory_returns_configured_builder() {
    let builder = RuntimeBuilder::new().with_sqlite_memory();
    assert_eq!(builder.storage.storage_type, "sqlite");
    assert!(builder.storage.sqlite.path.is_none());
}

/// with_channel_messaging() overrides previous messaging config.
#[test]
fn test_with_channel_messaging_returns_configured_builder() {
    let builder = RuntimeBuilder::new()
        .with_kafka("kafka:9092") // Set non-channel first
        .with_channel_messaging(); // Then switch to channel

    assert_eq!(builder.messaging.messaging_type, "channel");
}

/// with_uds() overrides previous transport config.
#[test]
fn test_with_uds_returns_configured_builder() {
    let builder = RuntimeBuilder::new()
        .with_tcp() // Set TCP first
        .with_uds(); // Then switch to UDS

    assert_eq!(builder.transport.transport_type, TransportType::Uds);
}

/// with_kafka() configures Kafka messaging.
#[test]
fn test_with_kafka_returns_configured_builder() {
    let builder = RuntimeBuilder::new().with_kafka("kafka:9092");
    assert_eq!(builder.messaging.messaging_type, "kafka");
    assert_eq!(builder.messaging.kafka.bootstrap_servers, "kafka:9092");
}
