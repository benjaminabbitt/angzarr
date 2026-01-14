# ⍼ Angzarr vs Alternatives

A technical comparison for architects evaluating event sourcing infrastructure.

---

## Comparison Matrix

| Capability | ⍼ Angzarr | AWS Lambda + Step Functions | GCP Cloud Run + Workflows | Axon Framework | EventStoreDB | Kafka + Custom |
|------------|------------|----------------------------|---------------------------|----------------|--------------|----------------|
| **Schema** | Protobuf-first, enforced | Application-defined | Application-defined | Java classes | JSON/binary | Application-defined |
| **Event Store** | Pluggable (SQLite, MongoDB) | DynamoDB/custom | Firestore/custom | Axon Server or custom | Native | Custom |
| **Optimistic Concurrency** | Native (sequence validation) | Manual | Manual | Native | Native | Manual |
| **Snapshots** | Built-in | Manual | Manual | Built-in | Manual (projections) | Manual |
| **Saga Orchestration** | Built-in | Step Functions | Workflows | Built-in | Manual | Manual |
| **Projectors** | Built-in (sync/async) | Lambda triggers | Pub/Sub triggers | Built-in | Projections | Consumers |
| **Multi-Language** | Native gRPC boundary | Any (containers) | Any (containers) | Java (primary) | Client libraries | Any |
| **Deployment** | K8s sidecar (~8MB) | Managed | Managed | Self-hosted/Cloud | Self-hosted/Cloud | Self-hosted |
| **Vendor Lock-in** | None | AWS | GCP | Axon (partial) | EventStore Ltd | None |
| **Operational Model** | You run K8s | Fully managed | Fully managed | You run or pay | You run or pay | You run |
| **Cost Model** | Compute only | Per invocation | Per invocation | License/Cloud | License/Cloud | Compute only |

---

## Detailed Comparisons

### vs AWS Lambda + Step Functions + EventBridge

**AWS Approach:**
```
API Gateway → Lambda → DynamoDB (events) → EventBridge → Lambda (projectors)
                                        → Step Functions (sagas)
```

**Angzarr Approach:**
```
gRPC → Angzarr Sidecar → Your Function → Postgres (events) → Projectors/Sagas
```

| Aspect | AWS | Angzarr |
|--------|-----|------------|
| **Event Store** | You build (DynamoDB streams, custom schema) | Built-in with sequence validation |
| **Optimistic Concurrency** | You implement (conditional writes) | Automatic |
| **Snapshots** | You build | Automatic |
| **Saga Coordination** | Step Functions (state machine DSL) | Code-based (your language) |
| **Event Replay** | You build | Built-in |
| **Aggregate Loading** | You build | Automatic (events + snapshot) |
| **Cold Start** | Yes (Lambda) | No (persistent containers) |
| **Cost at Scale** | Per-invocation adds up | Predictable compute |
| **Portability** | AWS-locked | Any K8s |

**When to choose AWS:**
- Already deep in AWS ecosystem
- Want fully managed, zero operations
- Workload fits Lambda execution model (short-lived, stateless)
- Can afford per-invocation pricing at scale

**When to choose Angzarr:**
- Need portable, vendor-agnostic solution
- Want event sourcing patterns built-in (not DIY)
- Prefer predictable costs over pay-per-invocation
- Need sub-millisecond latency (no cold starts)

---

### vs GCP Cloud Run + Pub/Sub + Workflows

**GCP Approach:**
```
Cloud Endpoints → Cloud Run → Firestore (events) → Pub/Sub → Cloud Run (projectors)
                                                 → Workflows (sagas)
```

| Aspect | GCP | Angzarr |
|--------|-----|------------|
| **Event Store** | Firestore (document model, you design) | Purpose-built with ES semantics |
| **Pub/Sub** | Managed, at-least-once | RabbitMQ/Kafka (configurable) |
| **Workflows** | YAML DSL, managed | Code-based sagas (your language) |
| **Consistency** | Eventual (Pub/Sub) | Configurable sync/async |
| **Replay** | Manual (query Firestore) | Built-in commands |
| **Cost** | Per-request + Pub/Sub + Firestore | Compute only |

**When to choose GCP:**
- GCP-native organization
- Want managed Pub/Sub and Workflows
- Simple event patterns (not complex aggregates)

