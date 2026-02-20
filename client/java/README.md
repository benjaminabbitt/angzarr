---
title: Java SDK
sidebar_label: Java
---

# angzarr-client-java

Java client library for Angzarr event-sourcing services.

:::tip Unified Documentation
For cross-language API reference with side-by-side comparisons, see the [SDK Documentation](/sdks).
:::

## Installation

Add to your `pom.xml`:

```xml
<dependency>
    <groupId>dev.angzarr</groupId>
    <artifactId>angzarr-client</artifactId>
    <version>0.1.0</version>
</dependency>
```

Or with Gradle:

```groovy
implementation 'dev.angzarr:angzarr-client:0.1.0'
```

## Usage

### Sending Commands

```java
import dev.angzarr.client.DomainClient;
import dev.angzarr.CommandResponse;
import java.util.UUID;

public class Example {
    public static void main(String[] args) {
        // Connect to aggregate coordinator
        try (DomainClient client = DomainClient.connect("localhost:1310")) {

            // Send a command to create a new aggregate
            CommandResponse response = client.commandNew("orders")
                .withCorrelationId("order-123")
                .withCommand("type.googleapis.com/examples.CreateOrder", createOrderCmd)
                .execute();

            // Get the new aggregate root ID from response
            UUID rootId = Helpers.rootUUID(response.getEvents());
            System.out.println("Created order: " + rootId);
        }
    }
}
```

### Querying Events

```java
import dev.angzarr.client.DomainClient;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import java.util.UUID;

// Connect to query service
try (DomainClient client = DomainClient.connect("localhost:1310")) {

    // Query events for an aggregate
    UUID rootId = UUID.fromString("...");
    EventBook events = client.query("orders", rootId)
        .getEventBook();

    // Iterate over events
    for (EventPage page : events.getPagesList()) {
        System.out.printf("Event %d: %s%n",
            Helpers.sequenceNum(page),
            Helpers.typeNameFromUrl(page.getEvent().getTypeUrl()));
    }
}
```

### Using Environment Variables

```java
// Connect using environment variable with fallback
DomainClient client = DomainClient.fromEnv("ANGZARR_ENDPOINT", "localhost:1310");
```

### Temporal Queries

```java
// Query state as of a specific sequence
EventBook events = client.query("orders", rootId)
    .asOfSequence(10)
    .getEventBook();

// Query state as of a specific time
EventBook events = client.query("orders", rootId)
    .asOfTime("2024-01-15T10:30:00Z")
    .getEventBook();

// Query a range of sequences
EventBook events = client.query("orders", rootId)
    .rangeTo(5, 15)
    .getEventBook();
```

### Error Handling

```java
import dev.angzarr.client.Errors.*;

try {
    CommandResponse response = client.command("orders", rootId)
        .withSequence(5)
        .withCommand(typeUrl, cmd)
        .execute();
} catch (ClientError e) {
    if (e.isNotFound()) {
        // Aggregate doesn't exist
    } else if (e.isPreconditionFailed()) {
        // Sequence mismatch (optimistic locking failure)
    } else if (e.isInvalidArgument()) {
        // Invalid command arguments
    } else if (e.isConnectionError()) {
        // Network/transport error
    }
}
```

### Speculative Execution

Test commands without persisting to the event store:

```java
import dev.angzarr.client.SpeculativeClient;
import dev.angzarr.SpeculateAggregateRequest;

try (SpeculativeClient client = SpeculativeClient.connect("localhost:1310")) {

    // Build speculative request with temporal state
    SpeculateAggregateRequest request = SpeculateAggregateRequest.newBuilder()
        .setCommand(commandBook)
        .addAllEvents(priorEvents)
        .build();

    // Execute without persistence
    CommandResponse response = client.aggregate(request);

    // Inspect projected events
    for (EventPage page : response.getEvents().getPagesList()) {
        System.out.println("Would produce: " + page.getEvent().getTypeUrl());
    }
}
```

## Client Types

| Client | Purpose |
|--------|---------|
| `QueryClient` | Query events from aggregates |
| `AggregateClient` | Send commands to aggregates |
| `SpeculativeClient` | Dry-run commands, test projectors/sagas |
| `DomainClient` | Combined query + aggregate for a domain |

## Error Types

| Error | Description | Introspection |
|-------|-------------|---------------|
| `ClientError` | Base class for all errors | All methods return `false` |
| `CommandRejectedError` | Business logic rejection | `isPreconditionFailed()` |
| `GrpcError` | gRPC transport failure | Based on status code |
| `ConnectionError` | Connection failure | `isConnectionError()` |
| `TransportError` | Transport-level failure | `isConnectionError()` |
| `InvalidArgumentError` | Invalid input | `isInvalidArgument()` |

## Helper Functions

```java
import dev.angzarr.client.Helpers;

// UUID conversion
UUID uuid = Helpers.protoToUUID(protoUUID);
Types.UUID protoUUID = Helpers.uuidToProto(uuid);

// Type URL helpers
String typeUrl = Helpers.typeUrl("examples.CreateOrder");  // "type.googleapis.com/examples.CreateOrder"
String typeName = Helpers.typeNameFromUrl(typeUrl);        // "CreateOrder"

// Cover accessors
String domain = Helpers.domain(eventBook);
String correlationId = Helpers.correlationId(eventBook);
UUID rootUUID = Helpers.rootUUID(eventBook);

// Sequence helpers
long nextSeq = Helpers.nextSequence(eventBook);
```

## License

AGPL-3.0-only
