---
sidebar_position: 3
---

# Testcontainers

Angzarr uses [testcontainers](https://rust.testcontainers.org/) to automatically provision real databases during test execution.

---

## Why Testcontainers

- **Zero setup** — Tests start required containers automatically
- **Isolation** — Each test gets a fresh container
- **Realistic** — Tests run against real databases, not mocks
- **Parallel-safe** — Dynamic port binding prevents conflicts
- **RAII cleanup** — Containers stop when dropped

---

## Basic Pattern

```rust
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};

async fn start_postgres() -> (ContainerAsync<GenericImage>, String) {
    let image = GenericImage::new("postgres", "16")
        .with_exposed_port(5432.tcp())
        .with_wait_for(WaitFor::message_on_stdout(
            "database system is ready to accept connections",
        ));

    let container = image
        .with_env_var("POSTGRES_USER", "testuser")
        .with_env_var("POSTGRES_PASSWORD", "testpass")
        .with_env_var("POSTGRES_DB", "testdb")
        .with_startup_timeout(Duration::from_secs(60))
        .start()
        .await
        .expect("Failed to start container");

    // Brief delay ensures full readiness
    tokio::time::sleep(Duration::from_secs(1)).await;

    let host_port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get mapped port");

    let connection_string = format!(
        "postgres://testuser:testpass@localhost:{}/testdb",
        host_port
    );

    (container, connection_string)
}
```

---

## Key Points

### Keep Container Handle in Scope

The container stops when dropped. Use `let (_container, url) = ...`:

```rust
#[tokio::test]
async fn test_database_operations() {
    let (_container, url) = start_postgres().await;

    // Container alive for test duration
    let pool = connect(&url).await;
    // ... run tests ...

    // Container stops when _container is dropped
}
```

### Use Dynamic Ports

Never hardcode ports. Use `get_host_port_ipv4()`:

```rust
let host_port = container.get_host_port_ipv4(5432).await.unwrap();
let url = format!("postgres://user:pass@localhost:{}/db", host_port);
```

### Wait for Readiness

Use `WaitFor` conditions or brief sleeps:

```rust
let image = GenericImage::new("postgres", "16")
    .with_wait_for(WaitFor::message_on_stdout(
        "database system is ready to accept connections",
    ));
```

---

## Backend-Specific Setup

### PostgreSQL

```rust
async fn start_postgres() -> (ContainerAsync<GenericImage>, String) {
    let image = GenericImage::new("postgres", "16")
        .with_exposed_port(5432.tcp())
        .with_wait_for(WaitFor::message_on_stdout("database system is ready"));

    let container = image
        .with_env_var("POSTGRES_USER", "test")
        .with_env_var("POSTGRES_PASSWORD", "test")
        .with_env_var("POSTGRES_DB", "test")
        .start().await.unwrap();

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    (container, format!("postgres://test:test@localhost:{}/test", port))
}
```

### Redis

```rust
async fn start_redis() -> (ContainerAsync<GenericImage>, String) {
    let image = GenericImage::new("redis", "7")
        .with_exposed_port(6379.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"));

    let container = image.start().await.unwrap();
    let port = container.get_host_port_ipv4(6379).await.unwrap();
    (container, format!("redis://localhost:{}", port))
}
```

### RabbitMQ

```rust
async fn start_rabbitmq() -> (ContainerAsync<GenericImage>, String) {
    let image = GenericImage::new("rabbitmq", "3-management")
        .with_exposed_port(5672.tcp())
        .with_wait_for(WaitFor::message_on_stdout("started"));

    let container = image
        .with_env_var("RABBITMQ_DEFAULT_USER", "test")
        .with_env_var("RABBITMQ_DEFAULT_PASS", "test")
        .start().await.unwrap();

    let port = container.get_host_port_ipv4(5672).await.unwrap();
    (container, format!("amqp://test:test@localhost:{}", port))
}
```

---

## Running Tests

### Full-Support Backends (SQLite, PostgreSQL)

Backends supporting all storage traits use Gherkin interface tests:

```bash
# SQLite (in-memory, no containers)
cargo test --test interfaces --features "sqlite test-utils"

# PostgreSQL (requires podman/docker)
STORAGE_BACKEND=postgres cargo test --test interfaces --features "postgres test-utils"
```

### Partial-Support Backends

Backends with partial storage support use direct macro tests:

```bash
# Redis (SnapshotStore only)
cargo test --test storage_redis --features redis

# immudb (EventStore only)
cargo test --test storage_immudb --features immudb

# NATS (EventStore only)
cargo test --test storage_nats --features nats
```

### Bus Tests

```bash
# AMQP tests
cargo test --test bus_amqp --features amqp
```

**Note:** Requires the podman socket:

```bash
systemctl --user start podman.socket
```

---

## Test Architecture

### Gherkin Interface Tests (Preferred)

For backends supporting all three storage traits (EventStore, SnapshotStore, PositionStore),
use the Gherkin interface tests. These provide human-readable contract specifications:

```gherkin
# tests/interfaces/features/event_store.feature
Scenario: First event in an aggregate's history starts at sequence 0
  Given an aggregate "player" with no events
  When I add 1 event to the aggregate
  Then the aggregate should have 1 event
  And the first event should have sequence 0
```

### Shared Test Macros (Partial Support)

For backends with partial storage support, shared test macros ensure consistent behavior:

```rust
// tests/storage/event_store_tests.rs
macro_rules! run_event_store_tests {
    ($store:expr) => {{
        test_add_and_get($store).await;
        test_add_multiple_events($store).await;
        test_get_from($store).await;
        // ... more tests
    }};
}
```

Each partial-support backend invokes the macros:

```rust
// tests/storage_redis.rs (SnapshotStore only)
#[tokio::test]
async fn test_redis_snapshot_store() {
    let (_container, url) = start_redis().await;
    let store = RedisSnapshotStore::new(&url, None).await.unwrap();

    run_snapshot_store_tests!(&store);
}
```

---

## Next Steps

- **[Testing](/operations/testing)** — Full testing strategy
- **[Databases](/tooling/databases/postgres)** — Database configuration
