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

- **stream.rs** - `StreamService`: Streams events to clients via gRPC. Maintains subscriptions by correlation ID and forwards matching events to connected clients through the gateway.

## Used By

- `angzarr-log` binary (log projector)
- `angzarr-stream` binary (stream projector)

## See Also

- `handlers/core/` - AMQP event handlers that forward to business coordinators
- `handlers/gateway/` - Command gateway for routing commands and queries
