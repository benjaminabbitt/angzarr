---
id: cascade-id
title: Cascade ID
hoverText: Identifier grouping events in a two-phase commit transaction. Scope is a single atomic operation.
---

# Cascade ID

Identifies a two-phase commit transaction boundary. Groups uncommitted events that should commit or rollback together.

**Key distinction from correlation_id:**

| ID | Question Answered | Scope |
|----|-------------------|-------|
| `correlation_id` | What business workflow does this belong to? | Entire workflow lifecycle |
| `cascade_id` | What should commit/rollback together? | Single 2PC attempt |

**Many cascades can belong to one correlation:**
- PM retry: same `correlation_id`, new `cascade_id` per attempt
- Multi-step workflow: same `correlation_id`, `cascade_id` per atomic phase

**Behavior:**
- Presence of `cascade_id` triggers `committed=false` writes
- Framework generates `cascade_id` for atomic execution
- Events with same `cascade_id` are confirmed or revoked together
- Empty `cascade_id` means standard committed-immediately semantics

**Query patterns:**
- PM state queries use `correlation_id` (complete workflow view)
- Commit/rollback operates on `cascade_id` (transaction boundary)
