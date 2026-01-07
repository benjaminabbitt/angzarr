# evented-rs

A CQRS/Event Sourcing infrastructure framework in Rust.

## Overview

evented-rs provides the infrastructure layer for event-sourced systems:
- Event persistence with sequence validation
- Snapshot optimization for aggregate replay
- gRPC and in-process event distribution
- Projector and saga coordination
- Multi-language support (Python FFI, Go FFI)

## Quick Start

```bash
# Run the server
cargo run --bin evented_server

# Or embed as a library
```

```rust
let evented = Evented::builder(EventedConfig::in_memory())
    .with_business_logic(MyBusinessLogic::new())
    .with_projector(MyProjector::new())
    .build()
    .await?;
```

## Architecture

Business logic lives in external services called via gRPC. evented-rs handles:
- **EventStore**: Persist and query events (SQLite, Redis)
- **SnapshotStore**: Optimize replay with snapshots
- **EventBus**: Distribute events to projectors/sagas
- **CommandHandler**: Orchestrate command processing
- **ProjectorCoordinator**: Route events to read model builders
- **SagaCoordinator**: Route events to cross-aggregate workflows

## Deployment Modes

evented-rs supports two deployment modes for each major component:

| Component | Production | Development |
|-----------|------------|-------------|
| Event Bus | `DirectEventBus` (gRPC) | `InProcessEventBus` |
| Storage | SQLite file / Redis | In-memory SQLite |
| Business Logic | External gRPC services | In-process trait implementations |

**In-process mode** (in-memory storage, `InProcessEventBus`, embedded facade) is intended for:
- Initial development and prototyping
- Debugging without network complexity
- Unit and integration testing
- Single-process deployments

This allows stepping through the entire command-event-projection flow in a debugger without gRPC boundaries, then switching to distributed components for production.

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

MIT
