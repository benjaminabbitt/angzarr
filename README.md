# evented-rs

A CQRS/Event Sourcing infrastructure framework in Rust.

## Overview

evented-rs provides the infrastructure layer for event-sourced systems:
- Event persistence with sequence validation
- Snapshot optimization for aggregate replay
- gRPC event distribution
- Projector and saga coordination
- Multi-language support (Python, Go, Rust)

## Getting Started

### Prerequisites

- Rust 1.70+
- Podman or Docker (for Kubernetes development)
- [just](https://github.com/casey/just) command runner
- grpcurl (optional, for debugging)

### 1. Clone and Build

```bash
git clone https://github.com/yourorg/evented-rs
cd evented-rs

# Build the framework
just build

# Run tests to verify setup
just test
```

### 2. Run In-Memory (Development)

The fastest path to experimentation—no external dependencies:

```bash
# Start the server with default SQLite storage
just run

# Or use in-memory storage for tests
cargo test --test acceptance
```

### 3. Run with Kubernetes (Production-Like)

For realistic multi-service deployments:

```bash
# Deploy evented + dependencies (RabbitMQ, Redis) to Kind cluster
just deploy

# Watch logs
just k8s-logs
```

### 4. Create Your First Domain

Create a command handler in your preferred language. Example in Rust:

```rust
// examples/rust/customer/src/lib.rs
pub struct CustomerLogic;

impl CustomerLogic {
    pub async fn handle(&self, domain: &str, cmd: ContextualCommand) -> Result<EventBook> {
        // Validate command against current state (cmd.prior_state)
        // Return events to persist
        Ok(EventBook {
            events: vec![Event { /* CustomerCreated */ }],
            ..Default::default()
        })
    }
}
```

Register it in `config.yaml`:

```yaml
business_logic:
  - domain: customer
    address: localhost:50052
```

## Architecture

Business logic lives in external services called via gRPC. evented-rs handles:
- **EventStore**: Persist and query events (SQLite, Redis)
- **SnapshotStore**: Optimize replay with snapshots
- **EventBus**: Distribute events to projectors/sagas
- **CommandHandler**: Orchestrate command processing
- **ProjectorCoordinator**: Route events to read model builders
- **SagaCoordinator**: Route events to cross-aggregate workflows

## Development Experience

CQRS/Event Sourcing systems are notoriously complex—event stores, snapshot optimization, distributed event routing, projection rebuilds, saga coordination, and concurrency control create significant infrastructure burden. evented-rs absorbs this complexity so you write only business logic.

### What You Write

**Command Handlers** — Pure functions that validate commands against current state and emit events:

```python
def handle_create_customer(command, state):
    if state.name:
        raise CommandRejectedError("Customer already exists")
    if not command.name:
        raise CommandRejectedError("Customer name is required")

    return CustomerCreated(name=command.name, email=command.email)
```

**Projectors** — React to events and build read models:

```go
func (p *ReceiptProjector) Project(event EventBook) {
    if event.TypeUrl.Contains("TransactionCompleted") {
        p.receipts.Store(event.AggregateId, buildReceipt(event))
    }
}
```

**Sagas** — Cross-aggregate workflows that react to events and emit commands:

```rust
fn on_transaction_completed(&self, event: TransactionCompleted) -> Vec<Command> {
    vec![AddLoyaltyPoints {
        customer_id: event.customer_id,
        points: calculate_points(event.amount),
    }]
}
```

### What evented-rs Handles

- Event persistence with sequence validation and optimistic concurrency
- Snapshot creation and optimized aggregate replay
- Event distribution to projectors and sagas via gRPC/AMQP
- Command routing to the correct business logic service
- Projection coordination and event delivery guarantees

### Multi-Language Support

Write business logic in your preferred language. All examples ship in Rust, Go, and Python with identical semantics:

```
examples/
├── rust/customer/     # Rust command handler
├── go/customer/       # Go command handler
├── python/customer/   # Python command handler
```

### Behavior-Driven Development

Acceptance tests use Gherkin syntax for executable specifications:

```gherkin
Feature: Command Handling

  Scenario: Handle command with existing history
    Given prior events for aggregate "order-456" in domain "orders":
      | sequence | event_type   |
      | 0        | OrderCreated |
      | 1        | ItemAdded    |
    When I send an "AddItem" command for aggregate "order-456"
    Then the business logic receives the command with 2 prior events
    And 3 events total exist for aggregate "order-456"
```

Tests run via `cargo test` using [cucumber-rs](https://github.com/cucumber-rs/cucumber). Feature files in `tests/acceptance/features/` document system behavior and serve as living documentation.

## CLI Reference

All commands use [just](https://github.com/casey/just). Run `just` with no arguments to see available commands.

### Development Workflow

| Command | Description |
|---------|-------------|
| `just build` | Build the framework |
| `just build-release` | Build optimized release binary |
| `just test` | Run all unit tests |
| `just acceptance-test` | Run Gherkin acceptance tests (no containers) |
| `just run` | Start the evented server |
| `just check` | Fast compile check without building |
| `just fmt` | Format code with rustfmt |
| `just lint` | Run clippy lints |

### Proto Generation

| Command | Description |
|---------|-------------|
| `just proto-generate` | Generate all language bindings (Rust, Go, Python) |
| `just proto-rust` | Generate Rust bindings only |
| `just proto-go` | Generate Go bindings only |
| `just proto-python` | Generate Python bindings only |
| `just proto-clean` | Remove generated files |

### Examples

| Command | Description |
|---------|-------------|
| `just examples-build` | Build all example services |
| `just examples-test` | Test all examples |
| `just examples-rust` | Build Rust examples only |
| `just examples-go` | Build Go examples only |
| `just examples-python` | Build Python examples only |

### Kubernetes/Kind

| Command | Description |
|---------|-------------|
| `just kind-create` | Create local Kind cluster |
| `just kind-delete` | Delete Kind cluster |
| `just deploy` | Build images, load into Kind, and deploy |
| `just redeploy` | Rebuild and redeploy (faster iteration) |
| `just undeploy` | Remove deployment |
| `just k8s-logs` | Stream evented pod logs |
| `just k8s-port-forward` | Forward gRPC ports to localhost |

### Testing

| Command | Description |
|---------|-------------|
| `just test` | Run unit tests |
| `just acceptance-test` | Run in-memory acceptance tests (no containers) |
| `just integration-test` | Deploy to Kind and run integration tests |
| `just integration-test-only` | Run integration tests against already-running cluster |

#### Test Types

**Acceptance Tests** (`just acceptance-test`)
- Run entirely in-memory using SQLite and stub services
- Fast, no external dependencies
- Test core framework logic: command handling, event persistence, snapshots
- Feature files: `tests/acceptance/features/*.feature`

**Integration Tests** (`just integration-test-only`)
- Run against deployed Kubernetes pods via gRPC
- Test full end-to-end flow: commands → business logic → events → projectors
- Requires `just deploy` first (or use `just integration-test` to deploy and test)
- Feature files: `tests/integration/features/*.feature`
- Projector logs show actual events: `kubectl logs -n evented -l app=rs-projector-log-customer`

## Debugging and Observability

### Logging

evented-rs uses [tracing](https://docs.rs/tracing) for structured logging. Control verbosity with `RUST_LOG`:

```bash
# Default: info level
just run

# Debug level for evented, info for dependencies
RUST_LOG=evented=debug just run

# Trace all SQL queries
RUST_LOG=sqlx=debug,evented=info just run

# Full trace (verbose)
RUST_LOG=trace just run
```

Log output is structured JSON in production, human-readable in development:

```
2024-01-15T10:30:45.123Z  INFO evented: Starting evented server
2024-01-15T10:30:45.456Z  INFO evented: Storage: sqlite at ./data/events.db
2024-01-15T10:30:45.789Z  INFO evented: Command handler listening on 0.0.0.0:1313
```

### Inspecting gRPC Services

Use [grpcurl](https://github.com/fullstorydev/grpcurl) to interact with services:

```bash
# List available services
grpcurl -plaintext localhost:1313 list

# Describe a service
grpcurl -plaintext localhost:1313 describe evented.BusinessCoordinator

# Send a command
grpcurl -plaintext -d '{
  "command": {
    "domain": "customer",
    "aggregate_id": "cust-001",
    "type_url": "CreateCustomer",
    "payload": "..."
  }
}' localhost:1313 evented.BusinessCoordinator/Handle
```

### Event Store Inspection

Query events directly via the EventQuery service:

```bash
# Get all events for an aggregate
grpcurl -plaintext -d '{
  "domain": "customer",
  "aggregate_id": "cust-001"
}' localhost:1314 evented.EventQuery/GetEvents

# Get events since a specific sequence
grpcurl -plaintext -d '{
  "domain": "customer",
  "aggregate_id": "cust-001",
  "from_sequence": 5
}' localhost:1314 evented.EventQuery/GetEvents
```

### SQLite Direct Access

For development debugging, query the SQLite database directly:

```bash
# Connect to the event store
sqlite3 data/events.db

# View recent events
SELECT domain, aggregate_id, sequence, type_url, created_at
FROM events
ORDER BY created_at DESC
LIMIT 10;

# Count events per aggregate
SELECT domain, aggregate_id, COUNT(*) as event_count
FROM events
GROUP BY domain, aggregate_id;
```

### Kubernetes Debugging

```bash
# Stream logs from evented pods
just k8s-logs

# Get pod status
kubectl get pods -n evented

# Describe pod for events/errors
kubectl describe pod -n evented -l app.kubernetes.io/name=evented

# Port forward for local debugging
just k8s-port-forward
# Then use grpcurl against localhost:1313
```

### Common Issues

| Symptom | Cause | Fix |
|---------|-------|-----|
| "Connection refused" on startup | Business logic service not running | Start your domain service first |
| "Failed to connect to projector" | Projector not reachable | Check projector address in config.yaml |
| Events not persisting | Database path not writable | Ensure `data/` directory exists with write permissions |
| AMQP connection failed | RabbitMQ not running | Start RabbitMQ or use DirectEventBus for local dev |

## Local Kubernetes Development

For local development with Kubernetes, evented-rs uses Kind (Kubernetes in Docker) with Podman.

### Prerequisites

All tooling is open-source and burdensome-license-free, so corporate users face no licensing risks:

- **Podman** - Container runtime (Docker-compatible, no Docker Desktop license)
- **Kind** - Local Kubernetes clusters using containers as nodes
- **kubectl** - Kubernetes CLI

### Setup

```bash
# Build images, create Kind cluster, load images, and deploy
just deploy

# For subsequent changes, use redeploy (faster)
just redeploy
```

### Just Commands

| Command | Description |
|---------|-------------|
| `just kind-create` | Create Kind cluster with port mappings for evented services |
| `just kind-delete` | Delete the Kind cluster |
| `just deploy` | Full deployment: build images, load into Kind, apply manifests |
| `just redeploy` | Rebuild and redeploy (faster iteration) |
| `just undeploy` | Remove deployment from cluster |

### Exposed Ports

The Kind cluster exposes these services to localhost:

| Port | Service |
|------|---------|
| 50051 | Evented command handler (gRPC) |
| 50052 | Evented event query (gRPC) |
| 5672 | RabbitMQ AMQP |
| 15672 | RabbitMQ Management UI |
| 6379 | Redis |

## Roadmap

Features to reach parity with mature frameworks like Axon:

### Aggregate Framework
- [ ] In-process aggregate hosting (command handlers co-located with framework)
- [ ] Aggregate lifecycle management (creation, loading, snapshotting)
- [ ] Aggregate annotations/macros for ergonomic handler definition

### Event Upcasting
- [ ] Transform old event versions to current schema during replay
- [ ] Upcaster chain registration and execution
- [ ] Schema versioning metadata on events

### Automatic Snapshotting
- [ ] Configurable snapshot triggers (every N events, time-based)
- [ ] Snapshot scheduling policies
- [ ] Background snapshot workers

### Deadline Management
- [ ] Schedule future triggers within aggregates/sagas
- [ ] Deadline cancellation
- [ ] Persistent deadline storage with leader election

### Distributed Command Routing
- [ ] Route commands to correct node in clustered deployment
- [ ] Consistent hashing for aggregate affinity
- [ ] Service discovery integration (Consul, Kubernetes)

### Projection Management
- [ ] Replay tokens for tracking processor position
- [ ] Reset/rebuild projections from event store
- [ ] Projection status monitoring and catch-up metrics

### Subscription Queries
- [ ] Live query updates pushed to clients
- [ ] Combine initial result with streaming updates
- [ ] Query registration and lifecycle management

### Production Event Store
- [ ] PostgreSQL backend
- [ ] Event store clustering/replication
- [ ] Compaction and archival policies

### Tooling
- [ ] Admin UI for event store inspection
- [ ] Projection status dashboard
- [ ] Event replay/debugging tools

## License

BSD-3-Clause

---

## Known Limitations

### Skaffold

We would prefer to use [Skaffold](https://skaffold.dev/) for local Kubernetes development, as it provides file watching and automatic rebuilds. However, Skaffold's Kind integration uses `kind load docker-image` which doesn't work with Podman—it expects images in the Docker daemon. Until Skaffold adds native Podman+Kind support (using `kind load image-archive`), we use a custom `just deploy` workflow that handles the `podman save` → `kind load image-archive` process.
