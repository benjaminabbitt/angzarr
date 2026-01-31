#Tem Partnership Opportunities

## Executive Summary

Angzarr is an open-source CQRS/Event Sourcing infrastructure framework built in Rust. It separates infrastructure concerns (event persistence, messaging, saga coordination, snapshot management) from business logic, which runs as external gRPC services in any language.

**Market positioning:** Infrastructure layer for event-sourced systems. Not a library -- a framework that applications connect to. Vendor-neutral, language-agnostic (any gRPC language), Kubernetes-native.

**Current state:** Pre-1.0, active development. Working implementations in Go, Python, and Rust with full BDD specifications. Standalone and Kubernetes deployment modes operational.

---

## Value Proposition

### For Consulting Firms

CQRS/Event Sourcing engagements carry significant implementation risk. Teams get bogged down in infrastructure -- event stores, snapshot optimization, distributed messaging, concurrency control -- before writing a single line of business logic. Engagements run long, budgets overrun, and clients question the architectural choice.

Angzarr changes the engagement model:

**Reduce implementation risk.** The framework handles the infrastructure that causes most project failures. Business logic becomes pure functions: `(state, command) -> events`. Handlers are 20-40 lines of code each, mechanically structured, unit-testable without infrastructure.

**Repeatable engagement pattern:**
1. Senior architects design protobuf schemas (commands, events, read models)
2. Framework deployment is a one-time Helm install
3. Junior developers implement handlers following a clear pattern
4. BDD specifications serve as executable contracts between you and the client

**Staff efficiently.** Schema design requires senior expertise. Handler implementation does not. The same pattern applies to AI code generation -- LLMs produce correct handlers because the contract is schema-defined and the pattern is mechanical.

**No "wrong stack" problem.** Client teams write business logic in their preferred language (Go, Python, Rust). The gRPC boundary means mixed teams interoperate without integration friction.

**Demonstrate value early.** A working domain with command handling, projections, and saga coordination can be stood up in days, not months. The client sees events flowing through the system before the engagement is half complete.

### For Cloud Providers (Google, AWS)

Angzarr's architecture aligns with cloud-native patterns and creates opportunities for platform integration:

**Serverless deployment paths.** The [roadmap](future_cloud_providers.md) includes feature-gated builds for Cloud Run (GCP) and Lambda (AWS). The sidecar model maps naturally to HTTP-triggered handlers. Push-based messaging (Pub/Sub, SQS) replaces pull-based AMQP consumers.

**Native storage/messaging integration.** The adapter architecture supports pluggable backends. Cloud-native adapters (Bigtable, DynamoDB, Pub/Sub, SQS, Kinesis) are bounded implementation tasks against well-defined traits.

**Reference architecture opportunity.** "Event sourcing on [platform]" is a common customer question with no standardized answer. Angzarr provides a reference architecture: schema-first, language-agnostic (any gRPC language), with clear separation between business logic and infrastructure.

**Minimal compute footprint.** ~8MB distroless sidecar containers. No JVM, no managed runtime overhead. Cost-efficient on per-second billing models.

**Service mesh compatibility.** Sidecar deployment model integrates with Istio, Envoy, and platform-native service mesh offerings.

---

## Current Maturity

### What Works Today

- Event persistence with optimistic concurrency (MongoDB, SQLite tested; [PostgreSQL](../src/storage/postgres/README.md), [Redis](../src/storage/redis/README.md) implemented but untested)
- Command handling with aggregate state reconstruction from snapshots + events
- Projector coordination (synchronous and asynchronous)
- Saga coordination with compensation flows
- Process manager support for complex stateful workflows
- Multi-language examples (Go, Python, Rust) with full BDD specifications
- Standalone mode (SQLite + channel bus + Unix domain sockets) for local development
- Kubernetes deployment via Helm charts
- Streaming infrastructure (gateway + stream services for real-time event delivery)
- Temporal queries (as-of-time, as-of-sequence)
- Event upcasting / schema evolution via gRPC sidecar
- Snapshot management with automatic persistence after command handling

### Honest Assessment

This is a pre-1.0 project. It will have teething issues. API stability is not guaranteed. Production deployment carries risk.

What this means for a partnership:

- **The architecture is sound.** The core patterns (sidecar coordination, gRPC boundary, protobuf-first schemas) are proven concepts assembled in a novel way.
- **The implementation needs maturation.** Edge cases in production workloads, performance characterization under load, operational tooling for day-2 concerns.
- **A maturation partner accelerates everyone.** Real-world usage drives the right fixes. A consulting firm deploying to client environments or a cloud provider running integration tests surfaces issues that solo development cannot.

### Roadmap Items (Partnership Acceleration Opportunities)

| Feature | Impact | Partnership Value |
|---------|--------|-------------------|
| Serverless deployment (Cloud Run, Lambda) | Cloud provider integration | High -- direct platform value |
| Kafka event bus adapter | Enterprise messaging | Medium |
| Admin UI and projection management | Operational visibility | Medium |
| Distributed command routing | Horizontal scaling | High -- needed at scale |
| Performance benchmarking | Adoption confidence | High -- removes uncertainty |
| Multi-region event replication | Global deployment | High -- enterprise requirement |

---

## Integration Points

Partners can integrate with Angzarr at well-defined boundaries:

**Storage adapters** -- Implement `EventStore` and `SnapshotStore` traits for your platform's storage (Bigtable, DynamoDB, Spanner, Cosmos DB).

**Messaging adapters** -- Implement `EventBus` trait for your platform's messaging (Pub/Sub, SQS, Kinesis, Event Hubs).

**Transport** -- gRPC boundary with standard protobuf contracts. Business logic services implement a single interface (`BusinessLogic.Handle`).

**Deployment** -- Helm charts and OCI container images. Infrastructure-as-code via OpenTofu modules for backing services.

**Observability** -- Structured tracing via the `tracing` crate. OpenTelemetry integration is straightforward.

---

## Engagement Models

**Co-development** -- Contribute a platform adapter (storage, messaging, serverless runtime) for your ecosystem. Joint development, shared testing, mutual benefit.

**Reference architecture** -- Joint case study deploying Angzarr on your platform. Published reference architecture for event sourcing on [GCP/AWS/Azure]. Conference talks, blog posts, documentation.

**Consulting partnership** -- Certified Angzarr implementation partner. Training materials, engagement playbooks, direct access to framework maintainers for technical support during client deployments.

**Sponsorship** -- Fund specific roadmap items (serverless deployment, Kafka adapter, performance benchmarking). Named sponsor recognition. Priority influence on feature direction.

---

## See Also

- [Technical Pitch](PITCH.md) -- Full architectural overview for technical decision-makers
- [Comparison to Alternatives](COMPARISON.md) -- Competitive positioning and honest gap analysis
- [Getting Started](getting-started.md) -- Hands-on evaluation path
- [Future Cloud Providers](future_cloud_providers.md) -- Detailed serverless deployment roadmap
