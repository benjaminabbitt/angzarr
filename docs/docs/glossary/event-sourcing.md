---
id: event-sourcing
title: Event Sourcing
hoverText: Pattern where state is derived by replaying events rather than storing current state directly.
---

# Event Sourcing

Pattern where state is derived by replaying a sequence of events rather than storing current state directly. Provides complete audit trail and temporal queries.

Benefits:
- Complete audit history of all changes
- Ability to rebuild state at any point in time
- Natural fit for distributed systems
- Supports temporal queries and debugging
