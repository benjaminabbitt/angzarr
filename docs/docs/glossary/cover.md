---
id: cover
title: Cover
hoverText: Identity and routing metadata containing domain, root ID, correlation ID, and edition.
---

# Cover

Identity and routing metadata for events and commands. The Cover tells the system where something belongs and how to route it.

## Structure

```protobuf
message Cover {
  string domain = 1;        // Bounded context name
  UUID root = 2;            // Aggregate root identifier
  string correlation_id = 3; // Cross-domain workflow ID
  Edition edition = 4;      // Timeline/branch identifier
}
```

## Fields

| Field | Purpose | Example |
|-------|---------|---------|
| `domain` | Bounded context | `"order"`, `"inventory"` |
| `root` | Aggregate instance | UUID |
| `correlation_id` | Workflow tracking | `"checkout-123"` |
| `edition` | Timeline branching | Main or diverged |

## Addressing Patterns

**By root:** Single aggregate instance
```
Cover { domain: "order", root: <uuid> }
```

**By correlation:** All aggregates in a workflow
```
Cover { domain: "order", correlation_id: "checkout-123" }
```

**Both:** Specific instance within workflow
```
Cover { domain: "order", root: <uuid>, correlation_id: "checkout-123" }
```

## Default Values

When not specified, Angzarr uses defaults:
- **Event ID:** `{domain}:{root_id}:{sequence}`
- **Event source:** `angzarr/{domain}`
