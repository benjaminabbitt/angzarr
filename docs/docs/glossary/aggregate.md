---
id: aggregate
title: Aggregate
hoverText: A consistency boundary enforcing invariants within a single transaction. Accepts commands, emits events.
---

# Aggregate

A cluster of domain objects treated as a single unit for data changes. The aggregate is the **consistency boundary**—all invariants within it are enforced in a single transaction.

**Abbreviation:** `agg`

## DDD Definition

Vaughn Vernon defines it: "A properly designed Aggregate is one that can be modified in any way required by the business with its invariants completely consistent within a single transaction."

Key properties:
- **Consistency boundary**: Everything inside must be consistent after each transaction
- **Root entity**: External references only go through the [aggregate root](/glossary/aggregate-root)
- **Transactional unit**: Only one aggregate is modified per transaction

## In Angzarr

Aggregates are the core building blocks:
- Receive commands from clients or other components
- Validate business rules against current state
- May query external systems (APIs, projections) for decision-making—but only *read*, never write
- Emit events that represent state changes
- Are identified by a [root ID](/glossary/aggregate-root) within their [domain](/glossary/domain)

Side effects to external systems belong in [projectors](/glossary/projector), not aggregates.

## Sizing Guidance

Aggregates should be:
- **Large enough** to enforce their invariants without runtime dependencies on other aggregates
- **Small enough** that unrelated updates don't compete for locks

Common anti-patterns:
- **Too small**: Aggregate can't make decisions alone, requires orchestration across aggregates for every operation
- **Too large**: Aggregate becomes a bottleneck, unrelated changes conflict

The test: **Can this aggregate enforce its business rules without calling out?** If it needs synchronous coordination with another aggregate, either the boundary is wrong or the invariant is shared (and should be in one aggregate).

## References

- Vaughn Vernon, "[Effective Aggregate Design](https://www.dddcommunity.org/library/vernon_2011/)"
