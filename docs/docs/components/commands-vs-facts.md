---
sidebar_position: 5
---

# Commands vs Facts

This document explores a fundamental tension in event-sourced systems: the difference between **commands** (requests that can be rejected) and **facts** (external realities that must be recorded).

---

## The Problem

In traditional CQRS/ES, the flow is clear:

```text title="illustrative - command flow"
Client → Command → Aggregate → Accept/Reject → Event
```

Commands express *intent*. The aggregate validates business rules and either:
- **Accepts**: Emits events recording what happened
- **Rejects**: Returns an error

But what happens when the thing has *already happened*? Consider:

| Scenario | Source | Can You Reject It? |
|----------|--------|-------------------|
| Stripe processed a payment | External webhook | No — money moved |
| FedEx delivered the package | External tracking | No — it's delivered |
| Game timer expired | Clock/scheduler | No — time passed |
| Regulatory ruling issued | Legal system | No — it's binding |

These aren't requests. They're **facts about external reality**. The aggregate can't reject them — the world has already changed.

---

## The Terminology Problem

Traditional CQRS conflates two distinct concepts:

| Concept | Traditional Term | Actual Meaning |
|---------|-----------------|----------------|
| Request for action | Command | "Please do X" — rejectable |
| Notification of fact | Command | "X happened" — not rejectable |

When a saga receives `PaymentProcessedByStripe` and needs to inform the Order aggregate, it sends a "command" like `RecordPaymentReceived`. But this isn't really a command — the aggregate can't refuse to acknowledge that payment occurred.

This creates awkward patterns:
- Aggregates with commands that "can never fail"
- Validation logic that always returns success
- Optimistic concurrency that doesn't make sense (sequence numbers guard against concurrent *decisions*, not concurrent *fact recording*)

---

## How Other Frameworks Handle This

### The Decider Pattern (Jérémie Chassaing)

