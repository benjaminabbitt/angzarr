# handlers/projectors

Projector service implementations.

## Purpose

These are actual projector implementations that process events and produce output. Unlike the core handlers (which forward events to external services), these projectors are self-contained services that implement the ProjectorCoordinator gRPC interface.

## Architecture

```
[angzarr-projector sidecar] --> [projector implementations]
                                        |
                                        v
                                (specific output)
```

## Modules

- **log.rs** - `LogService`: Pretty-prints events to stdout. Useful for debugging and monitoring event flow. Optionally decodes protobuf messages to JSON if a descriptor file is provided.

- **outbound/** - `OutboundService`: Unified outbound projector combining gRPC streaming (for gateways) with CloudEvents publishing (HTTP/Kafka). Supports JSON and protobuf encoding.

- **stream.rs** - `StreamService`: Legacy gRPC streaming service. Use `OutboundService` for new implementations.

- **cloudevents/** - CloudEvents sink implementations (HTTP, Kafka) and protobuf encoding.

## Used By

- `angzarr-log` binary (log projector)
- `angzarr-stream` binary (outbound projector - gRPC + CloudEvents)

## See Also

- `handlers/core/` - AMQP event handlers that forward to business coordinators
- `handlers/gateway/` - Command gateway for routing commands and queries
