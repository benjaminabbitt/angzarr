---
id: process-manager
title: Process Manager
hoverText: Multi-domain orchestrator correlating events via correlation ID. Acts as its own aggregate.
---

# Process Manager

Multi-domain orchestrator. Events from multiple domains in, commands out. Stateful correlation via correlation ID.

**Abbreviation:** `pmg`

Process managers are their own aggregate, with the correlation ID as aggregate root. The PM tracks the state of a specific cross-domain business process.

Use a process manager when you need to:
- React to events from multiple domains
- Maintain workflow state across domain boundaries
- Coordinate complex multi-step business processes
