---
id: bounded-context
title: Bounded Context
hoverText: A DDD concept for semantic/team boundaries. Angzarr doesn't model this directly—multiple Angzarr domains may exist within one bounded context.
---

# Bounded Context

A semantic boundary within which a domain model is defined and applicable. Terms have specific, unambiguous meanings within a bounded context. This is a **solution-space** concept defined by shared [ubiquitous language](https://martinfowler.com/bliki/UbiquitousLanguage.html) and team ownership.

## DDD Definition

From Eric Evans: A bounded context "explicitly defines the context within which a model applies... keep the model strictly consistent within these bounds."

Key characteristics:
- **Linguistic boundary**: Same words have consistent meaning within the context
- **Team ownership**: Typically owned by one team
- **Multiple aggregates**: Contains all aggregates that share the ubiquitous language
- **Model consistency**: Internal model is unified, no contradictions

## Not Directly Modeled in Angzarr

Angzarr does **not** model bounded contexts. Instead, Angzarr models [domains](/docs/glossary/domain)—aggregate namespaces and deployment units.

**The relationship:**
- **Team → Bounded Context**: 1:many possible, but **discouraged**. Language shifts between related contexts cause confusion—a team context-switching between "Order" (e-commerce) and "Order" (fulfillment) will make mistakes.
- **Bounded Context → Domains**: 1:many. Each context owns one or more Angzarr domains.
- **Domain → Aggregate**: 1:1. Each domain contains exactly one aggregate type.

```
Team ──1:many(discouraged)──→ Bounded Context ←──1:many──→ Domains ←──1:1──→ Aggregates
```

Prefer one team per bounded context. When a team must own multiple contexts, make the language boundaries explicit and minimize context-switching.

Teams track bounded context membership via K8s labels, not framework enforcement.

**Example:** In a poker system, a "Game Operations" bounded context (one team, shared language) might contain:
- `player` domain (Player aggregate)
- `table` domain (Table aggregate)
- `hand` domain (Hand aggregate)

These are three Angzarr domains within one bounded context.

## Implications

### For Sagas

Sagas connect Angzarr domains. Whether a saga is an ACL depends on whether it crosses bounded context boundaries:

| Saga Type | Crosses BC? | Translation? | Example |
|-----------|-------------|--------------|---------|
| Internal coordination | No | Minimal—shared language | `table` → `hand` within Game Ops |
| Anti-Corruption Layer | Yes | Heavy—different languages | Game Ops → Payments |

The code looks similar; the semantic weight differs. Use infrastructure tagging to distinguish:

```yaml
# Helm values or K8s labels
labels:
  angzarr.io/bounded-context: "game-ops"
  angzarr.io/saga-type: "acl"  # or "internal"
```

This enables:
- Querying all ACLs: `kubectl get pods -l angzarr.io/saga-type=acl`
- Grouping by bounded context in dashboards
- Policy enforcement (e.g., ACLs require additional review)

### For Team Organization

Bounded contexts should align with team ownership. When planning Angzarr deployments:
1. Identify your bounded contexts (team/language boundaries)
2. Map Angzarr domains to contexts
3. Sagas within a context = internal; across contexts = ACLs
4. Thick ACLs suggest misaligned boundaries

## References

- Martin Fowler, "[BoundedContext](https://martinfowler.com/bliki/BoundedContext.html)"
- Eric Evans at DDD Europe 2019, "[Defining Bounded Contexts](https://www.infoq.com/news/2019/06/bounded-context-eric-evans/)"
- [Domain glossary entry](/docs/glossary/domain) for Angzarr's concept
