---
id: aggregate-root
title: Aggregate Root
hoverText: The entry point entity for an aggregate. All external references go through the root, identified by UUID.
---

# Aggregate Root

The entry point entity for an aggregate. All external references go through the root, identified by UUID.

In Angzarr:
- Called `root` in the Cover structure
- Always a UUID (not a domain-specific ID like `order_id`)
- Combined with domain name forms the unique identity: `{domain}:{root_id}`

**Relationship to DDD:** The aggregate root is the standard DDD concept - a cluster of domain objects treated as a single unit for data changes. External entities can only hold references to the root.