**When to choose Angzarr:**
- Need event sourcing semantics (aggregates, sequence, replay)
- Want saga logic in code, not YAML
- Multi-cloud or on-prem requirements

---

### vs Axon Framework

**Axon Approach:**
```java
@Aggregate
public class OrderAggregate {
    @CommandHandler
    public void handle(CreateOrderCommand cmd) {
        apply(new OrderCreatedEvent(...));
    }

    @EventSourcingHandler
    public void on(OrderCreatedEvent event) {
        this.orderId = event.getOrderId();
    }
}
```

**Angzarr Approach:**
```python
def handle(self, ctx: ContextualCommand) -> EventBook:
    state = rebuild(ctx.prior_events)
    return EventBook(events=[OrderCreatedEvent(...)])
```

| Aspect | Axon | Angzarr |
|--------|------|------------|
| **Language** | Java (primary), Kotlin | Rust, Go, Python, Java, C# |
| **Aggregate Model** | Annotation-driven, framework manages | Function-based, you manage state |
| **Event Store** | Axon Server or JPA-based | MongoDB, SQLite, Redis |
| **Deployment** | Axon Server (managed or self-hosted) | K8s sidecars (self-hosted) |
| **Saga Model** | Annotation-driven (@SagaEventHandler) | Interface-based (gRPC) |
| **Learning Curve** | Steep (annotations, lifecycle) | Shallow (functions) |
| **Vendor Lock-in** | Axon ecosystem | None |
| **Licensing** | Open source + commercial | Open source |

**When to choose Axon:**
- Java/Kotlin shop
- Want full-featured, opinionated framework
- Prefer annotation-driven development
- Can use Axon Cloud or run Axon Server

**When to choose Angzarr:**
- Polyglot organization
- Prefer explicit over magic (no annotations)
- Want infrastructure layer, not full framework
- Need lightweight, function-based model

---

### vs EventStoreDB

**EventStoreDB Approach:**
```
Your App → EventStoreDB (gRPC/HTTP) → Subscriptions → Your Projector
```

EventStoreDB is a **database**, not a framework. You still build:
- Command handling
- Aggregate loading
- Saga orchestration
- Projection coordination

| Aspect | EventStoreDB | Angzarr |
|--------|--------------|------------|
| **What It Is** | Event store database | Infrastructure framework |
| **Event Store** | Native (purpose-built) | Pluggable (MongoDB, etc.) |
| **Command Handling** | You build | Built-in |
| **Aggregate Loading** | You build | Built-in |
| **Sagas** | You build | Built-in |
| **Projections** | Built-in (JS-based) | Built-in (any language) |
| **Multi-Language** | Client libraries | Native gRPC services |
| **Operational** | Database to run | Application sidecars |

**When to choose EventStoreDB:**
- Want best-in-class event database
- Will build coordination layer yourself
- Need EventStoreDB-specific features (projections DSL)

**When to choose Angzarr:**
- Want infrastructure layer, not just database
- Need saga/projector coordination out of box
- Prefer using existing databases (MongoDB, PostgreSQL)

---

### vs Kafka + Custom Implementation

**Kafka Approach:**
```
Producer → Kafka Topics → Consumer Groups → Your Projectors/Sagas
                ↓
         Custom Event Store (Kafka as log + DB for queries)
```

| Aspect | Kafka + Custom | Angzarr |
|--------|---------------|------------|
| **Event Log** | Kafka topics | MongoDB/SQLite/Redis |
| **Optimistic Concurrency** | You build (or accept none) | Native |
| **Aggregate Queries** | You build (DB + Kafka) | Built-in |
| **Snapshots** | You build | Built-in |
| **Sagas** | You build | Built-in |
| **Complexity** | High (Kafka + DB + custom code) | Low (framework handles) |
| **Kafka Expertise** | Required | Not required |

**When to choose Kafka:**
- Already have Kafka expertise and infrastructure
- Need Kafka-specific features (log compaction, massive scale)
- Building stream processing (not just event sourcing)

**When to choose Angzarr:**
- Want event sourcing without Kafka complexity
- Need aggregate-centric model (not just event log)
- Prefer simpler operational model

---

### vs Marten (.NET)

