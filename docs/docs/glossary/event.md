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
