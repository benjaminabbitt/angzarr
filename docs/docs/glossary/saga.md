---
id: saga
title: Saga
hoverText: Stateless domain bridge translating events from one domain into commands for another.
---

# Saga

Domain bridge component. Events from one domain in, commands or fact events to another domain out. Stateless translation between bounded contexts.

**Abbreviation:** `sag`

Sagas should contain minimal logic — just enough to map fields and construct target messages. If you need conditionals or business rules, that logic belongs in the source aggregate.

Sagas must be stateless — each event is processed independently with no memory of previous events.

## Output Types

Sagas can emit two types of messages:

| Output | Use When | Example |
|--------|----------|---------|
| **Command** | Target aggregate should decide | `ReserveFunds` — aggregate validates balance |
| **Fact Event** | Reporting external reality | `PaymentRecorded` — Stripe already processed it |

When the source event represents an external fact (payment processed, delivery confirmed), the saga emits a fact event rather than a command. The target aggregate records the fact without decision logic.

See [Commands vs Facts](/components/commands-vs-facts) for the full pattern.
