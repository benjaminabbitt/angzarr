# Command Handlers (Aggregates)

A **command handler** processes commands for a domain, validates business rules against current state, and emits events. In DDD terminology, this is the **aggregate**—the consistency boundary for a cluster of domain objects.

## Concepts

| Term | Definition |
|------|------------|
| **Domain** | A bounded context with its own ubiquitous language. Examples: `customer`, `transaction`, `inventory`. |
| **Aggregate Root** | The entity that owns all objects in the aggregate. Identified by UUID. |
| **Command** | A request to change state. May be accepted (events emitted) or rejected (error returned). |
| **Event** | Immutable record of state change. Past tense naming: `CustomerCreated`, not `CreateCustomer`. |
| **State** | Current aggregate state, reconstructed by replaying events. |
| **Snapshot** | Cached state at a sequence number. Optimization to skip replaying old events. |

---

## Component Responsibilities

### What Angzarr Provides

| Component | Responsibility |
|-----------|----------------|
| **BusinessCoordinator** | Receives commands from clients, loads prior events, calls your handler, persists resulting events |
| **EventStore** | Append-only storage of events with sequence validation and optimistic concurrency |
| **SnapshotStore** | Caches aggregate state at checkpoints to optimize replay |
| **EventBus** | Publishes persisted events to projectors and sagas via AMQP |
| **EventQuery** | Query service for retrieving events by domain/aggregate |

### What You Provide

| Component | Responsibility |
|-----------|----------------|
| **BusinessLogic service** | gRPC server implementing the `BusinessLogic` interface |
| **State reconstruction** | Logic to rebuild aggregate state from event history |
| **Command validation** | Business rules that accept or reject commands |
| **Event creation** | Transform valid commands into domain events |
| **Domain types** | Protobuf definitions for commands, events, and state |

---

## Architecture

```mermaid
sequenceDiagram
    participant Client
    participant Angzarr as Angzarr (BusinessCoordinator)
    participant Store as EventStore
    participant Logic as Your BusinessLogic
    participant Bus as EventBus (AMQP)

    Client->>Angzarr: CommandBook
    Note right of Angzarr: 1. Parse domain, aggregate ID
    Angzarr->>Store: Load prior events
    Store-->>Angzarr: EventBook (history)
    Note right of Angzarr: 2. Build ContextualCommand

    Angzarr->>Logic: ContextualCommand
    Note right of Logic: 1. Rebuild state from events
    Note right of Logic: 2. Validate command
    Note right of Logic: 3. Execute business logic
    Logic-->>Angzarr: BusinessResponse (events | rejection)

    alt Success
        Angzarr->>Store: Persist new events
        Angzarr->>Bus: Publish events
        Bus-->>Projectors: EventBook (async)
        Bus-->>Sagas: EventBook (async)
    end

    Angzarr-->>Client: Result
```

---

## gRPC Interfaces

### Client → Angzarr

Clients send commands to Angzarr's `BusinessCoordinator` service:

**[proto/angzarr/angzarr.proto](../proto/angzarr/angzarr.proto)**

```protobuf
// Angzarr exposes this to clients
service BusinessCoordinator {
  rpc Handle (CommandBook) returns (EventBook);
}

message CommandBook {
  Cover cover = 1;              // Domain + aggregate root ID
  repeated CommandPage pages = 2;  // Commands to process
  string correlation_id = 3;    // For tracking across services
}

message Cover {
  string domain = 1;            // e.g., "customer", "transaction"
  UUID root = 2;                // Aggregate root identifier
}

message CommandPage {
  google.protobuf.Any command = 1;  // The actual command (CreateCustomer, etc.)
}
```

### Angzarr → Your Handler

Angzarr calls your `BusinessLogic` service with prior events attached:

```protobuf
// You implement this service
service BusinessLogic {
  rpc Handle (ContextualCommand) returns (BusinessResponse);
}

message ContextualCommand {
  CommandBook command = 1;      // Original command from client
  EventBook events = 2;         // Prior events for this aggregate (for state rebuild)
}

message BusinessResponse {
  oneof result {
    EventBook events = 1;       // Success: new events to persist
    string rejection = 2;       // Failure: human-readable rejection reason
  }
}
```

