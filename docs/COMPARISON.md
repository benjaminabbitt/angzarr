# ⍼ Angzarr vs Alternatives

A technical comparison for architects evaluating event sourcing infrastructure.

---

## Comparison Matrix

| Capability | ⍼ Angzarr | AWS Lambda + Step Functions | GCP Cloud Run + Workflows | Axon Framework | Kafka + Custom |
|------------|------------|----------------------------|---------------------------|----------------|----------------|
| **Schema** | Protobuf-first, enforced | Application-defined | Application-defined | Java classes | Application-defined |
| **Event Store** | Pluggable (MongoDB, PostgreSQL†, SQLite, Redis†) | DynamoDB/custom | Firestore/custom | Axon Server or custom | Custom |
| **Optimistic Concurrency** | Native (sequence validation) | Manual | Manual | Native | Manual |
| **Snapshots** | Built-in | Manual | Manual | Built-in | Manual |
| **Saga Orchestration** | Built-in | Step Functions | Workflows | Built-in | Manual |
| **Process Managers** | Built-in | Step Functions | Workflows | Built-in | Manual |
| **Projectors** | Built-in (sync/async) | Lambda triggers | Pub/Sub triggers | Built-in | Consumers |
| **Event Streaming** | Built-in (correlation-based) | EventBridge | Pub/Sub | Built-in | Native |
| **Event Upcasting** | Built-in (sidecar service) | Manual | Manual | Built-in | Manual |
| **Temporal Queries** | Built-in (time + sequence) | Manual | Manual | Not built-in | Manual |
| **Language** | Any gRPC language | Any (containers) | Any (containers) | Java (primary) | Any |
| **Deployment** | K8s sidecar (~8MB) or standalone | Managed | Managed | Self-hosted/Cloud | Self-hosted |
| **Vendor Lock-in** | None | AWS | GCP | Axon (partial) | EventStore Ltd | None |
| **Operational Model** | You run K8s | Fully managed | Fully managed | You run or pay | You run or pay | You run |
| **Cost Model** | Compute only | Per invocation | Per invocation | License/Cloud | License/Cloud | Compute only |

