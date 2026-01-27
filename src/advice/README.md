# Advice Module

Cross-cutting concerns applied transparently to core implementations.

## Terminology

If you're coming from the Rust ecosystem, you may know this pattern as:
- **Layers** (Tower)
- **Middleware** (HTTP/gRPC)
- **Decorators/Wrappers** (general Rust)

The term "advice" comes from Aspect-Oriented Programming (AOP), where:
- **Advice** = behavior applied at specific points
- **Join points** = where advice can be applied (method calls, etc.)

We use "advice" to emphasize these are orthogonal concerns (metrics, tracing, retries)
that don't belong in business logic.

## Available Advice

| Advice | Purpose |
|--------|---------|
| `Instrumented<T>` | Metrics (counters, histograms) for storage operations |

## Usage

```rust
use angzarr::advice::Instrumented;
use angzarr::storage::{SqliteEventStore, init_storage_instrumented};

// Option 1: Use instrumented factory
let (events, snapshots) = init_storage_instrumented(&config).await?;

// Option 2: Wrap manually
let store = SqliteEventStore::new(pool);
let store = Instrumented::new(store, "sqlite");
let store: Arc<dyn EventStore> = Arc::new(store);
```

## Metrics Emitted

Requires a `metrics` recorder (e.g., `metrics-exporter-prometheus`, `opentelemetry-otlp`).

| Metric | Type | Labels |
|--------|------|--------|
| `angzarr_events_stored_total` | Counter | domain, storage |
| `angzarr_events_loaded_total` | Counter | domain, storage |
| `angzarr_snapshots_stored_total` | Counter | namespace, storage |
| `angzarr_snapshots_loaded_total` | Counter | namespace, storage |
| `angzarr_positions_updated_total` | Counter | handler, domain, storage |
| `angzarr_storage_duration_seconds` | Histogram | operation, storage |
