---
id: domain
title: Domain
hoverText: An aggregate namespace and deployment unit in Angzarr. Not equivalent to DDD's bounded context.
---

# Domain

**In Angzarr:** A namespace for a single aggregate type. Each domain is a deployment unit with its own event stream, identified by name (e.g., `order`, `inventory`, `player`). Events and commands are namespaced by domain (`order:OrderCreated`).

## Not a Bounded Context

Angzarr's "domain" is **not** equivalent to DDD's "bounded context." This is a critical distinction:

| Concept | Angzarr Domain | DDD Bounded Context |
|---------|----------------|---------------------|
| **Scope** | Single aggregate type | Multiple aggregates sharing language |
| **Purpose** | Namespace, deployment unit, routing | Semantic/linguistic boundary, team ownership |
| **Relationship** | One domain = one aggregate | One context = many aggregates |
| **Modeled by** | Angzarr directly | Organizational, not directly modeled |

**The ownership model:**

```
Team ──1:many(discouraged)──→ Bounded Context ──1:many──→ Domains ──1:1──→ Aggregates
```

- **Team → Bounded Context**: 1:many possible, but discouraged—language shifts between contexts cause confusion
- **Bounded Context → Domains**: Each context owns 1..many Angzarr domains
- **Domain → Aggregate**: Each domain contains exactly one aggregate type

Domain and bounded context are different concepts:

| | Domain | Bounded Context |
|---|--------|-----------------|
| **Type** | Infrastructure boundary | Organizational boundary |
| **Scope** | One aggregate | Many aggregates (many domains) |
| **Defines** | Deployment, routing, event streams | Team ownership, ubiquitous language |

**Example:** In a poker system, a "Game Operations" team might own:
- `player` domain (Player aggregate)
- `table` domain (Table aggregate)
- `hand` domain (Hand aggregate)

All three domains belong to one bounded context because one team owns them with shared language.

**Why multiple domains per team is common:**
- Aggregates split for scaling/deployment, not semantic reasons
- Anemic aggregates—intentionally simple (like Angzarr's demo examples) or accidentally under-designed. Anemic aggregates multiply because decision logic lives elsewhere.
- Early system evolution before aggregates absorb the decisions they should own

## Implications for Sagas

Because Angzarr domains aren't bounded contexts:

- **Sagas within a bounded context**: Internal coordination. The aggregates share ubiquitous language—no translation needed. These are *not* ACLs.
- **Sagas crossing bounded contexts**: True ACLs. They translate between different ubiquitous languages. A thick translation layer here signals modeling problems.

The saga's *code* looks the same either way. The difference is semantic—whether translation is happening or just routing.

## Why This Design?

Angzarr models what it can enforce: aggregate boundaries, event streams, deployment units. It deliberately does *not* model bounded contexts because:

1. Bounded contexts are organizational/team constructs
2. They're defined by shared language, not code structure
3. They change as organizations evolve
4. Enforcing them in infrastructure would be overly rigid

Teams should track which Angzarr domains belong to which bounded contexts via infrastructure tagging:

```yaml
# K8s labels on domain deployments
labels:
  angzarr.io/bounded-context: "game-ops"
  angzarr.io/domain: "player"

# On sagas, also indicate type
labels:
  angzarr.io/saga-type: "acl"      # crosses BC boundary
  # or
  angzarr.io/saga-type: "internal" # within BC
```

This makes bounded context membership queryable and enables policy enforcement without framework-level modeling.

## References

- Martin Fowler, "[BoundedContext](https://martinfowler.com/bliki/BoundedContext.html)"
- [Bounded Context glossary entry](/docs/glossary/bounded-context) for the DDD concept
