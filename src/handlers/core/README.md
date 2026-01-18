# handlers/core

Core angzarr sidecar event handlers.

## Purpose

These handlers receive events from the AMQP message bus and forward them to business logic coordinators via gRPC. They are the bridge between the event bus infrastructure and user-defined business logic.

## Architecture

```
[AMQP Event Bus] --> [core handlers] --> [Business Logic Coordinators]
                          |
                          v
                    (gRPC calls)
```

## Modules

- **projector.rs** - `ProjectorEventHandler`: Receives events from AMQP, forwards to ProjectorCoordinator services. Handles EventBook repair (fetching missing history) and publishes projector output back to AMQP for streaming.

- **saga.rs** - `SagaEventHandler`: Receives events from AMQP, forwards to SagaCoordinator services. Executes saga-produced commands via AggregateCoordinator and handles compensation when commands are rejected.

## Used By

- `angzarr-projector` sidecar binary
- `angzarr-saga` sidecar binary

## See Also

- `handlers/projectors/` - Actual projector implementations (log, stream)
- `services/aggregate.rs` - Aggregate coordinator (receives commands, not events)
