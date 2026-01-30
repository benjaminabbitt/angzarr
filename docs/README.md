# Documentation

## Architects & Decision Makers

- [PITCH.md](PITCH.md) — Full architectural pitch (standalone document for sharing)
- [COMPARISON.md](COMPARISON.md) — Detailed comparison vs Axon, AWS, GCP, Kafka

## Developers

### Getting Started

- [Getting Started](getting-started.md) — Prerequisites, installation, first domain, CLI reference
- [TOOLING.md](../TOOLING.md) — Development tools setup (just, bacon, mold, sccache, Kind)

### Implementation Guides

- [Command Handlers (Aggregates)](components/aggregate/aggregate.md) — Processing commands and emitting events
- [Projectors](components/projector/projectors.md) — Building read models and performing side effects
- [Sagas (Process Coordinators)](components/saga/sagas.md) — Orchestrating workflows across aggregates

### Reference

- [Patterns](patterns.md) — Outbox, upcasting, process manager, temporal query
- [Port Conventions](port-conventions.md) — Standardized five-port-per-pod scheme
- [Observability](PITCH.md#observability) — OpenTelemetry, tracing, metrics (baked into the sidecar)

## Sponsors & Partners

- [PARTNERS.md](PARTNERS.md) — Partnership opportunities, engagement models, roadmap

## Concepts

- [CQRS and Event Sourcing](cqrs-event-sourcing.md) — Background for those new to the pattern

## Quick Reference

| Component | Purpose | Receives | Produces |
|-----------|---------|----------|----------|
| Domain | Business capability (e.g., "flights", "customers") | — | — |
| Aggregate | One codebase per domain, scales horizontally | Commands + Event History | Events |
| Aggregate Root | Identity of instance within domain (hash of business keys) | — | — |
| Projector | Perform side effects (DB writes, streaming, caching) | Events | Projections / Side Effects |
| Saga (Process Coordinator) | Coordinate workflows across domains | Events | Commands to other domains |

## gRPC Contracts

- [proto/angzarr/](../proto/angzarr/) — Framework service definitions (aggregate, gateway, projector, saga, query)
- [proto/examples/](../proto/examples/) — Example domain types (cart, customer, order, product, inventory, fulfillment)

## Example Implementations

All patterns are implemented in Rust, Go, and Python with identical behavior:

```
examples/
├── features/           # Shared BDD specifications (Gherkin)
├── rust/               # Rust implementations
├── go/                 # Go implementations
└── python/             # Python implementations
```
