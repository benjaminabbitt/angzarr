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

Not all events come from command processing. External facts (payment confirmations, delivery updates) use a **fact sequence** instead of an integer:

```protobuf
oneof sequence_type {
  uint64 sequence = 1;        // Normal: position in stream
  FactSequence fact = 2;      // External fact marker
}
```

Fact sequences contain an idempotency key (`external_id`) for deduplication rather than an expected sequence for concurrency control. See [Commands vs Facts](/concepts/commands-vs-facts) for details.
