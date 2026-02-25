---
id: projection
title: Projection
hoverText: Query-optimized read model built by a projector from event streams.
---

# Projection

A query-optimized read model built by a [projector](/glossary/projector) from event streams. Projections are the "read side" of [CQRS](/glossary/cqrs).

## Characteristics

- **Derived:** Built from events, not the source of truth
- **Disposable:** Can be rebuilt by replaying events
- **Optimized:** Structured for specific query patterns
- **Eventually consistent:** May lag behind writes

## Examples

| Events | Projection | Query Pattern |
|--------|------------|---------------|
| `OrderCreated`, `ItemAdded` | Order summary table | Get order by ID |
| `PlayerRegistered`, `FundsDeposited` | Player balance view | Get current balance |
| `HandStarted`, `BetPlaced` | Active hands dashboard | List active hands |

## Projection vs Event Store

| Aspect | Projection | Event Store |
|--------|------------|-------------|
| Source of truth | No | Yes |
| Structure | Query-optimized | Append-only log |
| Rebuildable | Yes (replay events) | No (is the source) |
| Consistency | Eventually | Immediately |

## In Angzarr

Projectors output to:
- Databases (PostgreSQL, Redis)
- APIs (REST endpoints)
- Files (reports, exports)
- External systems (webhooks, queues)