†[PostgreSQL](../src/storage/postgres/README.md) and [Redis](../src/storage/redis/README.md) storage backends and the [Kafka](../src/bus/kafka/README.md) event bus are implemented but not yet integration tested. MongoDB, SQLite, and RabbitMQ are the tested backends.

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
gRPC → Angzarr Sidecar → Your Function → Storage (events) → Projectors/Sagas/PMs
```

| Aspect | AWS | Angzarr |
|--------|-----|------------|
| **Event Store** | You build (DynamoDB streams, custom schema) | Built-in with sequence validation (4 backends) |
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
| **Pub/Sub** | Managed, at-least-once | RabbitMQ, Kafka†, Pub/Sub, SNS/SQS (configurable) |
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
| **Language** | Java (primary), Kotlin | Any gRPC language |
| **Aggregate Model** | Annotation-driven, framework manages | Function-based, you manage state |
| **Event Store** | Axon Server or JPA-based | MongoDB, PostgreSQL†, SQLite, Redis† |
| **Deployment** | Axon Server (managed or self-hosted) | K8s sidecars or standalone mode |
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
- Want to use your team's preferred language (any gRPC language works)
- Prefer explicit over magic (no annotations)
- Want infrastructure layer, not full framework
- Need lightweight, function-based model

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
| **Event Log** | Kafka topics | MongoDB/PostgreSQL†/SQLite/Redis† |
| **Optimistic Concurrency** | You build (or accept none) | Native |
| **Aggregate Queries** | You build (DB + Kafka) | Built-in |
| **Snapshots** | You build | Built-in |
| **Sagas** | You build | Built-in |
| **Complexity** | High (Kafka + DB + custom code) | Low (framework handles) |
| **Kafka Expertise** | Required | Not required |

**When to choose Kafka + Custom:**
- Already have Kafka expertise and infrastructure
- Need Kafka-specific features (log compaction, massive scale)
- Building stream processing (not just event sourcing)

**When to choose Angzarr:**
- Want event sourcing patterns built-in (not DIY on top of Kafka)
- Need aggregate-centric model (not just event log)
- Can still use Kafka as the event bus backend if needed

---

### vs Marten (.NET)

| Aspect | Marten | Angzarr |
|--------|--------|------------|
| **Language** | C# / .NET | Any (gRPC boundary) |
| **Database** | PostgreSQL (required) | MongoDB, PostgreSQL†, SQLite, Redis† |
| **Model** | Library (embedded) | Distributed sidecars or standalone |
| **Sagas** | Wolverine integration | Built-in |
| **Projections** | Built-in (inline, async) | Built-in (sync, async) |

**When to choose Marten:**
- .NET shop
- Already using PostgreSQL
- Want embedded library, not distributed

**When to choose Angzarr:**
- Want to use your team's preferred language (not locked to .NET)
- Need distributed deployment
- Want database flexibility

---

## Architectural Decision Guide

### Choose Angzarr if you need:

1. **Language freedom** - Write business logic in any gRPC-supported language; most teams pick one, but the choice is theirs
2. **Infrastructure abstraction** - Swap storage/bus without code changes (4 storage backends, 6 bus implementations)
3. **Built-in ES patterns** - Optimistic concurrency, snapshots, sagas, projectors, process managers, event upcasting
4. **Flexible deployment** - K8s sidecars for distributed, standalone mode for development/simple deployments
5. **Vendor independence** - Run anywhere; supports AWS, GCP, and self-hosted infrastructure
6. **Function-based model** - Like Lambda, but for event sourcing
7. **Temporal queries** - Reconstruct historical state by time or sequence

### Choose managed cloud services (Lambda/Cloud Run) if you need:

1. **Zero operations** - Don't want to run Kubernetes
2. **Pay-per-use** - Low/variable traffic workloads
3. **Cloud-native integration** - Deep AWS/GCP ecosystem usage
4. **Willing to DIY** - Will build ES patterns yourself

### Choose Axon if you need:

1. **Java ecosystem** - All-in on JVM
2. **Full framework** - Want opinionated, annotation-driven approach
3. **Axon Cloud** - Managed infrastructure option

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
                        │          language-agnostic,            │
                        │          K8s-native)                  │
                        │                                       │
                        │                                       │
```

**Angzarr occupies a unique position:** infrastructure-layer abstraction (not full framework), language-agnostic via gRPC (use any language, not locked to one ecosystem), flexible deployment (K8s sidecars or standalone), with pluggable storage and messaging across cloud providers.

---

## Migration Considerations

| From | To Angzarr | Effort |
|------|--------------|--------|
| Monolith | Deploy sidecars, wrap existing logic as gRPC services | Medium |
| Lambda + custom ES | Replace storage/coordination, keep functions | Medium |
| Axon | Rewrite aggregates as functions, migrate events | Medium-High |
| Kafka + custom | Replace custom code with framework, use Kafka as event bus | Medium |

---

## Total Cost of Ownership

| Factor | Lambda/Cloud Run | Axon | Angzarr |
|--------|-----------------|------|------------|
| **Licensing** | None | Commercial options | Open source |
| **Compute** | Per-invocation | Cluster | K8s pods |
| **Storage** | DynamoDB/Firestore | Included or external | Your choice (4 backends) |
| **Operations** | Managed | You or Axon Cloud | You (K8s) |
| **Expertise** | Cloud-specific | Axon + Java | K8s + your languages |
| **Lock-in Cost** | High (cloud) | Medium (ecosystem) | Low (portable) |

---

## Honest Assessment: Angzarr Gaps and Limitations

No framework is perfect. Here's what to consider before adopting.

### Maturity

| Concern | Reality |
|---------|---------|
| **Production deployments** | Limited. Newer project with fewer battle-tested deployments than Axon |
| **Community size** | Small. You won't find Stack Overflow answers or extensive blog posts |
| **Enterprise support** | None. No commercial support option, no SLAs |
| **Documentation** | Incomplete. Examples exist, but gaps in advanced scenarios |

**Mitigation:** Evaluate with a proof-of-concept before committing. Plan for self-support.

