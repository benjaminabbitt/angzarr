# angzarr-client-go

Go client library for Angzarr event-sourcing services.

## Installation

```bash
go get github.com/benjaminabbitt/angzarr/client/go
```

## Usage

### Sending Commands

```go
package main

import (
    "context"
    "log"

    "github.com/google/uuid"
    angzarr "github.com/benjaminabbitt/angzarr/client/go"
    pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
)

func main() {
    // Connect to aggregate coordinator
    client, err := angzarr.NewAggregateClient("localhost:1310")
    if err != nil {
        log.Fatal(err)
    }
    defer client.Close()

    // Send a command to create a new aggregate
    resp, err := client.CommandNew("orders").
        WithCorrelationID("order-123").
        WithCommand("type.googleapis.com/examples.CreateOrder", &CreateOrderCommand{
            CustomerId: "customer-456",
        }).
        Execute(context.Background())
    if err != nil {
        log.Fatal(err)
    }

    // Get the new aggregate root ID
    rootID := angzarr.RootUUID(resp.Events)
    log.Printf("Created order: %s", rootID)
}
```

### Querying Events

```go
// Connect to query service
queryClient, err := angzarr.NewQueryClient("localhost:1310")
if err != nil {
    log.Fatal(err)
}
defer queryClient.Close()

// Query events for an aggregate
rootID := uuid.MustParse("...")
events, err := queryClient.Query("orders", rootID).
    GetEventBook(context.Background())
if err != nil {
    log.Fatal(err)
}

// Iterate over events
for _, page := range events.Pages {
    log.Printf("Event %d: %s", angzarr.SequenceNum(page), angzarr.TypeNameFromURL(page.Event.TypeUrl))
}
```

### Using Environment Variables

```go
// Connect using environment variable with fallback
client, err := angzarr.AggregateClientFromEnv("ANGZARR_ENDPOINT", "localhost:1310")
```

### Temporal Queries

```go
// Query state as of a specific sequence
events, err := queryClient.Query("orders", rootID).
    AsOfSequence(10).
    GetEventBook(ctx)

// Query state as of a specific time
events, err := queryClient.Query("orders", rootID).
    AsOfTime("2024-01-15T10:30:00Z").
    GetEventBook(ctx)

// Query a range of sequences
events, err := queryClient.Query("orders", rootID).
    RangeTo(5, 15).
    GetEventBook(ctx)
```

### Error Handling

```go
resp, err := client.Command("orders", rootID).
    WithSequence(5).
    WithCommand(typeURL, cmd).
    Execute(ctx)

if err != nil {
    if clientErr := angzarr.AsClientError(err); clientErr != nil {
        if clientErr.IsNotFound() {
            // Aggregate doesn't exist
        } else if clientErr.IsPreconditionFailed() {
            // Sequence mismatch (optimistic locking failure)
        } else if clientErr.IsConnectionError() {
            // Network/transport error
        }
    }
}
```

### Speculative Execution

Test commands without persisting to the event store:

```go
// Connect to speculative client
specClient, err := angzarr.NewSpeculativeClient("localhost:1310")
if err != nil {
    log.Fatal(err)
}
defer specClient.Close()

// Build speculative request with temporal state
request := &pb.SpeculateAggregateRequest{
    Command: commandBook,
    Events:  priorEvents,
}

// Execute without persistence
response, err := specClient.Aggregate(ctx, request)
if err != nil {
    log.Fatal(err)
}

// Inspect projected events
for _, page := range response.Events.Pages {
    log.Printf("Would produce: %s", page.Event.TypeUrl)
}
```

## Client Types

| Client | Purpose |
|--------|---------|
| `QueryClient` | Query events from aggregates |
| `AggregateClient` | Send commands to aggregates |
| `SpeculativeClient` | Dry-run commands, test projectors/sagas |
| `DomainClient` | Combined query + aggregate for a domain |
| `Client` | Full client with all capabilities |

## Helper Functions

```go
// UUID conversion
protoUUID := angzarr.UUIDToProto(uuid)
uuid, err := angzarr.ProtoToUUID(protoUUID)

// Type URL helpers
typeURL := angzarr.TypeURL("examples.CreateOrder")  // "type.googleapis.com/examples.CreateOrder"
typeName := angzarr.TypeNameFromURL(typeURL)        // "CreateOrder"

// Cover accessors
domain := angzarr.Domain(eventBook)
correlationID := angzarr.CorrelationID(eventBook)
rootUUID := angzarr.RootUUID(eventBook)

// Sequence helpers
nextSeq := angzarr.NextSequence(eventBook)
```

## License

AGPL-3.0-only
