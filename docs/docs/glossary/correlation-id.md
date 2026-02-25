---
id: correlation-id
title: Correlation ID
hoverText: Identifier for cross-domain business processes, stable across all events in a workflow.
---

# Correlation ID

Identifies a cross-domain business process. Not a domain entity ID (like `order_id` or `game_id`) - it's the identifier for the workflow/transaction that spans domains. Stable across all events in that process.

**Propagation rules:**
- Client must provide correlation_id on the initial command if cross-domain tracking is needed
- Framework does NOT auto-generate correlation_id - if not provided, it stays empty
- Once set, angzarr propagates correlation_id through sagas, PMs, and resulting commands
- Process managers require correlation_id - events without one are skipped