The [Functional Event Sourcing Decider](https://thinkbeforecoding.com/post/2021/12/17/functional-event-sourcing-decider) separates concerns:

```text title="illustrative - decider pattern"
decide: Command → State → Event list    # Decision logic
evolve: State → Event → State           # State transitions
```

Some implementations extend this with a separate path for facts:

```text title="illustrative - fact recording path"
record: Fact → State → Event list       # No validation, just acknowledge
```

### Axon Framework

Axon distinguishes between:
- **Decision commands**: Standard validation, can reject
- **Notification commands**: Minimal validation, expected to succeed

The aggregate handler checks which type and adjusts validation accordingly.

### Martin Fowler's Recording Pattern

From [Event Sourcing](https://martinfowler.com/eaaDev/EventSourcing.html):

> "Turn the interaction into events at the boundary of the system and use the record of events to remember what happened."

External interactions become `*Recorded` events:
- `PaymentReceived` (internal decision)
- `ExternalPaymentRecorded` (external fact)

### Anti-Corruption Layer

Many systems use an [ACL](https://learn.microsoft.com/en-us/azure/architecture/patterns/anti-corruption-layer) to translate external facts into internal domain concepts at the system boundary, before they reach aggregates.

---

## Angzarr's Solution

Angzarr addresses this by allowing sagas and external systems to pass **events** (not commands) to aggregates, distinguished by a **fact sequence indicator**.

### Commands vs Fact Events

| Message Type | PageHeader.sequence_type | Validation | Concurrency |
|--------------|--------------------------|------------|-------------|
| Command (client) | `sequence` (integer) | Full business rules | Optimistic locking |
| Command (saga) | `angzarr_deferred` | Full business rules | Framework-managed |
| Fact (external) | `external_deferred` | Idempotency only | Append-only |

When the aggregate coordinator receives a fact event:
1. **No business validation** — the fact already happened
2. **Idempotency check** — prevent duplicate recording via `PageHeader.external_deferred.external_id`
3. **Direct append** — no optimistic concurrency conflict possible
4. **State transition** — the `evolve` function updates aggregate state

### Protocol Structure

The `PageHeader` uses a `oneof` to distinguish sequence types:

```protobuf file=proto/angzarr/types.proto start=docs:start:page_header end=docs:end:page_header
```

**Key design:** The idempotency key (`external_id`) lives in `PageHeader.external_deferred`, keeping `Cover` focused on aggregate identity while `PageHeader` handles sequencing. The `ExternalDeferredSequence` carries both the idempotency key and a human-readable description. This ensures:
- Consistent deduplication at the coordinator
- Clear provenance tracking for audit trails
- Human-readable context for debugging

### Flow Comparison

**Traditional command flow (saga-to-aggregate):**
```text title="illustrative - traditional command flow"
Saga receives: OrderCompleted
Saga emits:    RecordPayment command (sequence=5)
Aggregate:     Validate → Accept/Reject → Emit PaymentRecorded
```

**Angzarr saga flow (command with deferred sequence):**
```text title="illustrative - Angzarr saga flow"
Saga receives: OrderCompleted
Saga emits:    StartFulfillment command
               - PageHeader.angzarr_deferred (framework-stamped)
Coordinator:   Validate → Accept/Reject → Assign sequence → Persist
```

**Angzarr external fact flow (webhook injection):**
```text title="illustrative - Angzarr fact flow"
Stripe webhook: PaymentReceived event
               - PageHeader.external_deferred.external_id = "pi_xxx"
               - PageHeader.external_deferred.description = "Stripe webhook"
Coordinator:   Check external_id → Assign sequence → Append → Publish
```

### Fact Processing Pipeline

The coordinator handles fact events through a configurable pipeline:

```text title="illustrative - fact processing pipeline"
Fact Event arrives (with ExternalDeferredSequence)
        ↓
Check idempotency (external_deferred.external_id)
        ↓
[If route_to_handler = true]  ←── Default: true
        ↓
Route to aggregate for state update
        ↓
Aggregate returns event
        ↓
Coordinator assigns real sequence number
        ↓
Persist event (sequence assigned in PageHeader)
        ↓
Publish event (with valid sequence)
```

**Key behavior:** The `ExternalDeferredSequence` marker triggers deferred sequence assignment. When the aggregate returns events, the coordinator:

1. Takes the next available sequence number for the aggregate root
2. Replaces `external_deferred` with `sequence` in the `PageHeader`
3. Persists and publishes the event with a valid sequence

Downstream consumers (sagas, projectors, process managers) always receive events with proper sequence numbers. The deferred sequence is purely an ingestion-time marker.

#### Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `route_to_handler` | `true` | When true, fact events are routed to the aggregate for state updates before persistence. When false, facts are persisted directly without aggregate involvement. |

Setting `route_to_handler = true` (the default) allows aggregates to:
- Update their internal state based on the fact
- Emit additional events in response to the fact
- Maintain consistency with their domain model

Setting it to `false` is useful for pure append-only fact logging where aggregate state isn't needed.

### Benefits

1. **Semantic clarity**: Facts are events, not commands. The type system reflects reality.

2. **No fake validation**: Aggregates don't need "commands that can't fail."

3. **Correct concurrency**: Facts don't compete with decisions — they're additive observations.

4. **Idempotency by design**: External IDs naturally deduplicate (Stripe payment ID, tracking number, etc.).

5. **Audit trail accuracy**: Events are labeled as externally-sourced vs internally-decided.

---

## When to Use Each

### Use Commands (with sequence) when:

- The aggregate is making a **decision**
- Business rules can **reject** the request
- Concurrent commands should **conflict** (optimistic concurrency)
- The source is an internal actor with intent

**Examples:**
- `CreateOrder` — validate inventory, customer status
- `ReserveFunds` — check available balance
- `PlaceBet` — validate game state, bet limits

### Use Fact Events (with external_deferred) when:

- Something **already happened** in the external world
- The aggregate must **acknowledge**, not decide
- Multiple notifications of the same fact should **deduplicate**
- The source is an external system or physical reality

**Examples:**
- `PaymentReceived` — Stripe confirmed payment
- `PackageDelivered` — FedEx tracking update
- `GameTimeExpired` — clock exhausted
- `RegulatoryHoldPlaced` — compliance system action

---

## External Fact Injection

External systems inject facts via the `HandleEvent` RPC:

```rust title="illustrative - external fact injection"
// Stripe webhook handler injects payment fact
async fn handle_stripe_webhook(payload: StripeEvent) {
    let event_request = EventRequest {
        events: Some(EventBook {
            cover: Some(Cover {
                domain: "order".into(),
                root: order_id.into(),
                ..Default::default()
            }),
            pages: vec![EventPage {
                header: Some(PageHeader {
                    sequence_type: Some(ExternalDeferred(ExternalDeferredSequence {
                        external_id: payload.payment_intent_id.clone(),  // Stripe ID for idempotency
                        description: "Stripe webhook".into(),
                    })),
                }),
                payload: Some(Event(Any::pack(PaymentReceived {
                    order_id,
                    amount: payload.amount,
                }))),
                ..Default::default()
            }],
            ..Default::default()
        }),
        route_to_handler: true,
        ..Default::default()
    };

    client.handle_event(event_request).await;
}
```

The aggregate coordinator handles fact events differently:
- Checks idempotency via `PageHeader.external_deferred.external_id`
- Skips business validation (fact already happened)
- Assigns sequence number and appends to event stream
- Publishes with assigned sequence to downstream consumers

## Saga-Produced Commands

Sagas translate events between domains. They return `SagaResponse` containing commands (not facts):

```rust title="illustrative - saga producing commands"
impl SagaHandler for OrderFulfillmentSaga {
    async fn handle(&self, source: &EventBook) -> Result<SagaResponse, Status> {
        let mut commands = Vec::new();

        for page in &source.pages {
            if let Some(order_completed) = extract_event::<OrderCompleted>(&page) {
                // Saga produces command for fulfillment domain
                // Framework stamps angzarr_deferred with source info
                commands.push(CommandBook {
                    cover: Some(Cover {
                        domain: "fulfillment".into(),
                        root: order_completed.order_id.into(),
                        ..Default::default()
                    }),
                    pages: vec![CommandPage {
                        header: Some(PageHeader::default()),  // Framework fills angzarr_deferred
                        command: Some(Any::pack(StartFulfillment {
                            order_id: order_completed.order_id,
                            items: order_completed.items.clone(),
                        })),
                        ..Default::default()
                    }],
                    ..Default::default()
                });
            }
        }

        Ok(SagaResponse { commands, events: vec![] })
    }
}
```

The framework stamps `angzarr_deferred` on saga-produced commands with source aggregate info for:
- Provenance tracking (which event triggered this command)
- Compensation routing (rejection flows back to source aggregate)

---

## Related Concepts

- [Command](/glossary/command) — Requests that may be rejected
- [Event](/glossary/event) — Immutable facts (internal or external)
- [Notification](/glossary/notification) — Transient signals (not persisted)
- [Saga](/glossary/saga) — Domain bridges that emit commands (with angzarr_deferred)
- [Sequence](/glossary/sequence) — Optimistic concurrency for commands

---

## Further Reading

- [Functional Event Sourcing Decider](https://thinkbeforecoding.com/post/2021/12/17/functional-event-sourcing-decider) — Jérémie Chassaing
- [Internal and External Events](https://event-driven.io/en/internal_external_events/) — Oskar Dudycz
- [Commands & Events: What's the difference?](https://codeopinion.com/commands-events-whats-the-difference/) — CodeOpinion
- [Martin Fowler - Event Sourcing](https://martinfowler.com/eaaDev/EventSourcing.html)
- [Anti-Corruption Layer](https://learn.microsoft.com/en-us/azure/architecture/patterns/anti-corruption-layer) — Azure Architecture
