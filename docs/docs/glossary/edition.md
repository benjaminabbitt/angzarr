---
id: edition
title: Edition
hoverText: Identifier for diverged timelines. Enables branching for speculative execution and historical analysis.
---

# Edition

An identifier for diverged timelines. Editions enable branching scenarios like speculative execution, historical analysis, and what-if queries.

## Concepts

**Main timeline:** Empty edition name (`""`)
**Branch:** Named edition diverging from main

## Divergence Types

### Implicit Divergence
Diverges from the first event in the edition:
```protobuf
Edition { name: "what-if-scenario" }
```

### Explicit Divergence
Per-domain divergence points:
```protobuf
Edition {
  name: "historical-branch",
  divergences: [
    { domain: "order", sequence: 100 },
    { domain: "inventory", sequence: 50 }
  ]
}
```

## Use Cases

1. **Speculative execution:** Test command outcomes without committing
2. **What-if analysis:** Explore alternative scenarios
3. **Historical branches:** Point-in-time forks for analysis
4. **Testing:** Isolated test scenarios

## Angzarr-Specific

Editions are an Angzarr-specific concept not found in standard event sourcing. They extend temporal queries with branching capability.

## Cleanup

Edition events can be deleted when no longer needed:
```protobuf
DeleteEditionEvents { domain: "order", edition: "test-branch" }
```
