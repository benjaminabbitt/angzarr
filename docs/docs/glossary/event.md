---
id: event
title: Event
hoverText: An immutable fact that something happened. Past tense naming (OrderCreated). Cannot be rejected.
---

# Event

An immutable fact that something has occurred. Events represent state changes that **have already happened** and cannot be rejected.

## Naming Convention

Events use **past tense** naming - they describe what happened:
- `OrderCreated`
- `StockReserved`
- `PlayerRegistered`
- `FundsDeposited`

## Event vs Command vs Notification

| Aspect | Event | Command | Notification |
|--------|-------|---------|--------------|
| Tense | Past | Imperative | Present |
| Persisted | Yes | No (transient) | No (transient) |
| Sequenced | Yes | Yes (expected) | No |
| Rejectable | No | Yes | N/A |
| Example | `OrderCreated` | `CreateOrder` | `CompensationRequired` |

## In Angzarr

Events are wrapped in an [EventPage](/glossary/event-page) containing:
- Sequence number (position in aggregate timeline)
- Timestamp
- Payload (or PayloadReference for large events)

Events are stored in the [Event Store](/glossary/event-store) and are the source of truth for aggregate state.

## Internal vs External Events

Events can originate from:
- **Internal decisions**: Aggregate processed a command and decided to emit events
- **External facts**: Saga or external system reporting something that already happened

External facts use a [FactSequence](/components/commands-vs-facts) marker instead of a sequence number. The idempotency key (`Cover.external_id`) propagates through the entire system, enabling:
- Deduplication at every coordinator
- Full traceability from external system to all downstream effects
- Consistent handling across sagas, process managers, and projectors
