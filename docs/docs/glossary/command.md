---
id: command
title: Command
hoverText: A request to perform an action. Named imperatively (CreateOrder, ReserveStock). May be rejected.
---

# Command

A request to perform an action. Commands express intent and **may be rejected** if business rules are violated.

## Naming Convention

Commands use **imperative** naming - they tell the system what to do:
- `CreateOrder`
- `ReserveStock`
- `RegisterPlayer`
- `DepositFunds`

## Command vs Event

| Aspect | Command | Event |
|--------|---------|-------|
| Tense | Imperative (do this) | Past (this happened) |
| Rejection | Can be rejected | Cannot be rejected |
| Validation | Validated by aggregate | Already validated |
| Example | `CreateOrder` | `OrderCreated` |

## In Angzarr

Commands are wrapped in a [CommandBook](/glossary/command-book) containing:
- The command payload
- Expected sequence number (for optimistic concurrency)
- Merge strategy (how to handle conflicts)
- Saga origin (for compensation tracking)

## Commands vs Facts

Not all messages to aggregates are rejectable requests. External realities (payments processed, packages delivered) are **facts** that must be recorded, not decided upon.

Angzarr distinguishes these by allowing [fact events](/components/commands-vs-facts) — events sent directly to aggregates with idempotency keys instead of sequence numbers. This preserves semantic clarity: commands request decisions, facts notify of external reality.
