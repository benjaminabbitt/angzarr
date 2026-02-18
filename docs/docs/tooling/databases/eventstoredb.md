---
sidebar_position: 99
---

# EventStoreDB

EventStoreDB is **not integrated** with ⍼ Angzarr.

---

## Why Not EventStoreDB

While EventStoreDB is a purpose-built event store with strong event sourcing primitives, it overlaps significantly with ⍼ Angzarr's design goals:

| Concern | EventStoreDB | ⍼ Angzarr |
|---------|--------------|-----------|
| **Event storage** | Native | Uses standard databases (PostgreSQL, Redis) |
| **Projections** | Built-in JavaScript | Language-agnostic projectors |
| **Subscriptions** | Proprietary protocol | Standard message buses (AMQP, Kafka) |
| **Language support** | .NET-centric, limited polyglot | Any gRPC language |
| **Distribution** | Cluster mode (commercial) | Kubernetes-native horizontal scaling |

### Redundancy

EventStoreDB provides event sourcing infrastructure—exactly what ⍼ Angzarr provides. Using both creates:

- **Redundant storage layers**: Events stored twice (EventStoreDB + ⍼ Angzarr's persistence)
- **Competing projection systems**: EventStoreDB projections vs. ⍼ Angzarr projectors
- **Subscription conflicts**: EventStoreDB subscriptions vs. message bus routing

### Language Constraints

EventStoreDB's strongest support is .NET. While clients exist for other languages, the ecosystem is .NET-centric. ⍼ Angzarr's gRPC-based approach provides equal support for Rust, Go, Python, Java, C#, and C++.

### Distribution Model

EventStoreDB clustering requires commercial licensing. ⍼ Angzarr achieves horizontal scaling through Kubernetes-native patterns with standard infrastructure (PostgreSQL + RabbitMQ/Kafka).

---

## Alternatives

For event storage, use:

- **[PostgreSQL](./postgres)** — Production default, ACID guarantees
- **[Redis](./redis)** — High-throughput scenarios
- **[immudb](./immudb)** — Immutable audit requirements
