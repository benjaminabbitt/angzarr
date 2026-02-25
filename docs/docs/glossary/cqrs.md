---
id: cqrs
title: CQRS
hoverText: Command Query Responsibility Segregation - separating read and write operations into different models.
---

# CQRS

Command Query Responsibility Segregation. Pattern separating read and write operations into different models optimized for their specific use cases.

In Angzarr:
- **Commands** go to aggregates (write side)
- **Queries** go to projections (read side)
- Projectors build read-optimized views from event streams
