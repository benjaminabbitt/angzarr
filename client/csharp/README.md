---
title: C# SDK
sidebar_label: C#
---

# Angzarr.Client

.NET client library for Angzarr event-sourcing services.

:::tip Unified Documentation
For cross-language API reference with side-by-side comparisons, see the [SDK Documentation](/sdks).
:::

## Installation

```bash
dotnet add package Angzarr.Client
```

Or add to your `.csproj`:

```xml
<PackageReference Include="Angzarr.Client" Version="0.1.0" />
```

## Usage

### Sending Commands

```csharp
using Angzarr.Client;

// Connect to aggregate coordinator
using var client = DomainClient.Connect("http://localhost:1310");

// Send a command to create a new aggregate
var response = client.CommandNew("orders")
    .WithCorrelationId("order-123")
    .WithCommand("type.googleapis.com/examples.CreateOrder", createOrderCmd)
    .Execute();

// Get the new aggregate root ID from response
var rootId = Helpers.RootGuid(response.Events);
Console.WriteLine($"Created order: {rootId}");
```

### Querying Events

```csharp
using Angzarr.Client;

// Connect to query service
using var client = DomainClient.Connect("http://localhost:1310");

// Query events for an aggregate
var rootId = Guid.Parse("...");
var events = client.QueryEvents("orders", rootId).GetEventBook();

// Iterate over events
foreach (var page in events.Pages)
{
    Console.WriteLine($"Event {Helpers.SequenceNum(page)}: {Helpers.TypeNameFromUrl(page.Event.TypeUrl)}");
}
```

### Using Environment Variables

```csharp
// Connect using environment variable with fallback
using var client = DomainClient.FromEnv("ANGZARR_ENDPOINT", "http://localhost:1310");
```

### Temporal Queries

```csharp
// Query state as of a specific sequence
var events = client.QueryEvents("orders", rootId)
    .AsOfSequence(10)
    .GetEventBook();

// Query state as of a specific time
var events = client.QueryEvents("orders", rootId)
    .AsOfTime("2024-01-15T10:30:00Z")
    .GetEventBook();

// Query a range of sequences
var events = client.QueryEvents("orders", rootId)
    .RangeTo(5, 15)
    .GetEventBook();
```

### Error Handling

```csharp
using Angzarr.Client;

try
{
    var response = client.Command("orders", rootId)
        .WithSequence(5)
        .WithCommand(typeUrl, cmd)
        .Execute();
}
catch (ClientError e)
{
    if (e.IsNotFound())
    {
        // Aggregate doesn't exist
    }
    else if (e.IsPreconditionFailed())
    {
        // Sequence mismatch (optimistic locking failure)
    }
    else if (e.IsInvalidArgument())
    {
        // Invalid command arguments
    }
    else if (e.IsConnectionError())
    {
        // Network/transport error
    }
}
```

### Speculative Execution

Test commands without persisting to the event store:

```csharp
using Angzarr.Client;

using var client = SpeculativeClient.Connect("http://localhost:1310");

// Build speculative request with temporal state
var request = new SpeculateAggregateRequest
{
    Command = commandBook,
    Events = { priorEvents }
};

// Execute without persistence
var response = client.Aggregate(request);

// Inspect projected events
foreach (var page in response.Events.Pages)
{
    Console.WriteLine($"Would produce: {page.Event.TypeUrl}");
}
```

### Compensation Context

Extract rejection details when saga/PM commands fail:

```csharp
using Angzarr.Client;

// In your saga or process manager rejection handler
var context = CompensationContext.FromNotification(notification);

Console.WriteLine($"Issuer: {context.IssuerName} ({context.IssuerType})");
Console.WriteLine($"Rejection reason: {context.RejectionReason}");
Console.WriteLine($"Failed command type: {context.RejectedCommandType()}");
Console.WriteLine($"Source event sequence: {context.SourceEventSequence}");
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
| `CommandRejectedError` | Business logic rejection | `IsPreconditionFailed()` |
| `GrpcError` | gRPC transport failure | Based on status code |
| `ConnectionError` | Connection failure | `IsConnectionError()` |
| `TransportError` | Transport-level failure | `IsConnectionError()` |
| `InvalidArgumentError` | Invalid input | `IsInvalidArgument()` |

## Helper Functions

```csharp
using Angzarr.Client;

// GUID conversion
var guid = Helpers.ProtoToGuid(protoUuid);
var protoUuid = Helpers.GuidToProto(guid);

// Type URL helpers
var typeUrl = Helpers.TypeUrl("examples.CreateOrder");  // "type.googleapis.com/examples.CreateOrder"
var typeName = Helpers.TypeNameFromUrl(typeUrl);        // "CreateOrder"

// Cover accessors
var domain = Helpers.Domain(eventBook);
var correlationId = Helpers.CorrelationId(eventBook);
var rootGuid = Helpers.RootGuid(eventBook);

// Sequence helpers
var nextSeq = Helpers.NextSequence(eventBook);
```

## License

AGPL-3.0-only
