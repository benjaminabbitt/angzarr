---
id: snapshot
title: Snapshot
hoverText: Cached aggregate state at a point in time. Optimization to avoid replaying all events.
---

# Snapshot

Cached aggregate state at a point in time. Snapshots optimize [replay](/glossary/replay) by avoiding the need to apply all historical events.

## When to Snapshot

By default, Angzarr snapshots every 16 events. This balances:
- Storage cost (more snapshots = more storage)
- Replay performance (fewer events to apply)

## Snapshot Retention Policies

| Policy | Behavior | Use Case |
|--------|----------|----------|
| `RETENTION_DEFAULT` | Persist every 16 events | Normal operation |
| `RETENTION_PERSIST` | Keep indefinitely | Business milestones |
| `RETENTION_TRANSIENT` | Delete when newer written | Temporary checkpoints |

## Structure

A snapshot contains:
- Serialized aggregate state
- Sequence number at snapshot time
- Timestamp
- Retention policy

## Trade-offs

**Without snapshots:**
- Replay all events from beginning
- Slower aggregate load
- No storage overhead

**With snapshots:**
- Replay only events after snapshot
- Faster aggregate load
- Additional storage for snapshot data
