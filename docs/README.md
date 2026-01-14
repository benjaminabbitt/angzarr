# Angzarr Documentation

## Architecture Guides

- [CQRS and Event Sourcing Concepts](cqrs-event-sourcing.md) — Background for those new to the pattern
- [Command Handlers (Aggregates)](command-handlers.md) — Processing commands and emitting events
- [Projectors](projectors.md) — Building read models from event streams
- [Sagas](sagas.md) — Orchestrating workflows across aggregates

## Quick Reference

| Component | Purpose | Receives | Produces |
|-----------|---------|----------|----------|
| Command Handler | Enforce business rules | Commands + Event History | Events |
| Projector | Build read models | Events | Projections (optional) |
| Saga | Coordinate workflows | Events | Commands |

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
