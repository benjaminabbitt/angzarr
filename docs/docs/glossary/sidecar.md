---
id: sidecar
title: Sidecar
hoverText: Container deployed alongside business logic that handles cross-cutting concerns like event persistence and routing.
---

# Sidecar

A container deployed alongside business logic that handles cross-cutting concerns. The sidecar pattern separates infrastructure from business code.

## In Angzarr

The [Coordinator](/glossary/coordinator) is deployed as a sidecar container with your business logic. It handles:
- Event persistence and retrieval
- Command routing
- Subscription management
- Health checks
- gRPC communication

## Architecture

```
┌─────────────────────────────────┐
│           Pod                    │
│  ┌─────────────┐ ┌────────────┐ │
│  │  Business   │ │ Coordinator│ │
│  │   Logic     │◄──► (Sidecar) │ │
│  │  (your code)│ │            │ │
│  └─────────────┘ └────────────┘ │
└─────────────────────────────────┘
```

## Benefits

- **Language agnostic:** Business logic in any language
- **Separation of concerns:** Infrastructure code isolated
- **Consistent behavior:** Same coordinator for all components
- **Independent scaling:** Coordinator handles connection pooling
