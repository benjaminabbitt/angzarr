---
sidebar_position: 2
---

# Commands vs Facts

This document explores a fundamental tension in event-sourced systems: the difference between **commands** (requests that can be rejected) and **facts** (external realities that must be recorded).

---

## The Problem

In traditional CQRS/ES, the flow is clear:

```
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

```
decide: Command → State → Event list    # Decision logic
evolve: State → Event → State           # State transitions
```

Some implementations extend this with a separate path for facts:

```
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

| Message Type | Sequence Field | Validation | Concurrency |
|--------------|---------------|------------|-------------|
| Command | Expected sequence (integer) | Full business rules | Optimistic locking |
| Fact Event | Fact indicator (oneof) | Idempotency only | Append-only |

When the aggregate coordinator receives a fact event:
1. **No business validation** — the fact already happened
2. **Idempotency check** — prevent duplicate recording (using external ID)
3. **Direct append** — no optimistic concurrency conflict possible
4. **State transition** — the `evolve` function updates aggregate state

### Protocol Structure

The sequence field uses a `oneof` to distinguish:

```protobuf
message EventPage {
  oneof sequence_type {
    uint64 sequence = 1;           // Normal: position in stream
    FactSequence fact = 2;         // Fact: external reality marker
  }
  // ... rest of event
}

message FactSequence {
  string external_id = 1;          // Idempotency key (e.g., Stripe payment ID)
  string source = 2;               // Origin system identifier
}
```

### Flow Comparison

**Traditional command flow:**
```
Saga receives: OrderCompleted
Saga emits:    RecordPayment command (sequence=5)
Aggregate:     Validate → Accept/Reject → Emit PaymentRecorded
```

**Angzarr fact flow:**
```
Saga receives: OrderCompleted
Saga emits:    PaymentRecorded event (fact={external_id: "pi_xxx"})
Aggregate:     Idempotency check → Append → Update state
```

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

### Use Fact Events (with fact sequence) when:

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

## Saga Implementation

Sagas emitting facts use the event path:

```rust
// Traditional: emit command
fn execute(&self, event: OrderCompleted, dest: &EventBook) -> CommandBook {
    CommandBook::new(
        RecordPayment { order_id: event.order_id, amount: event.total },
        dest.next_sequence(),  // Optimistic concurrency
    )
}

// Angzarr: emit fact event
fn execute(&self, event: OrderCompleted, dest: &EventBook) -> EventBook {
    EventBook::fact(
        PaymentRecorded { order_id: event.order_id, amount: event.total },
        FactSequence {
            external_id: event.payment_id,  // Stripe ID for idempotency
            source: "stripe",
        },
    )
}
```

The aggregate coordinator handles fact events differently:
- Checks idempotency via `external_id`
- Skips business validation (fact already happened)
- Appends directly to event stream

---

## Related Concepts

- [Command](/glossary/command) — Requests that may be rejected
- [Event](/glossary/event) — Immutable facts (internal or external)
- [Notification](/glossary/notification) — Transient signals (not persisted)
- [Saga](/glossary/saga) — Domain bridges that may emit commands or facts
- [Sequence](/glossary/sequence) — Optimistic concurrency for commands

---

## Further Reading

- [Functional Event Sourcing Decider](https://thinkbeforecoding.com/post/2021/12/17/functional-event-sourcing-decider) — Jérémie Chassaing
- [Internal and External Events](https://event-driven.io/en/internal_external_events/) — Oskar Dudycz
- [Commands & Events: What's the difference?](https://codeopinion.com/commands-events-whats-the-difference/) — CodeOpinion
- [Martin Fowler - Event Sourcing](https://martinfowler.com/eaaDev/EventSourcing.html)
- [Anti-Corruption Layer](https://learn.microsoft.com/en-us/azure/architecture/patterns/anti-corruption-layer) — Azure Architecture