### No Managed Option

Unlike Axon Cloud or AWS/GCP managed services:

- **You run infrastructure** - K8s for distributed, or standalone mode for simpler deployments
- **You manage backing services** - Storage and messaging backends
- **You handle upgrades** - No automated platform updates

**Standalone mode** reduces the operational bar significantly for development and simpler use cases (embedded SQLite + in-memory bus, single binary).

**If you need fully managed:** Consider Axon Cloud or cloud-native alternatives.

### Feature Status

| Feature | Status | Notes |
|---------|--------|-------|
| **Event upcasting / schema evolution** | Implemented | External upcaster sidecar via gRPC, configurable per deployment |
| **Automatic snapshotting** | Implemented | Automatic when business logic returns snapshot state; configurable read/write toggles |
| **[PostgreSQL backend](../src/storage/postgres/README.md)** | Implemented (untested) | Full EventStore + SnapshotStore with sea-query; not yet integration tested |
| **SQLite backend** | Implemented | Full EventStore + SnapshotStore with sqlx migrations; standalone and embedded modes |
| **[Redis backend](../src/storage/redis/README.md)** | Implemented (untested) | Full EventStore + SnapshotStore with sorted sets; not yet integration tested |
| **[Kafka event bus](../src/bus/kafka/README.md)** | Implemented (untested) | Consumer groups, SASL auth, SSL/TLS, topic-per-domain; not yet integration tested |
| **AWS SNS/SQS event bus** | Implemented | SNS publish, SQS subscribe, DLQ support, LocalStack testing |
| **Google Pub/Sub event bus** | Implemented | ADC auth, ordering keys, DLQ support |
| **Event streaming** | Implemented | Correlation-based streaming with client disconnect detection |
| **Temporal queries** | Implemented | Point-in-time by timestamp (as_of_time) and by sequence (as_of_sequence) |
| **Deadline management** | Implemented | Timeout scheduler for process managers; emits ProcessTimeout events |
| **Distributed command routing** | Implemented | Gateway with domain-based routing and service discovery (K8s, DNS, env) |
| **Process managers** | Implemented | Event-driven orchestration with two-phase prepare/execute protocol |
| **Standalone mode** | Implemented | Single-process embedded runtime with SQLite + channel bus |
| **Projection replay/reset** | Partial | Synchronize stream can replay events to projectors; no automated state reset |
| **Event store clustering** | Not implemented | Relies on backing store clustering (MongoDB replica sets, etc.) |

**Remaining gaps:** Event store clustering and automated projection state reset. [PostgreSQL](../src/storage/postgres/README.md), [Redis](../src/storage/redis/README.md), and [Kafka](../src/bus/kafka/README.md) adapters are implemented but not yet integration tested — MongoDB, SQLite, and RabbitMQ are the tested backends.

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
3. **Small team, no K8s expertise** - Standalone mode reduces this, but distributed deployment needs K8s
4. **Proven at massive scale required** - Need battle-tested at millions of events/second
5. **Regulatory compliance** - Need vendor certifications (SOC2, HIPAA, etc.)

### Comparison: Remaining Gaps vs Alternatives

| Gap | Axon | Lambda+Custom |
|-----|------|---------------|
| Managed option | Axon Cloud | Native |
| Enterprise support | Yes | AWS/GCP support |
| Production maturity | High | High |
| Projection state reset | Built-in | Manual |
| Event store clustering | Built-in (Axon Server) | DynamoDB Streams |

---

## Summary: Right Tool for the Job

**Angzarr is a good fit when:**
- You want language freedom — use any gRPC-supported language without framework lock-in
- You want infrastructure-layer abstraction over full framework
- You need pluggable storage and messaging across cloud providers
- You can tolerate building on a newer project
- Your scale requirements are moderate (thousands, not millions, of events/second)
- You want vendor independence over managed convenience
- You need standalone mode for development and K8s for production

**Angzarr is a poor fit when:**
- You need battle-tested production maturity today
- You require commercial support and SLAs
- You prefer fully managed services over self-hosted
- You need event store clustering beyond what backing stores provide
