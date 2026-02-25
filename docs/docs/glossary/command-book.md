---
id: command-book
title: CommandBook
hoverText: Collection of commands with sequence expectations, merge strategy, and saga origin tracking.
---

# CommandBook

A collection of commands to be sent to aggregates. The CommandBook is the output of sagas and process managers.

## Structure

```protobuf
message CommandBook {
  Cover cover = 1;           // Target aggregate identity
  repeated CommandPage pages = 2;  // Commands to execute
  SagaCommandOrigin saga_origin = 3;  // For compensation tracking
}
```

## CommandPage

Each page contains:
- **Command payload:** The actual command (Any type)
- **Sequence:** Expected aggregate sequence (for concurrency)
- **Merge strategy:** How to handle conflicts

## Merge Strategies

| Strategy | Behavior |
|----------|----------|
| `MERGE_STRICT` | Reject on sequence mismatch |
| `MERGE_COMMUTATIVE` | Allow if mutations don't overlap |
| `MERGE_AGGREGATE_HANDLES` | Aggregate decides |
| `MERGE_MANUAL` | Route to DLQ for review |

## Saga Origin

Tracks the source of saga-issued commands for compensation:
- Saga name
- Triggering aggregate (domain, root)
- Triggering event sequence

If the command is rejected, a [RejectionNotification](/glossary/notification) is sent back to the saga.
