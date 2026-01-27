# Angzarr Documentation

## Architecture Guides

- [CQRS and Event Sourcing Concepts](cqrs-event-sourcing.md) — Background for those new to the pattern
- [Patterns](patterns.md) — Outbox, upcasting, process manager, temporal query
- [Command Handlers (Aggregates)](command-handlers.md) — Processing commands and emitting events
- [Projectors](projectors.md) — Building read models and performing side effects
- [Sagas (Process Coordinators)](sagas.md) — Orchestrating workflows across aggregates
- [Port Conventions](port-conventions.md) — Standardized five-port-per-pod scheme

## Quick Reference

| Component | Purpose | Receives | Produces |
|-----------|---------|----------|----------|
| Domain | Business capability (e.g., "flights", "customers") | — | — |
| Aggregate | One codebase per domain, scales horizontally | Commands + Event History | Events |
| Aggregate Root | Identity of instance within domain (hash of business keys) | — | — |
| Projector | Perform side effects (DB writes, streaming, caching) | Events | Projections / Side Effects |
| Saga (Process Coordinator) | Coordinate workflows across domains | Events | Commands to other domains |

## Example Implementations

All patterns are implemented in Rust, Go, and Python with identical behavior:

```
examples/
├── features/           # Shared BDD specifications (Gherkin)
├── rust/               # Rust implementations
├── go/                 # Go implementations
└── python/             # Python implementations
```

## gRPC Contracts

- [proto/angzarr/angzarr.proto](../proto/angzarr/angzarr.proto) — Framework services
- [proto/examples/domains.proto](../proto/examples/domains.proto) — Example domain types

## Additional Resources

- [COMPARISON.md](COMPARISON.md) — How Angzarr compares to other frameworks
- [PITCH.md](PITCH.md) — Value proposition and use cases
- [ONE_PAGER.md](ONE_PAGER.md) — Executive summary
