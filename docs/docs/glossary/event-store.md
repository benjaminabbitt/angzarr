---
id: event-store
title: Event Store
hoverText: Append-only database of events. The source of truth for aggregate state.
---

# Event Store

An append-only database that stores events. The event store is the **source of truth** for aggregate state.

## Characteristics

- **Append-only:** Events are never modified or deleted (except for GDPR/legal requirements)
- **Ordered:** Events have sequence numbers within each aggregate
- **Immutable:** Once written, events cannot change
- **Complete:** Contains full history of all state changes

## In Angzarr

Angzarr supports multiple event store backends:
- PostgreSQL (recommended for production)
- SQLite (local development/testing)
- Redis (high-throughput scenarios)
- NATS JetStream (distributed)

## Operations

| Operation | Description |
|-----------|-------------|
| Append | Add new events with sequence validation |
| Read by root | Get all events for an aggregate |
| Read by correlation | Get events across aggregates in a workflow |
| Temporal query | Get events at a point in time |

## Relationship to Read Models

The event store is the write side. [Projectors](/glossary/projector) consume events and build read-optimized [projections](/glossary/projection) for queries.
