# CQRS/ES Patterns

This guide documents common patterns used in CQRS and Event Sourcing architectures. For core concepts, see [CQRS and Event Sourcing](cqrs-event-sourcing.md).

---

## Pattern Catalog

| Category | Patterns |
|----------|----------|
| [Delivery & Consistency](#delivery--consistency-patterns) | Outbox, Inbox, Idempotent Consumer |
| [Schema Evolution](#schema-evolution-patterns) | Upcasting, Weak Schema, Double Publish |
| [Coordination](#coordination-patterns) | Process Manager, Saga |
| [Query](#query-patterns) | Temporal Query, Snapshot Query |

---

## Delivery & Consistency Patterns

### Outbox Pattern

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

**The outbox pattern is often superfluous.** Many messaging systems already provide durability:

| Messaging Layer | Built-in Durability | Outbox Needed? |
|-----------------|---------------------|----------------|
| **Kafka** | Yes (replicated log) | Rarely |
| **RabbitMQ** | Optional (persistent queues) | Maybe |
| **In-memory** | No | Yes, if delivery matters |

**Use outbox when:**
- Network to broker is unreliable
- Broker lacks durability guarantees
- Compliance requires local audit trail
- You need exactly-once semantics

**Skip outbox when:**
- Using Kafka or durable brokers (you're paying twice for the same guarantee)
- Best-effort delivery acceptable
- Latency is critical

#### Cost & Complexity

**Understand what you're getting into:**
- **Latency:** +1-5ms per event (2 SQL round-trips)
- **Duplication:** Events stored in outbox AND broker
- **Storage:** Outbox grows during outages
- **Operations:** Recovery process, monitoring, maintenance

If your messaging layer already guarantees delivery, outbox adds cost without benefit.

#### Alternatives

| Approach | Description | Trade-off |
|----------|-------------|-----------|
| **Change Data Capture (CDC)** | Database log tailing (Debezium) | Infrastructure complexity |
| **Transactional Event Store** | Event store with built-in pub/sub | Vendor lock-in |
| **Listen-to-Yourself** | Consumer reads from same store | Eventual consistency only |

---

### Inbox Pattern

The **Inbox Pattern** ensures idempotent message processing. Incoming message IDs are stored, and duplicates are detected and ignored.

#### The Problem

With at-least-once delivery, consumers may receive the same message multiple times:
- Publisher retries after timeout (but message was delivered)
- Message broker redelivers after consumer crash
- Network partition causes duplicate delivery

#### The Solution

```sql
CREATE TABLE message_inbox (
    message_id      UUID PRIMARY KEY,
    processed_at    TIMESTAMP NOT NULL DEFAULT NOW()
);
```

```python
async def handle_message(message):
    # Check if already processed
    exists = await db.query("""
        SELECT 1 FROM message_inbox WHERE message_id = $1
    """, message.id)

    if exists:
        log.info(f"Duplicate message {message.id}, skipping")
        return

    async with db.transaction():
        # Process the message
        await process_business_logic(message)

        # Record as processed (same transaction)
        await db.execute("""
            INSERT INTO message_inbox (message_id) VALUES ($1)
        """, message.id)
```

#### Inbox Cleanup

Old entries can be pruned after a retention period:

```sql
DELETE FROM message_inbox
WHERE processed_at < NOW() - INTERVAL '7 days';
```

---

### Idempotent Consumer

Beyond the inbox pattern, consumers should be **naturally idempotent** where possible:

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
4. Transformed events passed to business logic

This design allows:
- Language-agnostic upcasters (implement in any language)
- Independent deployment lifecycle
- Clear separation of concerns

---

### Weak Schema

**Weak Schema** uses tolerant readers that map available fields and provide defaults for missing ones. No explicit versioning required.

```python
def deserialize_order_created(data: dict) -> OrderCreated:
    return OrderCreated(
        order_id=data["order_id"],
        customer_id=data.get("customer_id") or data.get("customerId"),  # Handle rename
        currency=data.get("currency", "USD"),  # Default for new field
        items=data.get("items", []),
    )
```

**Best for:** JSON/document stores, rapid iteration, forward compatibility.

**Avoid when:** Strict contracts required, breaking changes frequent.

---

### Double Publish

During migration, publish **both old and new versions** of events:

```python
def publish_order_created(order):
    # Old consumers still work
    publish(OrderCreatedV1(
        customerId=order.customer_id,
        total=order.total,
    ))

    # New consumers use new version
    publish(OrderCreatedV2(
        customer_id=order.customer_id,
        total_cents=order.total_cents,
        currency=order.currency,
    ))
```

**Migration steps:**
1. Deploy producer with double publish
2. Migrate consumers to V2
3. Remove V1 publishing

---

## Coordination Patterns

### Process Manager

> **WARNING: You probably don't need this.** Before implementing a Process Manager, ask yourself:
> 1. Can a simple saga + destination queries solve this?
> 2. Is the "state" you want to track already derivable from existing aggregates?
> 3. Are you adding Process Manager because the workflow is genuinely complex?
>
> **Default to saga.** Only use Process Manager when saga cannot handle your use case.

A **Process Manager** is a stateful coordinator for long-running workflows that span multiple aggregates. It is implemented as its own aggregate domain, with event-sourced state and correlation_id-based tracking. See [Process Manager](process-manager.md) for full implementation guide.

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

For full details, see [Process Manager](process-manager.md).

---

## Query Patterns

### Temporal Query

**Temporal Query** retrieves the state of an aggregate at any point in history. Event sourcing makes this trivial: replay events up to the desired point.

#### Use Cases

- **Audit:** What was the account balance on March 15th?
- **Debugging:** What was the system state when the bug occurred?
- **Compliance:** Prove what data existed at time of transaction
- **Analytics:** Historical trend analysis

#### Implementation

```rust
impl EventStore {
    /// Query state at a specific point in time.
    pub async fn get_state_at(
        &self,
        aggregate_id: &Uuid,
        as_of: DateTime<Utc>,
    ) -> Result<AggregateState> {
        // Fetch events up to timestamp
        let events = self.get_events(Query {
            aggregate_id: Some(*aggregate_id),
            until_timestamp: Some(as_of),
            ..Default::default()
        }).await?;

        // Replay to build state
        let mut state = AggregateState::default();
        for event in events {
            state.apply(&event);
        }

        Ok(state)
    }

    /// Query state at a specific sequence number.
    pub async fn get_state_at_sequence(
        &self,
        aggregate_id: &Uuid,
        sequence: u32,
    ) -> Result<AggregateState> {
        let events = self.get_events(Query {
            aggregate_id: Some(*aggregate_id),
            until_sequence: Some(sequence),
            ..Default::default()
        }).await?;

        let mut state = AggregateState::default();
        for event in events {
            state.apply(&event);
        }

        Ok(state)
    }
}
```

#### Query API

```protobuf
message TemporalQuery {
  string domain = 1;
  Uuid root = 2;

  oneof point_in_time {
    google.protobuf.Timestamp as_of_time = 3;
    uint32 as_of_sequence = 4;
  }
}
```

#### With Snapshots

For performance, combine with snapshots:

```rust
pub async fn get_state_at(
    &self,
    aggregate_id: &Uuid,
    as_of: DateTime<Utc>,
) -> Result<AggregateState> {
    // Find latest snapshot before target time
    let snapshot = self.snapshot_store
        .get_latest_before(aggregate_id, as_of)
        .await?;

    let (mut state, start_sequence) = match snapshot {
        Some(s) => (s.state, s.sequence + 1),
        None => (AggregateState::default(), 0),
    };

    // Replay events from snapshot to target time
    let events = self.get_events(Query {
        aggregate_id: Some(*aggregate_id),
        from_sequence: Some(start_sequence),
        until_timestamp: Some(as_of),
        ..Default::default()
    }).await?;

    for event in events {
        state.apply(&event);
    }

    Ok(state)
}
```

#### Temporal Projections

For frequently-queried historical data, build temporal projections:

```sql
-- Daily balance snapshots for reporting
CREATE TABLE account_balance_history (
    account_id  UUID NOT NULL,
    as_of_date  DATE NOT NULL,
    balance     DECIMAL(18, 2) NOT NULL,
    PRIMARY KEY (account_id, as_of_date)
);
```

Projector populates at end of each day or on-demand.

---

## Next Steps

- [Sagas](sagas.md) — Cross-aggregate workflows and compensation
- [Projectors](projectors.md) — Building read models from event streams
- [Command Handlers](command-handlers.md) — Processing commands and emitting events
