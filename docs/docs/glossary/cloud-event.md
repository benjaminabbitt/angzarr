---
id: cloud-event
title: CloudEvent
hoverText: Standardized event envelope for external consumption following the CloudEvents specification.
---

# CloudEvent

A standardized event envelope following the [CloudEvents specification](https://cloudevents.io/). Used for external event consumption and integration.

## Structure

```protobuf
message CloudEvent {
  string type = 1;                    // Event type
  google.protobuf.Any payload = 2;    // Event data
  map<string, string> extensions = 3; // Custom attributes

  // Optional overrides
  optional string id = 4;
  optional string source = 5;
  optional string subject = 6;
}
```

## Default Values

When not explicitly set:
- **id:** `{domain}:{root_id}:{sequence}`
- **source:** `angzarr/{domain}`

## CloudEventsResponse

Return type from projectors that emit external events:

```protobuf
message CloudEventsResponse {
  repeated CloudEvent events = 1;
}
```

Projectors can emit:
- **0 events:** Skip/filter
- **1 event:** Typical case
- **N events:** Fan-out pattern

## Use Cases

- External system integration
- Webhook delivery
- Message queue publishing
- Cross-system event sharing

## Relationship to Internal Events

| Aspect | Internal Event | CloudEvent |
|--------|----------------|------------|
| Format | Protobuf Any | CloudEvents spec |
| Audience | Angzarr components | External systems |
| Produced by | Aggregates | Projectors |