### Event and State Containers

```protobuf
message EventBook {
  Cover cover = 1;                    // Domain + aggregate root ID
  repeated EventPage pages = 2;       // Events in sequence order
  google.protobuf.Any snapshot_state = 3;  // Optional: current state for snapshotting
  string correlation_id = 4;          // Propagated from command
}

message EventPage {
  int64 sequence = 1;                 // Monotonic sequence within aggregate
  google.protobuf.Any event = 2;      // The actual event (CustomerCreated, etc.)
  string timestamp = 3;               // RFC 3339 timestamp
}
```

---

## Handler Pattern

Every command handler follows this pattern:

1. **Receive** command + prior events
2. **Rebuild** current state from events (applying snapshot if present)
3. **Validate** command against state (preconditions + input validation)
4. **Execute** business logic
5. **Return** new events with updated snapshot state

**Input: ContextualCommand**
```json
{
  "command": { "CreateCustomer": { "name": "Alice", "email": "alice@example.com" } },
  "events": []
}
```

```mermaid
flowchart TD
    Input[ContextualCommand] --> S1[1. Rebuild state from events<br/>state = CustomerState::default]
    S1 --> S2[2. Validate preconditions<br/>✓ Customer doesn't exist]
    S2 --> S3[3. Validate input<br/>✓ Name provided<br/>✓ Email provided]
    S3 --> S4[4. Create event<br/>CustomerCreated]
    S4 --> S5[5. Return EventBook with new state]
    S5 --> Output[BusinessResponse]
```

**Output: BusinessResponse**
```json
{
  "events": {
    "pages": [{ "CustomerCreated": { "name": "Alice", "email": "..." } }],
    "snapshot_state": { "CustomerState": { "name": "Alice", "loyalty_points": 0 } }
  }
}
```

---

## State Reconstruction

State is rebuilt by applying events in sequence:

```
Prior Events:
  [0] CustomerCreated { name: "Alice", email: "alice@example.com" }
  [1] LoyaltyPointsAdded { points: 100, ... }
  [2] LoyaltyPointsAdded { points: 50, ... }

Rebuild:
  state = CustomerState::default()
  apply(CustomerCreated) → state.name = "Alice", state.email = "..."
  apply(LoyaltyPointsAdded) → state.loyalty_points = 100
  apply(LoyaltyPointsAdded) → state.loyalty_points = 150

Result:
  CustomerState { name: "Alice", loyalty_points: 150, ... }
```

With snapshots, only events after the snapshot are replayed:

```
Snapshot at sequence 1:
  CustomerState { name: "Alice", loyalty_points: 100 }

Events after snapshot:
  [2] LoyaltyPointsAdded { points: 50, ... }

Rebuild:
  state = snapshot.state
  apply(LoyaltyPointsAdded) → state.loyalty_points = 150
```

---

## Example Implementations

### Customer Domain

Handles customer lifecycle and loyalty points:

| Command | Precondition | Events Emitted |
|---------|--------------|----------------|
| `CreateCustomer` | Customer must not exist | `CustomerCreated` |
| `AddLoyaltyPoints` | Customer must exist, points > 0 | `LoyaltyPointsAdded` |
| `RedeemLoyaltyPoints` | Customer must exist, sufficient balance | `LoyaltyPointsRedeemed` |

**Implementations:**

| Language | File |
|----------|------|
| Rust | [examples/rust/customer/src/lib.rs](../examples/rust/customer/src/lib.rs) |
| Go | [examples/go/customer/logic/customer.go](../examples/go/customer/logic/customer.go) |
| Python | [examples/python/customer/customer_logic.py](../examples/python/customer/customer_logic.py) |

### Transaction Domain

Handles financial transactions with discounts:

| Command | Precondition | Events Emitted |
|---------|--------------|----------------|
| `CreateTransaction` | Transaction must not exist | `TransactionCreated` |
| `ApplyDiscount` | Transaction pending, valid discount | `DiscountApplied` |
| `CompleteTransaction` | Transaction pending | `TransactionCompleted` |
| `CancelTransaction` | Transaction pending | `TransactionCancelled` |

