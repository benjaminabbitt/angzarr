//! OTel-native metrics instruments.
//!
//! Centralized metric definitions using OpenTelemetry meters.
//! All instruments are lazily initialized and feature-gated behind `otel`.
//!
//! Naming follows OTel semantic conventions (dot-separated).
//! The OTel Collector / Prometheus exporter converts dots to underscores.

use std::sync::LazyLock;

use opentelemetry::metrics::{Counter, Histogram, Meter};
use opentelemetry::{global, KeyValue};

static METER: LazyLock<Meter> = LazyLock::new(|| global::meter("angzarr"));

// ============================================================================
// Command Pipeline
// ============================================================================

/// Duration of command handling (handle, handle_sync, dry_run).
pub static COMMAND_DURATION: LazyLock<Histogram<f64>> = LazyLock::new(|| {
    METER
        .f64_histogram("angzarr.command.duration")
        .with_description("Command handling duration")
        .with_unit("s")
        .build()
});

/// Total commands processed.
pub static COMMAND_TOTAL: LazyLock<Counter<u64>> = LazyLock::new(|| {
    METER
        .u64_counter("angzarr.command.total")
        .with_description("Total commands processed")
        .build()
});

// ============================================================================
// Event Bus
// ============================================================================

/// Duration of event bus publish operations.
pub static BUS_PUBLISH_DURATION: LazyLock<Histogram<f64>> = LazyLock::new(|| {
    METER
        .f64_histogram("angzarr.bus.publish.duration")
        .with_description("Event bus publish duration")
        .with_unit("s")
        .build()
});

/// Total event bus publish operations.
pub static BUS_PUBLISH_TOTAL: LazyLock<Counter<u64>> = LazyLock::new(|| {
    METER
        .u64_counter("angzarr.bus.publish.total")
        .with_description("Total event bus publish operations")
        .build()
});

// ============================================================================
// Saga
// ============================================================================

/// Duration of saga orchestration.
pub static SAGA_DURATION: LazyLock<Histogram<f64>> = LazyLock::new(|| {
    METER
        .f64_histogram("angzarr.saga.duration")
        .with_description("Saga orchestration duration")
        .with_unit("s")
        .build()
});

/// Total saga retry attempts.
pub static SAGA_RETRY_TOTAL: LazyLock<Counter<u64>> = LazyLock::new(|| {
    METER
        .u64_counter("angzarr.saga.retry.total")
        .with_description("Total saga retry attempts")
        .build()
});

/// Total saga compensations triggered.
pub static SAGA_COMPENSATION_TOTAL: LazyLock<Counter<u64>> = LazyLock::new(|| {
    METER
        .u64_counter("angzarr.saga.compensation.total")
        .with_description("Total saga compensations triggered")
        .build()
});

// ============================================================================
// Process Manager
// ============================================================================

/// Duration of process manager orchestration.
pub static PM_DURATION: LazyLock<Histogram<f64>> = LazyLock::new(|| {
    METER
        .f64_histogram("angzarr.pm.duration")
        .with_description("Process manager orchestration duration")
        .with_unit("s")
        .build()
});

// ============================================================================
// Projector
// ============================================================================

/// Duration of projector event handling.
pub static PROJECTOR_DURATION: LazyLock<Histogram<f64>> = LazyLock::new(|| {
    METER
        .f64_histogram("angzarr.projector.duration")
        .with_description("Projector event handling duration")
        .with_unit("s")
        .build()
});

// ============================================================================
// Dead Letter Queue
// ============================================================================

/// Total DLQ publish operations.
pub static DLQ_PUBLISH_TOTAL: LazyLock<Counter<u64>> = LazyLock::new(|| {
    METER
        .u64_counter("angzarr.dlq.publish.total")
        .with_description("Total DLQ publish operations")
        .build()
});

/// Duration of DLQ publish operations.
pub static DLQ_PUBLISH_DURATION: LazyLock<Histogram<f64>> = LazyLock::new(|| {
    METER
        .f64_histogram("angzarr.dlq.publish.duration")
        .with_description("DLQ publish duration")
        .with_unit("s")
        .build()
});

// ============================================================================
// Helper
// ============================================================================

/// Create a domain label.
pub fn domain_attr(domain: &str) -> KeyValue {
    KeyValue::new("domain", domain.to_string())
}

/// Create an outcome label.
pub fn outcome_attr(outcome: &str) -> KeyValue {
    KeyValue::new("outcome", outcome.to_string())
}

/// Create a bus_type label.
pub fn bus_type_attr(bus_type: &str) -> KeyValue {
    KeyValue::new("bus_type", bus_type.to_string())
}

/// Create a component label (aggregate, projector, saga, process_manager).
pub fn component_attr(component: &str) -> KeyValue {
    KeyValue::new("component", component.to_string())
}

/// Create a name label (specific component instance name).
pub fn name_attr(name: &str) -> KeyValue {
    KeyValue::new("name", name.to_string())
}

/// Create a reason_type label for DLQ entries.
pub fn reason_type_attr(reason_type: &str) -> KeyValue {
    KeyValue::new("reason_type", reason_type.to_string())
}

/// Create a backend label for DLQ entries.
pub fn backend_attr(backend: &str) -> KeyValue {
    KeyValue::new("backend", backend.to_string())
}
