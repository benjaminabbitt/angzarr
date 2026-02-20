# angzarr-client

Rust client library for Angzarr event-sourcing services.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
angzarr-client = "0.1"
```

## Usage

### Sending Commands

```rust
use angzarr_client::{DomainClient, CommandBuilder};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to aggregate coordinator
    let client = DomainClient::connect("http://localhost:1310").await?;

    // Send a command to create a new aggregate
    let response = CommandBuilder::create_new(&client.aggregate, "orders")
        .with_correlation_id("order-123")
        .with_command("type.googleapis.com/examples.CreateOrder", &create_order_cmd)?
        .execute()
        .await?;

    // Get the new aggregate root ID from response
    let root_id = angzarr_client::root_uuid(&response.events)?;
    println!("Created order: {}", root_id);

    Ok(())
}
```

### Querying Events

```rust
use angzarr_client::{DomainClient, QueryBuilder};
use uuid::Uuid;

// Connect to query service
let client = DomainClient::connect("http://localhost:1310").await?;

// Query events for an aggregate
let root_id = Uuid::parse_str("...")?;
let events = QueryBuilder::new(&client.query, "orders", root_id)
    .get_event_book()
    .await?;

// Iterate over events
for page in events.pages {
    println!("Event {}: {}",
        angzarr_client::sequence_num(&page),
        angzarr_client::type_name_from_url(&page.event.as_ref().unwrap().type_url));
}
```

### Using Environment Variables

```rust
// Connect using environment variable with fallback
let client = DomainClient::from_env("ANGZARR_ENDPOINT", "http://localhost:1310").await?;
```

### Temporal Queries

```rust
// Query state as of a specific sequence
let events = QueryBuilder::new(&client.query, "orders", root_id)
    .as_of_sequence(10)
    .get_event_book()
    .await?;

// Query state as of a specific time
let events = QueryBuilder::new(&client.query, "orders", root_id)
    .as_of_time("2024-01-15T10:30:00Z")?
    .get_event_book()
    .await?;

// Query a range of sequences
let events = QueryBuilder::new(&client.query, "orders", root_id)
    .range_to(5, 15)
    .get_event_book()
    .await?;
```

### Error Handling

```rust
use angzarr_client::{ClientError, DomainClient};

match client.aggregate.handle(command).await {
    Ok(response) => {
        // Process response
    }
    Err(ClientError::NotFound(msg)) => {
        // Aggregate doesn't exist
    }
    Err(ClientError::PreconditionFailed(msg)) => {
        // Sequence mismatch (optimistic locking failure)
    }
    Err(ClientError::InvalidArgument(msg)) => {
        // Invalid command arguments
    }
    Err(ClientError::Connection(msg)) => {
        // Network/transport error
    }
    Err(e) => {
        // Other errors
    }
}
```

### Speculative Execution

Test commands without persisting to the event store:

```rust
use angzarr_client::SpeculativeClient;
use angzarr_client::proto::SpeculateAggregateRequest;

// Connect to speculative client
let client = SpeculativeClient::connect("http://localhost:1310").await?;

// Build speculative request with temporal state
let request = SpeculateAggregateRequest {
    command: Some(command_book),
    events: prior_events,
};

// Execute without persistence
let response = client.aggregate(request).await?;

// Inspect projected events
for page in response.events.as_ref().unwrap().pages {
    println!("Would produce: {}", page.event.as_ref().unwrap().type_url);
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

## Error Types

| Error Variant | Description |
|---------------|-------------|
| `ClientError::NotFound` | Aggregate doesn't exist |
| `ClientError::PreconditionFailed` | Sequence mismatch (optimistic locking) |
| `ClientError::InvalidArgument` | Invalid command arguments |
| `ClientError::Connection` | Network/transport error |
| `ClientError::Grpc` | gRPC-level error |

## Feature Flags

```toml
[dependencies]
angzarr-client = { version = "0.1", features = ["macros"] }
```

| Feature | Description |
|---------|-------------|
| `macros` | Enable proc macros for aggregate/saga handlers |

## Router Macros

With the `macros` feature, you can use proc macros for cleaner handler definitions:

```rust
use angzarr_macros::handles;

#[handles(RegisterPlayer)]
async fn register(
    ctx: &HandlerContext,
    cmd: RegisterPlayer,
    state: &PlayerState,
) -> Result<PlayerRegistered, CommandRejectedError> {
    // Handler implementation
}
```

## License

AGPL-3.0-only
