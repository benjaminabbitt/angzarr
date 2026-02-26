---
id: sequence
title: Sequence Number
hoverText: Ordered position of an event in aggregate timeline. Ensures ordering and idempotent application.
---

# Sequence Number

The ordered position of an event within an aggregate's timeline. Sequence numbers start at 0 and increment by 1.

## Purpose

1. **Ordering:** Guarantees events are applied in correct order
2. **Idempotency:** Prevents double-application of the same event
3. **Concurrency control:** Detects conflicting concurrent writes

## In Commands

Commands include an **expected sequence number**. If the aggregate's current sequence doesn't match:

| Merge Strategy | Behavior |
|----------------|----------|
| `MERGE_STRICT` | Reject command |
| `MERGE_COMMUTATIVE` | Allow if mutations don't overlap |
| `MERGE_AGGREGATE_HANDLES` | Let aggregate decide |
| `MERGE_MANUAL` | Route to DLQ |

## Example

```
Aggregate: order-123
Current sequence: 5

Command arrives expecting sequence 5
  → Valid, produces event with sequence 6

Command arrives expecting sequence 4
  → Conflict! Handle per merge strategy
```

## Sequence vs Timestamp

- **Sequence:** Logical ordering within aggregate (gaps not allowed)
- **Timestamp:** Physical time (may have gaps, clock skew)

Use sequence for state reconstruction; use timestamp for temporal queries.

## Fact Sequences

Not all events come from command processing. External facts (payment confirmations, delivery updates) use a **FactSequence** marker instead of an integer:

```protobuf
message EventPage {
  oneof sequence_type {
    uint32 sequence = 1;        // Normal: position in stream
    FactSequence fact = 5;      // External fact marker
  }
}

message FactSequence {
  string source = 1;            // Origin system (e.g., "stripe")
  string description = 2;       // Human-readable description
}
```

The idempotency key lives on `Cover.external_id`, not in FactSequence. This ensures:
- System-wide propagation through sagas, PMs, and projectors
- Consistent deduplication at every coordinator
- Full traceability from external source to all effects

### FactSequence Elimination

The `FactSequence` marker is **replaced with a real sequence number at persistence time**. When the coordinator receives a fact event:

1. Routes to aggregate (if `route_facts_to_aggregate = true`, the default)
2. Aggregate updates state and returns events
3. Coordinator assigns the next sequence number
4. Persists with `sequence` instead of `fact`
5. Publishes event with valid sequence

Downstream consumers (sagas, projectors, process managers) always receive events with proper sequence numbers—they never see `FactSequence` markers.

See [Commands vs Facts](/concepts/commands-vs-facts) for details.
