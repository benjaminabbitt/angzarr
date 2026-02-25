---
id: event-page
title: EventPage
hoverText: Individual event with sequence number, timestamp, and payload within an EventBook.
---

# EventPage

An individual event within an [EventBook](/glossary/event-book). Each page represents one state change.

## Structure

```protobuf
message EventPage {
  uint64 sequence = 1;      // Position in aggregate timeline
  google.protobuf.Timestamp timestamp = 2;
  google.protobuf.Any event = 3;  // Or PayloadReference
}
```

## Fields

| Field | Purpose |
|-------|---------|
| `sequence` | Ordering within aggregate (0, 1, 2, ...) |
| `timestamp` | When event was persisted |
| `event` | Serialized event payload |

## Large Payloads

For events exceeding message bus limits, use `PayloadReference`:

```protobuf
message PayloadReference {
  PayloadStorageType storage_type = 1;  // GCS, S3, filesystem
  string location = 2;                   // Storage path
}
```

This implements the **Claim Check Pattern** - store large payloads externally and reference them.

## Immutability

Once persisted, EventPages cannot be modified. This guarantees:
- Audit trail integrity
- Consistent replay
- Trust in historical data