| Aspect | Marten | Angzarr |
|--------|--------|------------|
| **Language** | C# / .NET | Rust, Go, Python, Java, C# |
| **Database** | PostgreSQL (required) | MongoDB, SQLite, Redis, PostgreSQL |
| **Model** | Library (embedded) | Distributed sidecars |
| **Sagas** | Wolverine integration | Built-in |
| **Projections** | Built-in (inline, async) | Built-in (sync, async) |

**When to choose Marten:**
- .NET shop
- Already using PostgreSQL
- Want embedded library, not distributed

**When to choose Angzarr:**
- Polyglot organization
- Need distributed deployment
- Want database flexibility

---

## Architectural Decision Guide

### Choose Angzarr if you need:

1. **Multi-language business logic** - Teams write in their preferred language
2. **Infrastructure abstraction** - Swap storage/bus without code changes
3. **Built-in ES patterns** - Optimistic concurrency, snapshots, sagas, projectors
4. **Kubernetes-native** - Sidecar pattern, horizontal scaling
5. **Vendor independence** - Run anywhere, no cloud lock-in
6. **Function-based model** - Like Lambda, but for event sourcing

### Choose managed cloud services (Lambda/Cloud Run) if you need:

1. **Zero operations** - Don't want to run Kubernetes
2. **Pay-per-use** - Low/variable traffic workloads
3. **Cloud-native integration** - Deep AWS/GCP ecosystem usage
4. **Willing to DIY** - Will build ES patterns yourself

### Choose Axon if you need:

1. **Java ecosystem** - All-in on JVM
2. **Full framework** - Want opinionated, annotation-driven approach
3. **Axon Cloud** - Managed infrastructure option

### Choose EventStoreDB if you need:

1. **Best event database** - Purpose-built for event storage
2. **Build own coordination** - Will implement sagas/projectors yourself
3. **EventStoreDB features** - Projections DSL, subscriptions

---

## Summary Positioning

```
                    Managed ◀─────────────────────────────▶ Self-Hosted
                        │                                       │
                        │   Lambda/Cloud Run                    │
                        │   (managed, DIY patterns)             │
                        │                                       │
    Full Framework ◀────┼───────────────────────────────────────┼────▶ Infrastructure Only
                        │                                       │
                        │         Axon                          │
                        │    (full framework,                   │
                        │     Java-centric)                     │
                        │                                       │
                        │              Angzarr               │
                        │         (infrastructure layer,        │
                        │          multi-language,              │
                        │          K8s-native)                  │
                        │                                       │
                        │                      EventStoreDB     │
                        │                   (database only,     │
                        │                    build the rest)    │
                        │                                       │
```

**Angzarr occupies a unique position:** infrastructure-layer abstraction (not full framework), multi-language native (not just clients), self-hosted but simple (K8s sidecars, not complex clustering).

---

## Migration Considerations

| From | To Angzarr | Effort |
|------|--------------|--------|
| Monolith | Deploy sidecars, wrap existing logic as gRPC services | Medium |
| Lambda + custom ES | Replace storage/coordination, keep functions | Medium |
| Axon | Rewrite aggregates as functions, migrate events | Medium-High |
| EventStoreDB | Keep or migrate storage, add coordination layer | Medium |
| Kafka + custom | Replace custom code with framework, optional Kafka bus | Medium |

---

## Total Cost of Ownership

| Factor | Lambda/Cloud Run | Axon | EventStoreDB | Angzarr |
|--------|-----------------|------|--------------|------------|
| **Licensing** | None | Commercial options | Commercial options | Open source |
| **Compute** | Per-invocation | Cluster | Cluster | K8s pods |
| **Storage** | DynamoDB/Firestore | Included or external | Included | Your choice |
| **Operations** | Managed | You or Axon Cloud | You or managed | You (K8s) |
| **Expertise** | Cloud-specific | Axon + Java | EventStoreDB | K8s + your languages |
| **Lock-in Cost** | High (cloud) | Medium (ecosystem) | Medium (database) | Low (portable) |

---

## Honest Assessment: Angzarr Gaps and Limitations

No framework is perfect. Here's what Angzarr doesn't do well (yet), and what you should consider before adopting.

### Maturity

