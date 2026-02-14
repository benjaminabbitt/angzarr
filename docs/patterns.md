# CQRS/ES Patterns

This guide documents common patterns used in CQRS and Event Sourcing architectures. For core concepts, see [CQRS and Event Sourcing](cqrs-event-sourcing.md).

---

## Pattern Catalog

| Category | Patterns |
|----------|----------|
| [Delivery & Consistency](#delivery--consistency-patterns) | Outbox, Idempotent Consumer |
| [Schema Evolution](#schema-evolution-patterns) | Upcasting |
| [Coordination](#coordination-patterns) | Correlation ID, Sync Mode, Merge Strategy, Process Manager, Saga |
| [Query](#query-patterns) | Temporal Query |

---

## Delivery & Consistency Patterns

### Outbox Pattern

> **WARNING: You probably don't need this.** Modern managed messaging services (Kafka, SQS, Pub/Sub, Kinesis) already guarantee delivery. The outbox pattern adds latency, complexity, and operational overhead. Only consider it if your messaging layer genuinely lacks durability—which is rare with cloud providers. See [When to Use](#when-to-use-and-when-not-to) before implementing.

The **Outbox Pattern** ensures atomicity between database writes and event publishing. Instead of publishing events directly (which can fail independently of the database transaction), events are written to an "outbox" table within the same transaction, then published asynchronously by a separate process.

#### The Problem

```
1. Aggregate handles command
2. Events persisted to event store     ← Transaction commits
3. Events published to message bus     ← This can fail!
```

If step 3 fails, events are persisted but never published. Subscribers miss events. System becomes inconsistent.

#### The Solution

```
1. Aggregate handles command
2. Events persisted to event store  }
3. Events written to outbox table   }  ← Single transaction
4. Background process polls outbox
5. Events published to message bus
6. Outbox entries marked as published
```

#### Outbox Table Schema

```sql
CREATE TABLE event_outbox (
    id              UUID PRIMARY KEY,
    aggregate_id    UUID NOT NULL,
    event_type      TEXT NOT NULL,
    event_data      JSONB NOT NULL,
    created_at      TIMESTAMP NOT NULL DEFAULT NOW(),
    published_at    TIMESTAMP,          -- NULL = not yet published
    retry_count     INT NOT NULL DEFAULT 0
);

CREATE INDEX idx_outbox_unpublished ON event_outbox (created_at)
    WHERE published_at IS NULL;
```

#### Publishing Process

```python
async def publish_outbox_events():
    """Background process that publishes pending outbox events."""
    while True:
        # Fetch unpublished events in order
        events = await db.query("""
            SELECT * FROM event_outbox
            WHERE published_at IS NULL
            ORDER BY created_at
            LIMIT 100
            FOR UPDATE SKIP LOCKED
        """)

        for event in events:
            try:
                await message_bus.publish(event)
                await db.execute("""
                    UPDATE event_outbox
                    SET published_at = NOW()
                    WHERE id = $1
                """, event.id)
            except Exception as e:
                await db.execute("""
                    UPDATE event_outbox
                    SET retry_count = retry_count + 1
                    WHERE id = $1
                """, event.id)

        await asyncio.sleep(0.1)  # Polling interval
```

#### Trade-offs

| Advantage | Disadvantage |
|-----------|--------------|
| Guaranteed delivery (at-least-once) | Added complexity (outbox table, publisher) |
| Atomic with business transaction | Polling latency (typically <100ms) |
| Survives crashes and restarts | Requires idempotent consumers |

#### When to Use (and When Not To)

> **You probably don't need the outbox pattern.** Modern managed messaging services provide strong durability guarantees. Adding an outbox on top of these services means paying twice for the same guarantee—once in your database, once in the broker. Before enabling outbox, confirm your messaging layer actually lacks durability.

| Messaging Layer | Built-in Durability | Outbox Needed? |
|-----------------|---------------------|----------------|
| **Kafka** | Yes (replicated log, configurable acks) | No |
| **AWS SQS** | Yes (redundant storage across AZs) | No |
| **AWS SNS** | Yes (with SQS subscription) | No |
| **AWS Kinesis** | Yes (replicated across AZs, 24h-365d retention) | No |
| **GCP Pub/Sub** | Yes (synchronous replication, 7d retention) | No |
| **Azure Service Bus** | Yes (geo-redundant storage) | No |
| **RabbitMQ** | Optional (persistent queues + publisher confirms) | Maybe—only if not using persistence |
| **Redis Streams** | Optional (depends on AOF/RDB config) | Maybe—if AOF disabled |
| **NATS JetStream** | Yes (replicated streams) | No |
| **In-memory/Channel** | No | Yes, if delivery matters |

**The only scenarios where outbox makes sense:**
- Using an in-memory or non-durable message transport
- Regulatory/compliance requires a local audit trail before transmission
- Network between app and broker is genuinely unreliable (rare with cloud providers)

**Skip outbox when:**
- Using any managed cloud messaging service (SQS, Pub/Sub, Kinesis, etc.)
- Using Kafka with `acks=all`
- Using RabbitMQ with persistent queues and publisher confirms
- Best-effort delivery is acceptable
- Latency is critical (outbox adds 1-5ms per publish)

#### Cost & Complexity

**Understand what you're getting into:**
- **Latency:** +1-5ms per event (2 SQL round-trips)
- **Duplication:** Events stored in outbox AND broker
- **Storage:** Outbox grows during outages
- **Operations:** Recovery process, monitoring, maintenance

If your messaging layer already guarantees delivery, outbox adds cost without benefit.

#### Disabling the Outbox

The outbox is **disabled by default**. If you've enabled it and want to turn it off:

**Via configuration:**
```yaml
messaging:
  outbox:
    enabled: false
```

**Via environment variable:**
```bash
ANGZARR_OUTBOX_ENABLED=false
```

**In Rust (RuntimeBuilder):**
```rust
// Simply don't call .with_outbox() — it's opt-in
let runtime = RuntimeBuilder::new()
    .with_event_bus(bus)  // No outbox wrapper
    .build();
```

When disabled, events are published directly to the message bus without the intermediate outbox table. This is the recommended configuration for Kafka, SQS, Pub/Sub, and other durable messaging systems.

#### Alternatives

| Approach | Description | Trade-off |
|----------|-------------|-----------|
| **Change Data Capture (CDC)** | Database log tailing (Debezium) | Infrastructure complexity |
| **Transactional Event Store** | Event store with built-in pub/sub | Vendor lock-in |
| **Listen-to-Yourself** | Consumer reads from same store | Eventual consistency only |

---

### Idempotent Consumer

Consumers should be **naturally idempotent** where possible:

| Operation | Idempotent? | Fix |
|-----------|-------------|-----|
| `INSERT` | No | Use `INSERT ... ON CONFLICT DO NOTHING` |
| `UPDATE SET x = x + 1` | No | Use `UPDATE SET x = $value` (absolute) |
| `UPDATE SET x = $value` | Yes | Already idempotent |
| `DELETE WHERE id = $1` | Yes | Already idempotent |

**Event sourcing helps:** Events contain absolute state (`new_balance: 150`), not deltas (`add: 50`). Replaying produces the same result.

---

## Schema Evolution Patterns

### Upcasting

**Upcasting** transforms old event versions to the current version when reading from the event store. The stored events remain unchanged; transformation happens on read.

#### The Problem

Event schemas evolve:
- Fields renamed (`customerId` → `customer_id`)
- Fields added (new `currency` field with default)
- Fields removed (deprecated `legacy_flag`)
- Structure changed (flat → nested)

Old events in the store don't match current code expectations.

#### The Solution

```
Event Store          Upcaster Chain           Application
┌─────────────┐      ┌─────────────┐         ┌─────────────┐
│ OrderV1     │──────│ V1 → V2     │────────▶│ OrderV3     │
│ OrderV2     │──────│ V2 → V3     │────────▶│ (current)   │
│ OrderV3     │──────│ (passthrough)│────────▶│             │
└─────────────┘      └─────────────┘         └─────────────┘
```

#### Implementation

```rust
pub trait Upcaster {
    fn can_upcast(&self, event_type: &str, version: u32) -> bool;
    fn upcast(&self, event: RawEvent) -> RawEvent;
}

pub struct OrderCreatedV1ToV2;

impl Upcaster for OrderCreatedV1ToV2 {
    fn can_upcast(&self, event_type: &str, version: u32) -> bool {
        event_type == "OrderCreated" && version == 1
    }

    fn upcast(&self, mut event: RawEvent) -> RawEvent {
        // V1 had "customerId", V2 has "customer_id"
        if let Some(customer_id) = event.data.remove("customerId") {
            event.data.insert("customer_id".to_string(), customer_id);
        }
        // V2 added "currency" with default
        event.data.entry("currency".to_string())
            .or_insert(json!("USD"));

        event.version = 2;
        event
    }
}
```

#### Upcaster Chain

```rust
pub struct UpcasterChain {
    upcasters: Vec<Box<dyn Upcaster>>,
}

impl UpcasterChain {
    pub fn upcast_to_current(&self, mut event: RawEvent) -> RawEvent {
        loop {
            let mut upcasted = false;
            for upcaster in &self.upcasters {
                if upcaster.can_upcast(&event.event_type, event.version) {
                    event = upcaster.upcast(event);
                    upcasted = true;
                    break;
                }
            }
            if !upcasted {
                break;  // No more upcasters apply
            }
        }
        event
    }
}
```

#### Rules for Valid Upcasts

A new version must be **derivable** from the old version:

| Change | Valid Upcast? | Approach |
|--------|---------------|----------|
| Add field with default | Yes | Insert default value |
| Rename field | Yes | Copy value to new name |
| Remove field | Yes | Drop field (no action) |
| Change type (compatible) | Yes | Convert value |
| Change type (incompatible) | No | New event type needed |
| Change semantic meaning | No | New event type needed |

**Greg Young's rule:** If you can't derive the new version from the old, it's not a new version—it's a new event.

#### Trade-offs

| Advantage | Disadvantage |
|-----------|--------------|
| Events are immutable (audit trail preserved) | Transformation cost on every read |
| Old code can still read (forward compatibility) | Chain complexity grows over time |
| No migration needed | Must maintain upcaster for each version |

#### Angzarr Implementation

Angzarr implements upcasting as a separate container in the aggregate pod, called via gRPC:

```
┌─────────────────────────────────────────────────────────────────┐
│                      Aggregate Pod                               │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐      │
│  │   Angzarr    │───▶│   Upcaster   │───▶│  Business    │      │
│  │   Sidecar    │    │  Container   │    │   Logic      │      │
│  └──────────────┘    └──────────────┘    └──────────────┘      │
└─────────────────────────────────────────────────────────────────┘
```

**Configuration:**

```yaml
upcaster:
  enabled: true
  address: "localhost:50053"
  timeout_ms: 5000
```

Or via environment:
- `ANGZARR_UPCASTER_ENABLED=true`
- `ANGZARR_UPCASTER_ADDRESS=localhost:50053`

**Proto definition:**

```protobuf
service Upcaster {
  rpc Upcast (UpcastRequest) returns (UpcastResponse);
}

message UpcastRequest {
  string domain = 1;
  repeated EventPage events = 2;
}

message UpcastResponse {
  repeated EventPage events = 1;
}
```

**Flow:**

1. Events loaded from storage
2. Entire EventBook passed to upcaster in one call
3. Upcaster returns transformed events (same order, same count)
4. Transformed events passed to client logic

This design allows:
- Language-agnostic upcasters (implement in any language)
- Independent deployment lifecycle
- Clear separation of concerns

---

## Coordination Patterns

### Correlation ID

The **correlation_id** links related events across domains in a multi-step business workflow. It's an optional, client-provided identifier — the framework does NOT generate one if absent.

#### When You Need correlation_id

| Use Case | Required? | Reason |
|----------|-----------|--------|
| **Process Managers** | Yes | PM aggregates use correlation_id as root. Without it, PMs don't activate. |
| **Event Streaming** | Yes | `ExecuteStream` subscribes to events by correlation_id. Empty = no subscription. |
| **Cross-domain tracing** | Yes | Following a business transaction across order → fulfillment → shipping. |
| **Single-domain commands** | No | A simple "create cart" command doesn't need correlation. |
| **Independent aggregates** | No | Aggregates that don't participate in workflows don't need it. |

#### When You Don't Need correlation_id

**Most commands don't need correlation_id.** If you're not:
- Using Process Managers
- Using `ExecuteStream` (real-time event streaming)
- Tracing a business process across multiple domains

...then leave correlation_id empty. The framework will process your command normally.

#### Framework Behavior

```
Client sends command:
  ├─ With correlation_id → PMs activate, events linked across domains
  └─ Without correlation_id → Command processed, PMs skip
```

**Key points:**
- Framework does NOT auto-generate correlation_id
- Empty correlation_id is valid for single-domain operations
- PMs check for empty correlation_id and skip processing (by design)
- Streaming endpoints reject empty correlation_id (nothing to subscribe to)

#### How correlation_id Propagates

Once set on the initial command, angzarr propagates it automatically:

```
1. Client → Order Aggregate: CreateOrder (correlation_id: "order-123")
2. Order Aggregate emits: OrderCreated (correlation_id: "order-123")
3. Saga receives: OrderCreated
4. Saga emits: CreateShipment (correlation_id: "order-123")  ← propagated
5. Fulfillment Aggregate emits: ShipmentCreated (correlation_id: "order-123")
6. PM receives: All events with correlation_id: "order-123"
```

You set it once; the framework carries it through.

#### Choosing a correlation_id Value

Use a meaningful business identifier:

| Good | Why |
|------|-----|
| `order-{order_id}` | Traces an order through its lifecycle |
| `checkout-{session_id}` | Groups all checkout-related events |
| `reservation-{booking_ref}` | Links reservation workflow events |

| Avoid | Why |
|-------|-----|
| Random UUID for every command | Creates meaningless correlation; use domain root instead |
| Reusing IDs across unrelated workflows | Events get incorrectly grouped |
| User ID as correlation | Too broad; one user has many workflows |

#### Example: With and Without correlation_id

**Without correlation_id** (simple command):

```bash
# Cart operations - no cross-domain workflow
# Connect directly to the cart aggregate coordinator
grpcurl -plaintext -d '{
  "cover": {
    "domain": "cart",
    "root": {"value": "BASE64_CART_UUID"}
  },
  "pages": [...]
}' localhost:1310 angzarr.AggregateCoordinator/Handle
```

**With correlation_id** (workflow requiring PM):

```bash
# Order checkout - triggers fulfillment PM
# Connect directly to the order aggregate coordinator
grpcurl -plaintext -d '{
  "cover": {
    "domain": "order",
    "root": {"value": "BASE64_ORDER_UUID"},
    "correlation_id": "checkout-abc123"
  },
  "pages": [...]
}' localhost:1310 angzarr.AggregateCoordinator/Handle
```

#### Anti-Patterns

**Don't add correlation_id "just in case":**

```python
# WRONG - adding correlation_id to everything
def create_cart(user_id: str) -> CommandBook:
    return CommandBook(
        cover=Cover(
            domain="cart",
            root=new_uuid(),
            correlation_id=f"cart-{new_uuid()}"  # Unnecessary
        ),
        ...
    )

# RIGHT - only when needed for cross-domain workflows
def checkout_cart(cart_id: str, order_id: str) -> CommandBook:
    return CommandBook(
        cover=Cover(
            domain="order",
            root=order_uuid,
            correlation_id=f"checkout-{order_id}"  # Needed: PM will track this
        ),
        ...
    )
```

**Don't confuse correlation_id with aggregate root:**
- **Root**: Identity of the aggregate instance (e.g., order-123)
- **Correlation ID**: Links events across domains in a workflow

They can be related (e.g., correlation_id = "order-{order_id}") but serve different purposes.

---

### Sync Mode

Commands can be executed asynchronously (default) or synchronously with different levels of coordination. Use `SyncCommandBook` with a `SyncMode` to control this behavior.

#### Sync Modes

```protobuf
enum SyncMode {
  SYNC_MODE_NONE = 0;     // Async: fire and forget (default)
  SYNC_MODE_SIMPLE = 1;   // Wait for sync projectors only
  SYNC_MODE_CASCADE = 2;  // Full sync: projectors + saga cascade
}
```

| Mode | Behavior | Latency | Use Case |
|------|----------|---------|----------|
| `NONE` | Publish events to bus, return immediately | Lowest | Most commands; eventual consistency acceptable |
| `SIMPLE` | Wait for registered sync projectors | Medium | Read-after-write consistency for projections |
| `CASCADE` | Wait for projectors + downstream saga effects | Highest | Full workflow must complete before response |

#### EventBook Repair: Getting Context from the Bus

When projectors receive events via the bus, they may get **incomplete EventBooks**—just the new events from the current command, not the full aggregate history.

**The problem:**
```
Aggregate has events: [0, 1, 2, 3, 4]
Command produces: [5, 6]
Projector receives via bus: [5, 6] ← Missing context!
```

**The solution:** `EventBookRepairer` automatically detects incomplete EventBooks and fetches the full history from the EventQuery service before forwarding to projectors.

An EventBook is **complete** if:
- It has a snapshot, OR
- Its first event has sequence 0

```rust
// In ProjectorCoordinatorService
let event_book = self.repairer.repair(event_book).await?;
// Now projector has events [0, 1, 2, 3, 4, 5, 6]
```

#### When to Use Each Mode

**SYNC_MODE_NONE (default):**
```rust
// Fire and forget - client doesn't wait for projectors
gateway.execute(command_book).await?;
```
- Command processed, events published to bus
- Projectors run asynchronously in background
- Lowest latency, eventual consistency

**SYNC_MODE_SIMPLE:**
```rust
// Wait for sync projectors to complete
let sync_command = SyncCommandBook {
    command: Some(command_book),
    sync_mode: SyncMode::Simple.into(),
};
let response = aggregate.handle_sync(sync_command).await?;
// response.projections contains sync projector results
```
- Command processed, events persisted
- Sync projectors called and awaited
- Response includes projection results
- Use when client needs read-after-write consistency

**SYNC_MODE_CASCADE:**
```rust
// Wait for full saga cascade to complete
let sync_command = SyncCommandBook {
    command: Some(command_book),
    sync_mode: SyncMode::Cascade.into(),
};
let response = aggregate.handle_sync(sync_command).await?;
```
- Command processed, events persisted
- Sync projectors called
- Downstream saga effects awaited
- **Expensive:** Use sparingly, only when the entire workflow must complete synchronously

#### Performance Considerations

| Mode | DB Writes | Network Calls | Typical Latency |
|------|-----------|---------------|-----------------|
| `NONE` | 1 (events) | 1 (bus publish) | 5-20ms |
| `SIMPLE` | 1 (events) | 1 + N (projectors) | 20-100ms |
| `CASCADE` | 1 + M (saga targets) | 1 + N + M | 100ms-seconds |

**Guidelines:**
- Default to `NONE` unless you have a specific consistency requirement
- Use `SIMPLE` for APIs that read back data immediately after write
- Avoid `CASCADE` in hot paths; consider background processing instead

---

### Merge Strategy

The **MergeStrategy** enum controls how the aggregate coordinator handles sequence conflicts when concurrent commands target the same aggregate. This is optimistic concurrency control at the framework level.

#### The Problem

In event sourcing, each command must specify the sequence number it expects:

```
Command 1: sequence=5 → succeeds (aggregate at seq 5)
Command 2: sequence=5 → conflict! (aggregate now at seq 6)
```

Without concurrency control, the second command would silently overwrite or conflict. MergeStrategy determines the response.

#### Strategies

```protobuf
enum MergeStrategy {
  MERGE_COMMUTATIVE = 0;      // Default: retryable conflict
  MERGE_STRICT = 1;           // Immediate rejection
  MERGE_AGGREGATE_HANDLES = 2; // Delegate to aggregate
}
```

| Strategy | Error Code | Behavior | Use Case |
|----------|------------|----------|----------|
| `COMMUTATIVE` | `FAILED_PRECONDITION` | Retryable error with fresh state | Most commands; sagas with auto-retry |
| `STRICT` | `ABORTED` | Immediate, non-retryable rejection | Commands requiring latest state |
| `AGGREGATE_HANDLES` | N/A (bypasses validation) | Aggregate implements conflict resolution | Counter increments, set operations, CRDTs |

#### MERGE_COMMUTATIVE (Default)

Most commands use COMMUTATIVE. When a sequence conflict occurs:

1. Coordinator returns `FAILED_PRECONDITION` with current EventBook
2. Client extracts fresh state from error details
3. Client rebuilds command with correct sequence
4. Client retries

**Saga retry flow:**
```
Saga emits command (seq=5) → Conflict (aggregate at seq=7)
                           ← FAILED_PRECONDITION + EventBook
Saga fetches fresh state (seq=7)
Saga emits command (seq=7) → Success
```

#### MERGE_STRICT

Use when commands MUST be based on the absolute latest state:

```python
# Financial transfer - stale state could cause overdraft
command = CommandBook(
    cover=Cover(domain="account", root=account_id),
    pages=[CommandPage(
        sequence=current_seq,
        command=TransferFunds(amount=1000),
        merge_strategy=MergeStrategy.MERGE_STRICT,
    )],
)
```

On conflict: `ABORTED` error. Client must explicitly reload state and re-evaluate.

#### MERGE_AGGREGATE_HANDLES

Bypass coordinator validation entirely. The aggregate receives the full EventBook and implements its own conflict resolution:

```rust
// Counter aggregate - increments are naturally commutative
impl AggregateHandler for CounterAggregate {
    async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, Status> {
        let current_value = self.compute_value(&ctx.events);
        let increment = extract_increment(&ctx.command);

        // No conflict possible - just add
        Ok(EventBook {
            pages: vec![EventPage {
                sequence: ctx.events.next_sequence(),
                event: CounterIncremented { new_value: current_value + increment },
            }],
            ..Default::default()
        })
    }
}
```

Use cases:
- Counter aggregates (increment by N)
- Set operations (add/remove items)
- CRDTs (conflict-free replicated data types)
- Idempotent operations

#### Setting MergeStrategy

**Proto:**
```protobuf
message CommandPage {
  uint32 sequence = 1;
  google.protobuf.Any command = 2;
  MergeStrategy merge_strategy = 3;
}
```

**Rust:**
```rust
CommandPage {
    sequence: next_seq,
    command: Some(any_packed_command),
    merge_strategy: MergeStrategy::MergeStrict as i32,
}
```

**Python:**
```python
CommandPage(
    sequence=next_seq,
    command=any_packed_command,
    merge_strategy=MergeStrategy.MERGE_COMMUTATIVE,
)
```

**Go:**
```go
&pb.CommandPage{
    Sequence:      nextSeq,
    Command:       anyPackedCommand,
    MergeStrategy: pb.MergeStrategy_MERGE_STRICT,
}
```

#### Testing

Comprehensive integration tests are available in Gherkin format:
- **Feature file:** [`examples/features/unit/merge_strategy.feature`](../examples/features/unit/merge_strategy.feature)
- **Rust tests:** `tests/standalone_integration/merge_strategy.rs`

The Gherkin tests document all edge cases including:
- Correct sequence succeeds for all strategies
- Stale sequence behavior per strategy
- Future sequence handling
- Default strategy when unspecified
- Counter and set aggregate examples

---

### Process Manager

> **WARNING: You probably don't need this.** Before implementing a Process Manager, ask yourself:
> 1. Can a simple saga + destination queries solve this?
> 2. Is the "state" you want to track already derivable from existing aggregates?
> 3. Are you adding Process Manager because the workflow is genuinely complex?
>
> **Default to saga.** Only use Process Manager when saga cannot handle your use case.

A **Process Manager** is a stateful coordinator for long-running workflows that span multiple aggregates. It is implemented as its own aggregate domain, with event-sourced state and correlation_id-based tracking. See [Process Manager](components/process-manager/process-manager.md) for full implementation guide.

#### When Process Manager Is Warranted

- Workflow state is NOT derivable from aggregates (PM owns unique state)
- You need to query workflow status independently ("show all pending fulfillments")
- Timeout/scheduling logic is complex enough to merit its own aggregate
- You must react to events from MULTIPLE domains (fan-in pattern)

#### Saga vs Process Manager

| Aspect | Saga | Process Manager |
|--------|------|-----------------|
| State | Stateless | Event-sourced in own domain |
| Domain subscription | Single domain (recommended) | Multiple domains |
| Complexity | Low | High |
| Correlation | Via cover.correlation_id | Via cover.correlation_id |
| Use case | Simple event → commands | Complex multi-step workflows |
| Timeouts | Not built-in | TimeoutScheduler |

#### State Design: Workflow, Not Aggregate Mirror

Process Manager state should be **workflow-oriented**, not a mirror of aggregate fields:

```protobuf
// WRONG - just mirroring aggregate state
message OrderFulfillmentState {
  bool payment_confirmed = 1;   // Copy of payment aggregate
  bool inventory_reserved = 2;  // Copy of inventory aggregate
  string customer_email = 3;    // Copy of customer aggregate
}

// RIGHT - workflow-focused
message OrderFulfillmentState {
  string order_id = 1;
  FulfillmentStage stage = 2;                    // Workflow concept
  repeated string completed_prerequisites = 3;   // What has happened
  repeated string pending_prerequisites = 4;     // What we're waiting for
  bool final_action_issued = 5;                  // Idempotency guard
  google.protobuf.Timestamp deadline = 6;        // Workflow timeout
}
```

If your PM state is just copies of aggregate fields, you don't need a PM — query the aggregates.

#### Fan-In Pattern

The primary reason for Process Manager: waiting for multiple domains to complete.

```
Payment domain:   PaymentConfirmed   → PM: { payment: done }     → no action
Shipping domain:  CarrierAssigned    → PM: { shipping: done }    → no action
Inventory domain: StockReserved      → PM: { inventory: done }   → ALL DONE → FulfillmentReady
```

Saga cannot handle this (race condition when events arrive simultaneously). PM serializes via aggregate sequence.

#### Two-Phase Protocol

```protobuf
service ProcessManager {
  rpc GetSubscriptions (GetSubscriptionsRequest) returns (GetSubscriptionsResponse);
  rpc Prepare (ProcessManagerPrepareRequest) returns (ProcessManagerPrepareResponse);
  rpc Handle (ProcessManagerHandleRequest) returns (ProcessManagerHandleResponse);
}
```

1. **GetSubscriptions**: PM declares which domains it subscribes to (at startup)
2. **Prepare**: PM declares additional destinations needed beyond trigger
3. **Handle**: PM receives full context, returns commands + PM events

#### Timeouts

Process managers use the `TimeoutScheduler` service:

1. PM state includes deadline timestamps
2. TimeoutScheduler queries for stale process instances
3. Emits `ProcessTimeout` events to the bus
4. PM handles timeout events like any other event

For full details, see [Process Manager](components/process-manager/process-manager.md).

---

## Query Patterns

### Temporal Query

**Temporal Query** retrieves the state of an aggregate at any point in history. Event sourcing makes this trivial: replay events up to the desired point.

#### Use Cases

- **Audit:** What was the account balance on March 15th?
- **Debugging:** What was the system state when the bug occurred?
- **Compliance:** Prove what data existed at time of transaction
- **Analytics:** Historical trend analysis

#### Query API

Temporal queries are a first-class selection mode in the `Query` message:

```protobuf
message TemporalQuery {
  oneof point_in_time {
    google.protobuf.Timestamp as_of_time = 1;  // Events with created_at <= this
    uint32 as_of_sequence = 2;                  // Events with sequence <= this
  }
}

message Query {
  Cover cover = 1;
  oneof selection {
    SequenceRange range = 3;
    SequenceSet sequences = 4;
    TemporalQuery temporal = 5;   // Point-in-time query
  }
}
```

Two modes:
- **`as_of_time`** — Returns all events with `created_at` <= the specified timestamp. Use when you need state at a real-world point in time (audit, compliance).
- **`as_of_sequence`** — Returns all events with sequence <= the specified number. Use when you need state at a logical point (debugging, replay).

Both modes replay from sequence 0 without snapshots, ensuring correct historical reconstruction.

#### gRPC Usage

Query through the `EventQuery` service:

```protobuf
service EventQuery {
  rpc GetEventBook (Query) returns (EventBook);       // Unary
  rpc GetEvents (Query) returns (stream EventBook);    // Server streaming
  rpc Synchronize (stream Query) returns (stream EventBook);  // Bidirectional
}
```

**By timestamp** — "What was this cart's state at midnight Jan 2, 2025?"

```bash
grpcurl -plaintext -d '{
  "cover": {
    "domain": "cart",
    "root": {"value": "BASE64_ENCODED_UUID"}
  },
  "temporal": {
    "as_of_time": "2025-01-02T00:00:00Z"
  }
}' localhost:50052 angzarr.EventQuery/GetEventBook
```

**By sequence** — "What was this cart's state after the 3rd event?"

```bash
grpcurl -plaintext -d '{
  "cover": {
    "domain": "cart",
    "root": {"value": "BASE64_ENCODED_UUID"}
  },
  "temporal": {
    "as_of_sequence": 2
  }
}' localhost:50052 angzarr.EventQuery/GetEventBook
```

Sequences are zero-indexed: `as_of_sequence: 2` returns events 0, 1, 2.

#### Programmatic Usage

**Rust:**
```rust
use angzarr::proto::{Cover, Query, TemporalQuery};
use angzarr::proto::query::Selection;
use angzarr::proto::temporal_query::PointInTime;

let query = Query {
    cover: Some(Cover {
        domain: "cart".to_string(),
        root: Some(proto_uuid),
        correlation_id: String::new(),
    }),
    selection: Some(Selection::Temporal(TemporalQuery {
        point_in_time: Some(PointInTime::AsOfTime(prost_types::Timestamp {
            seconds: 1735776000, // 2025-01-02T00:00:00Z
            nanos: 0,
        })),
    })),
};

let response = event_query_client.get_event_book(query).await?;
let book = response.into_inner();
// book.pages contains events up to the specified timestamp
// book.snapshot is None (temporal queries skip snapshots)
```

**Python:**
```python
from angzarr import angzarr_pb2 as angzarr
from google.protobuf.timestamp_pb2 import Timestamp

query = angzarr.Query(
    cover=angzarr.Cover(domain="cart", root=angzarr.Uuid(value=root_bytes)),
    temporal=angzarr.TemporalQuery(
        as_of_time=Timestamp(seconds=1735776000),
    ),
)

response = event_query_stub.GetEventBook(query)
# response.pages contains events up to the specified timestamp
```

**Go:**
```go
query := &pb.Query{
    Cover: &pb.Cover{
        Domain: "cart",
        Root:   &pb.Uuid{Value: rootBytes},
    },
    Selection: &pb.Query_Temporal{
        Temporal: &pb.TemporalQuery{
            PointInTime: &pb.TemporalQuery_AsOfTime{
                AsOfTime: timestamppb.New(time.Date(2025, 1, 2, 0, 0, 0, 0, time.UTC)),
            },
        },
    },
}

response, err := eventQueryClient.GetEventBook(ctx, query)
// response.Pages contains events up to the specified timestamp
```

#### How It Works Internally

1. The `EventQuery` service receives the `Query` with `TemporalQuery` selection
2. Angzarr routes to the appropriate repository method:
   - `as_of_time` → `get_temporal_by_time()` — filters by `created_at <= timestamp`
   - `as_of_sequence` → `get_temporal_by_sequence()` — filters by `sequence <= n`
3. Events are replayed from sequence 0 (snapshots are intentionally skipped)
4. The returned `EventBook` contains only events up to the specified point
5. The client reconstructs state by applying events in order

Snapshots are skipped because a snapshot may have been taken *after* the requested point in time, which would produce incorrect historical state.

#### Performance

All storage backends maintain indexes for temporal queries:

| Backend | Index |
|---------|-------|
| PostgreSQL | `(domain, root, created_at)` |
| SQLite | `(domain, root, created_at)` |
| MongoDB | `created_at` field index |
| Redis | Sorted set with timestamp scores |

For aggregates with many events, temporal queries replay from the beginning. If this becomes a bottleneck, build temporal projections (see below).

#### Temporal Projections

For frequently-queried historical data, build a projector that materializes snapshots:

```sql
-- Daily balance snapshots for reporting
CREATE TABLE account_balance_history (
    account_id  UUID NOT NULL,
    as_of_date  DATE NOT NULL,
    balance     DECIMAL(18, 2) NOT NULL,
    PRIMARY KEY (account_id, as_of_date)
);
```

A projector populates this at end of each day or on-demand. This shifts the cost from query time to write time — appropriate when the same historical points are queried repeatedly.

---

## Next Steps

- [Sagas](components/saga/sagas.md) — Cross-aggregate workflows and compensation
- [Projectors](components/projector/projectors.md) — Building read models from event streams
- [Command Handlers](components/aggregate/aggregate.md) — Processing commands and emitting events