**Implementations:**

| Language | File |
|----------|------|
| Rust | [examples/rust/transaction/src/lib.rs](../examples/rust/transaction/src/lib.rs) |
| Go | [examples/go/transaction/logic/transaction.go](../examples/go/transaction/logic/transaction.go) |
| Python | [examples/python/transaction/transaction_logic.py](../examples/python/transaction/transaction_logic.py) |

---

## Domain Types

Commands, events, and state are defined in protobuf:

**[proto/examples/domains.proto](../proto/examples/domains.proto)**

### Customer Types

```protobuf
// Commands (imperative: what to do)
message CreateCustomer {
  string name = 1;
  string email = 2;
}

message AddLoyaltyPoints {
  int32 points = 1;
  string reason = 2;    // e.g., "signup_bonus", "purchase:txn-123"
}

message RedeemLoyaltyPoints {
  int32 points = 1;
  string reason = 2;    // e.g., "discount_applied"
}

// Events (past tense: what happened)
message CustomerCreated {
  string name = 1;
  string email = 2;
  string created_at = 3;  // RFC 3339 timestamp
}

message LoyaltyPointsAdded {
  int32 points = 1;
  int32 new_balance = 2;
  int32 lifetime_points = 3;
  string reason = 4;
  string added_at = 5;
}

message LoyaltyPointsRedeemed {
  int32 points = 1;
  int32 new_balance = 2;
  string reason = 3;
  string redeemed_at = 4;
}

// State (current aggregate state for snapshots)
message CustomerState {
  string name = 1;
  string email = 2;
  int32 loyalty_points = 3;     // Current balance (can decrease)
  int32 lifetime_points = 4;    // Total earned (never decreases)
}
```

---

## BDD Specifications

Business behavior is specified in Gherkin feature files:

| Feature | File |
|---------|------|
| Customer lifecycle | [examples/features/customer.feature](../examples/features/customer.feature) |
| Transaction flow | [examples/features/transaction.feature](../examples/features/transaction.feature) |

Example scenario:

```gherkin
Scenario: Add loyalty points to existing customer
  Given a customer "cust-123" exists with 50 loyalty points
  When I send an "AddLoyaltyPoints" command with 100 points
  Then a "LoyaltyPointsAdded" event is emitted
  And the customer has 150 loyalty points
```

---

## Validation Strategy

### Preconditions (State-Based)

Check current state before processing:

| Check | Error Message |
|-------|---------------|
| Customer exists for AddPoints | "Customer does not exist" |
| Customer doesn't exist for Create | "Customer already exists" |
| Transaction is pending for Complete | "Transaction is not pending" |
| Sufficient balance for Redeem | "Insufficient points" |

### Input Validation

Check command fields:

| Check | Error Message |
|-------|---------------|
| Name provided | "Customer name is required" |
| Email provided | "Customer email is required" |
| Points positive | "Points must be positive" |
| Discount valid | "Invalid discount type" |

### Error Constants

Error messages are centralized in constants for consistency:

| Language | Location |
|----------|----------|
| Rust | [examples/rust/customer/src/lib.rs](../examples/rust/customer/src/lib.rs) — `errmsg` module |
| Go | [examples/go/customer/logic/errors.go](../examples/go/customer/logic/errors.go) |
| Python | [examples/python/customer/customer_logic.py](../examples/python/customer/customer_logic.py) — `ErrorMessages` class |

---

## Testing

### Unit Tests

Each language has unit tests for the logic module:

```bash
# Rust
cargo test -p customer --lib

# Go
cd examples/go/customer && go test ./logic/...

# Python
cd examples/python/customer && uv run pytest test_customer_logic.py
```

### Acceptance Tests (BDD)

Gherkin scenarios test full behavior:

```bash
# Rust
cargo test -p customer --test cucumber

# Go
cd examples/go/customer && go test ./features/...

# Python
cd examples/python/customer && uv run pytest features/
```

---

## Next Steps

- [Projectors](projectors.md) — React to events, build read models
- [Sagas](sagas.md) — Orchestrate workflows across aggregates