| Concern | Reality |
|---------|---------|
| **Production deployments** | Limited. Newer project with fewer battle-tested deployments than Axon or EventStoreDB |
| **Community size** | Small. You won't find Stack Overflow answers or extensive blog posts |
| **Enterprise support** | None. No commercial support option, no SLAs |
| **Documentation** | Incomplete. Examples exist, but gaps in advanced scenarios |

**Mitigation:** Evaluate with a proof-of-concept before committing. Plan for self-support.

### No Managed Option

Unlike Axon Cloud, EventStoreDB Cloud, or AWS/GCP managed services:

- **You run Kubernetes** - No serverless/managed deployment option
- **You manage infrastructure** - MongoDB, RabbitMQ, etc.
- **You handle upgrades** - No automated platform updates

**If you need managed:** Consider Axon Cloud or cloud-native alternatives.

### Missing Features (Roadmap Items)

| Feature | Status | Impact |
|---------|--------|--------|
| **Event upcasting / schema evolution** | Not implemented | Must handle event versioning manually |
| **Automatic snapshotting** | Not implemented | Must trigger snapshots explicitly |
| **PostgreSQL backend** | Planned | Limited to MongoDB/SQLite/Redis for now |
| **Kafka event bus** | Planned | RabbitMQ only for distributed deployments |
| **Projection replay/reset** | Not implemented | Cannot rebuild read models from scratch |
| **Subscription queries** | Not implemented | No live query updates |
| **Deadline management** | Not implemented | No scheduled/delayed commands |
| **Distributed command routing** | Not implemented | No consistent hashing for aggregate affinity |
| **Event store clustering** | Not implemented | Single-node event store (rely on MongoDB clustering) |

**Reality check:** If you need these today, you'll either build them yourself or choose a more mature alternative.

### Operational Complexity

| Aspect | Challenge |
|--------|-----------|
| **Debugging** | Distributed tracing across sidecars requires setup (Jaeger, etc.) |
| **Monitoring** | No built-in dashboards; integrate with Prometheus/Grafana yourself |
| **Alerting** | No opinionated alerting rules provided |
| **Log aggregation** | Structured logging exists, but aggregation is your responsibility |

### Enterprise Features Not Present

| Feature | Status |
|---------|--------|
| **Multi-tenancy** | Not built-in (implement at application layer) |
| **RBAC / Authorization** | Not built-in (implement at gateway/application layer) |
| **Audit logging** | Events are auditable, but no dedicated audit service |
| **Encryption at rest** | Depends on backing service (MongoDB encryption, etc.) |
| **Rate limiting** | Not built-in |

### Performance Unknowns

| Aspect | Status |
|--------|--------|
| **Benchmarks** | Limited published benchmarks |
| **Scale testing** | No documented tests at 10K+ events/second |
| **Latency profiles** | Not formally characterized |

**Recommendation:** Run your own benchmarks for your specific workload.

### When NOT to Choose Angzarr

1. **Need managed/serverless** - No operational burden tolerance
2. **Need enterprise support** - Require vendor SLAs and support contracts
3. **Need features on roadmap today** - Event upcasting, Kafka, PostgreSQL
4. **Small team, no K8s expertise** - Operational overhead too high
5. **Proven at massive scale required** - Need battle-tested at millions of events/second
6. **Regulatory compliance** - Need vendor certifications (SOC2, HIPAA, etc.)

### Comparison: Feature Gaps vs Alternatives

| Gap | Axon | EventStoreDB | Lambda+Custom |
|-----|------|--------------|---------------|
| Event upcasting | Built-in | Manual | Manual |
| Automatic snapshots | Built-in | N/A (projections) | Manual |
| Managed option | Axon Cloud | EventStoreDB Cloud | Native |
| Enterprise support | Yes | Yes | AWS/GCP support |
| Production maturity | High | High | High |

---

## Summary: Right Tool for the Job

**Angzarr is a good fit when:**
- You value polyglot flexibility over ecosystem maturity
- You have Kubernetes expertise and operational capacity
- You prefer infrastructure-layer abstraction over full framework
- You can tolerate building on a newer project
- Your scale requirements are moderate (thousands, not millions, of events/second)
- You want vendor independence over managed convenience

**Angzarr is a poor fit when:**
- You need battle-tested production maturity today
- You require commercial support and SLAs
- You lack Kubernetes operational expertise
- You need specific roadmap features immediately
- You prefer managed services over self-hosted
