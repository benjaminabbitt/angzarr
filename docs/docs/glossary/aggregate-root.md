---
id: aggregate-root
title: Aggregate Root
hoverText: The entry point entity for an aggregate. All external references go through the root, identified by UUID.
---

# Aggregate Root

The entry point entity for an [aggregate](/glossary/aggregate). All external references must go through the root—no direct access to internal entities.

## In Angzarr

- Called `root` in the [Cover](/glossary/cover) structure
- Always a UUID (not a domain-specific ID like `order_id`)
- Combined with domain name forms the unique identity: `{domain}:{root_id}`

**Example:** An order aggregate has root ID `550e8400-e29b-41d4-a716-446655440000`. Its full identity is `order:550e8400-e29b-41d4-a716-446655440000`.

## DDD Relationship

The aggregate root is standard DDD terminology. From Evans:
- A cluster of domain objects treated as a single unit for data changes
- External entities can only hold references to the root
- The root controls access to internals and enforces invariants

The root ID is stable for the lifetime of the aggregate. Events reference the root ID in their [Cover](/glossary/cover), enabling event sourcing and replay.
