---
id: target
title: Target
hoverText: A domain + list of event types used for subscriptions to filter which events a component receives.
---

# Target

A subscription filter specifying which events a component receives. Combines a domain name with optional event type filters.

## Structure

```protobuf
message Target {
  string domain = 1;           // Domain to subscribe to
  repeated string types = 2;   // Event types (empty = all)
}
```

## Configuration

**Environment variable**:
```bash
# Format: domain:Type1,Type2;domain2:Type3
ANGZARR_SUBSCRIPTIONS="order:OrderCreated,OrderCompleted;inventory"
```

## Filter Semantics

| Configuration | Events Received |
|---------------|-----------------|
| `order` | All events from order domain |
| `order:OrderCreated` | Only OrderCreated from order |
| `order:OrderCreated,OrderCancelled` | Both types |

## Use by Component Type

| Component | Typical Target |
|-----------|---------------|
| Saga | Single domain, specific events |
| Process Manager | Multiple domains |
| Projector | Single domain, all or specific events |
