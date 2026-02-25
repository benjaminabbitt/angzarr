---
id: aggregate
title: Aggregate
hoverText: Domain logic component accepting commands and emitting events. Single domain source of truth.
---

# Aggregate

Domain logic component. Accepts commands, emits events. Single domain. The source of truth for a bounded context.

**Abbreviation:** `agg`

Aggregates are the core building blocks in Angzarr. They:
- Receive commands from clients or other components
- Validate business rules against current state
- Emit events that represent state changes
- Are identified by a root ID within their domain
