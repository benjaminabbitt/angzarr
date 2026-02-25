---
id: merge-strategy
title: Merge Strategy
hoverText: Controls how concurrent commands are handled when sequence numbers conflict.
---

# Merge Strategy

Controls how the system handles concurrent commands when sequence numbers don't match expectations. This is Angzarr's approach to optimistic concurrency.

## Strategies

| Strategy | Behavior | Use Case |
|----------|----------|----------|
| `MERGE_COMMUTATIVE` | Allow if mutations don't overlap | Independent field updates |
| `MERGE_STRICT` | Reject on any mismatch | Financial transactions |
| `MERGE_AGGREGATE_HANDLES` | Aggregate decides | Complex business rules |
| `MERGE_MANUAL` | Route to DLQ | Human review needed |

## Default: MERGE_COMMUTATIVE

Most commands use commutative merge. Two concurrent commands succeed if they modify different parts of state:

```
Current state: { balance: 100, name: "Alice" }

Command A (seq 5): SetName("Bob")     → OK, modifies name
Command B (seq 5): Deposit(50)        → OK, modifies balance
```

## MERGE_STRICT

For operations that must be serialized:

```
Current state: { balance: 100 }

Command A (seq 5): Withdraw(50)  → OK, balance = 50
Command B (seq 5): Withdraw(75)  → REJECTED, sequence mismatch
```

## MERGE_AGGREGATE_HANDLES

The aggregate's command handler receives both the command and conflict context, deciding how to proceed.

## MERGE_MANUAL

Routes to [Dead Letter Queue](/glossary/dlq) for human review when automatic resolution isn't appropriate.
