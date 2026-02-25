---
id: bounded-context
title: Bounded Context
hoverText: A semantic boundary within which a domain model is defined and applicable. In Angzarr, this maps to a Domain.
---

# Bounded Context

A semantic boundary within which a domain model is defined and applicable. Terms have specific, unambiguous meanings within a bounded context.

**In Angzarr:** Bounded contexts map directly to [Domains](/glossary/domain). Each domain:
- Has its own aggregates with cohesive behavior
- Owns its data and logic
- Communicates with other domains only via events and commands through [Sagas](/glossary/saga)

**Example:** In a poker system:
- `player` domain: bankroll, registration, fund management
- `table` domain: seating, buy-ins, game configuration
- `hand` domain: cards, betting rounds, pot distribution

Each domain uses the same word "player" but with different meaning (player aggregate vs player at table vs player in hand).
