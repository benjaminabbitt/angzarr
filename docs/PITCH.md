# ⍼ Angzarr

**A schema-first CQRS/ES framework for polyglot business logic**

The symbol ⍼ ([U+237C](https://en.wikipedia.org/wiki/Angzarr)) has existed in Unicode since 2002 with no defined purpose. The right angle represents the origin point—your event store. The zigzag arrow represents events cascading through your system. We gave it meaning.

---

## The Problem

CQRS and Event Sourcing deliver real architectural benefits: full audit history, temporal queries, independent read/write scaling, and natural alignment with domain-driven design. The implementation cost, however, remains steep.

Teams adopting CQRS/ES face a consistent set of challenges:

- **Infrastructure gravity**: Event stores, message buses, projection databases, and their failure modes dominate early development cycles. Business logic becomes entangled with persistence concerns.
- **Schema management**: Events are append-only and permanent. Schema evolution—adding fields, deprecating event types, maintaining backward compatibility across years of stored events—requires discipline that frameworks rarely enforce.
- **Operational complexity**: Snapshotting, projection rebuilds, idempotency, exactly-once delivery, and saga coordination demand specialized knowledge. Each concern leaks into application code.
- **Language lock-in**: Most CQRS/ES frameworks assume a single-language ecosystem. Organizations with mixed stacks either maintain parallel implementations or force standardization.

The result: CQRS/ES often remains confined to greenfield projects with dedicated teams, despite its suitability for complex domains.

---

## The Angzarr Approach

Angzarr inverts the typical framework relationship. Rather than providing libraries that applications import, Angzarr provides infrastructure that applications connect to.

**Protocol Buffers define the contract.** Commands, events, queries, and read models are declared in `.proto` files. This schema becomes the source of truth—for serialization, validation, documentation, and cross-service compatibility. Protobuf's established rules for backward-compatible evolution apply directly to your event schema.

**gRPC provides the boundary.** Business logic—aggregates, command handlers, event handlers, saga orchestrators—runs as gRPC services in any supported language. The framework communicates exclusively through generated protobuf messages over gRPC. Your domain code never imports Angzarr libraries; it implements generated service interfaces.

**The framework handles the rest.** Event persistence, command routing, event distribution, snapshot storage, idempotency, and saga state management run within Angzarr's Rust core. Your business logic receives commands with full event history and emits events. Side effects stay on one side of the gRPC boundary.

```
┌─ Pod ────────────────────────────────────────────────────────────────┐
│  ┌──────────────────────┐      ┌──────────────────────────────────┐ │
│  │   Your Aggregate     │ gRPC │      Angzarr Sidecar (~8MB)      │ │
│  │   (BusinessLogic)    │◄────►│  ┌────────────┐ ┌─────────────┐  │ │
│  │                      │      │  │  Business  │ │  Projector  │  │ │
│  │  Pure business logic │      │  │Coordinator │ │ Coordinator │  │ │
│  └──────────────────────┘      │  └────────────┘ └─────────────┘  │ │
│                                │  ┌────────────┐ ┌─────────────┐  │ │
│                                │  │   Event    │ │    Saga     │  │ │
│                                │  │   Query    │ │ Coordinator │  │ │
│                                │  └────────────┘ └─────────────┘  │ │
│                                └───────────────┬──────────────────┘ │
└────────────────────────────────────────────────┼────────────────────┘
                                                 │
┌─ Pod ────────────────────────────────────────┐ │
│  ┌──────────────────────┐      ┌─────────────┴────────────────────┐
│  │     Your Saga        │ gRPC │      Angzarr Sidecar (~8MB)      │
│  │     (Python)         │◄────►│             ...                  │
│  └──────────────────────┘      └───────────────┬──────────────────┘
└────────────────────────────────────────────────┼────────────────────┘
                                                 │
                              ┌──────────────────┴──────────────────┐
                              ▼                                     ▼
                     ┌──────────────┐                      ┌──────────────┐
                     │  Event Store │                      │  Message Bus │
                     │  (Postgres)  │                      │  (RabbitMQ)  │
                     └──────────────┘                      └──────────────┘
```

---

## The Book Metaphor

Angzarr models event-sourced aggregates as books. An **EventBook** contains the complete history of an aggregate root: its identity (the **Cover**), an optional **Snapshot** for efficient replay, and ordered **EventPages** representing individual domain events.

This metaphor provides intuitive semantics: you read a book to understand its history, append pages as events occur, and bookmark your place with snapshots.

```protobuf
// Core identity: domain + aggregate root ID
message Cover {
  string domain = 1;
  UUID root = 2;
}

// Individual event with sequence number and timestamp
message EventPage {
  oneof sequence {
    uint32 num = 1;    // Normal sequenced event
    bool force = 2;    // Force-write (conflict resolution)
  }
  google.protobuf.Timestamp createdAt = 3;
  google.protobuf.Any event = 4;
  bool synchronous = 5;
}

// Point-in-time aggregate state for replay optimization
message Snapshot {
  uint32 sequence = 1;
  google.protobuf.Any state = 2;
}

// Complete aggregate history
message EventBook {
  Cover cover = 1;
  Snapshot snapshot = 2;
  repeated EventPage pages = 3;
}
```

Commands follow the same pattern—a **CommandBook** contains one or more **CommandPages** targeting a single aggregate:

```protobuf
message CommandPage {
  uint32 sequence = 1;
  bool synchronous = 2;
  google.protobuf.Any command = 3;
}

message CommandBook {
  Cover cover = 1;
  repeated CommandPage pages = 2;
}
```

---

## Schema-First Development

Domain events and commands are defined in your own `.proto` files. Angzarr's protocol uses `google.protobuf.Any` to wrap these domain-specific messages, maintaining type information while keeping the framework protocol stable.

```protobuf
// domain/inventory.proto — your domain schema

syntax = "proto3";
package inventory;

// Commands
message CreateProduct {
  string sku = 1;
  string name = 2;
  int32 initial_quantity = 3;
}

message AdjustInventory {
  int32 quantity_delta = 1;
  string reason = 2;
}

// Events
message ProductCreated {
  string sku = 1;
  string name = 2;
  int32 initial_quantity = 3;
  google.protobuf.Timestamp created_at = 4;
}

message InventoryAdjusted {
  int32 previous_quantity = 1;
  int32 new_quantity = 2;
  int32 delta = 3;
  string reason = 4;
}

// Snapshot state
message ProductState {
  string sku = 1;
  string name = 2;
  int32 current_quantity = 3;
}
```

Business logic implements the `BusinessLogic` service interface. The framework delivers a **ContextualCommand**—the aggregate's EventBook (snapshot + subsequent events) bundled with the CommandBook—and receives an EventBook containing the resulting events:

```protobuf
// Framework protocol — implemented by your aggregate

message ContextualCommand {
  EventBook events = 1;   // Current state: snapshot + events since
  CommandBook command = 2; // Command(s) to process
}

service BusinessLogic {
  rpc Handle(ContextualCommand) returns (EventBook);
}
```

Your aggregate implementation unpacks the Any-wrapped messages, applies domain logic, and returns events:

```go
// Example: Go aggregate implementation

func (s *ProductAggregate) Handle(ctx context.Context, cmd *angzarr.ContextualCommand) (*angzarr.EventBook, error) {
    // Hydrate state from snapshot + events
    state := s.rehydrate(cmd.Events)

    // Process each command
    var newPages []*angzarr.EventPage
    for _, page := range cmd.Command.Pages {
        events, err := s.processCommand(state, page.Command)
        if err != nil {
            return nil, err
        }
        newPages = append(newPages, events...)
    }

    return &angzarr.EventBook{
        Cover: cmd.Events.Cover,
        Pages: newPages,
    }, nil
}

func (s *ProductAggregate) processCommand(state *ProductState, cmd *anypb.Any) ([]*angzarr.EventPage, error) {
    switch cmd.TypeUrl {
    case "type.googleapis.com/inventory.CreateProduct":
        var create inventory.CreateProduct
        if err := cmd.UnmarshalTo(&create); err != nil {
            return nil, err
        }
        // Pure domain logic — no I/O, no framework coupling
        if state != nil {
            return nil, errors.New("product already exists")
        }
        return []*angzarr.EventPage{{
            Sequence: &angzarr.EventPage_Num{Num: 1},
            CreatedAt: timestamppb.Now(),
            Event: mustAny(&inventory.ProductCreated{
                Sku:             create.Sku,
                Name:            create.Name,
                InitialQuantity: create.InitialQuantity,
            }),
        }}, nil

    case "type.googleapis.com/inventory.AdjustInventory":
        // ... handle adjustment
    }
    return nil, errors.New("unknown command type")
}
```

---

## Coordinator Pattern

Angzarr uses coordinators to route messages between external clients and your business logic services. This separation keeps your domain code focused on business rules while the framework handles:

- Event persistence and retrieval
- Optimistic concurrency via sequence numbers
- Snapshot management
- Synchronous vs. asynchronous processing paths

```protobuf
service BusinessCoordinator {
  // Route commands to appropriate BusinessLogic, persist resulting events
  rpc Handle(CommandBook) returns (SynchronousProcessingResponse);

  // Record events directly (for integration/migration scenarios)
  rpc Record(EventBook) returns (SynchronousProcessingResponse);
}

message SynchronousProcessingResponse {
  repeated EventBook books = 1;
  repeated Projection projections = 2;
}
```

The `SynchronousProcessingResponse` enables request-response patterns where callers need immediate confirmation of state changes and resulting projections—useful for APIs that must return updated read models.

---

## Projections

Unlike frameworks that treat projections as an afterthought, Angzarr provides first-class projection infrastructure. Projectors consume EventBooks and produce typed Projection messages:

```protobuf
message Projection {
  Cover cover = 1;           // Source aggregate
  string projector = 2;      // Projector identifier
  uint32 sequence = 3;       // Last processed event sequence
  google.protobuf.Any projection = 4;  // Projected read model
}

service Projector {
  rpc Handle(EventBook) returns (google.protobuf.Empty);      // Async
  rpc HandleSync(EventBook) returns (Projection);              // Sync
}

service ProjectorCoordinator {
  rpc Handle(EventBook) returns (google.protobuf.Empty);      // Fan-out async
  rpc HandleSync(EventBook) returns (Projection);              // Sync with response
}
```

Synchronous projections enable CQRS patterns where commands must return updated read models—the coordinator orchestrates the command → event → projection pipeline and returns results atomically.

---

## Saga Coordination

Long-running business processes span multiple aggregates and external systems. Angzarr's saga coordinator manages state and compensation without requiring your saga logic to handle persistence or delivery guarantees.

Sagas receive EventBooks (triggering events from any aggregate), execute business logic, and can emit commands to other aggregates or external systems:

```protobuf
service Saga {
  rpc Handle(EventBook) returns (google.protobuf.Empty);                    // Async
  rpc HandleSync(EventBook) returns (SynchronousProcessingResponse);        // Sync
}

service SagaCoordinator {
  rpc Handle(EventBook) returns (google.protobuf.Empty);                    // Route to sagas
  rpc HandleSync(EventBook) returns (SynchronousProcessingResponse);        // Sync pipeline
}
```

The `SynchronousProcessingResponse` from saga processing includes both the EventBooks produced by saga-triggered commands and any resulting projections—enabling complex orchestrations to complete within a single request-response cycle when required.

---

## Event Queries

The EventQuery service provides direct access to the event store for replay, debugging, and custom projection rebuilding:

```protobuf
service EventQuery {
  // Retrieve events for a specific aggregate
  rpc GetEvents(Query) returns (stream EventBook);

  // Subscribe to events (live tail with optional historical replay)
  rpc Synchronize(stream Query) returns (stream EventBook);

  // List all aggregate roots (for full replay scenarios)
  rpc GetAggregateRoots(google.protobuf.Empty) returns (stream AggregateRoot);
}

message Query {
  string domain = 1;
  UUID root = 2;
  uint32 lowerBound = 3;  // Sequence range for partial replay
  uint32 upperBound = 4;
}

message AggregateRoot {
  string domain = 1;
  UUID root = 2;
}
```

Streaming responses handle large event histories efficiently. The `Synchronize` RPC enables catch-up subscriptions—replay historical events then seamlessly transition to live updates.

---

## Pluggable Infrastructure

Angzarr's core abstracts storage and messaging behind adapter interfaces. Custom adapters implement a defined trait.

**Event Store Adapters**
- SQLite (local development)
- MongoDB (production)
- *PostgreSQL (planned)*
- *Redis (planned)*

**Message Bus Adapters**
- Direct gRPC (development, simple deployments)
- RabbitMQ (production)
- *Kafka (planned)*

Configuration is declarative:

```yaml
# config.yaml (production)
storage:
  type: mongodb
  path: mongodb://user:pass@mongo:27017
  database: events

bus:
  type: amqp
  url: amqp://user:pass@rabbitmq:5672

business_logic:
  - domain: inventory
    address: localhost:50051

projectors:
  - name: inventory-summary
    address: localhost:50052
    synchronous: true

sagas:
  - name: order-fulfillment
    address: localhost:50053
```

```yaml
# config.yaml (local development)
storage:
  type: sqlite
  path: ./data/events.db

bus:
  type: direct  # gRPC calls, no message broker

business_logic:
  - domain: inventory
    address: localhost:50051
```

---

## Sidecar Deployment Model

Angzarr runs as a sidecar container alongside your business logic. Each pod contains your service and an Angzarr instance communicating over localhost gRPC.

**Security posture:**
- **Minimal attack surface**: ~8MB distroless container with no shell, no package manager, no unnecessary binaries
- **No network exposure**: Sidecar communicates with your service over localhost only; external traffic routes through your service's existing ingress
- **Principle of least privilege**: Sidecar requires only outbound connections to event store and message bus

**Operational characteristics:**
- Horizontal scaling follows your service scaling—no separate capacity planning
- Sidecar failure restarts independently; Kubernetes health checks apply normally
- No shared state between sidecars; coordination happens through event store and bus
- Local gRPC eliminates network latency between your logic and the framework

```yaml
# Deployment with Angzarr sidecar
apiVersion: apps/v1
kind: Deployment
metadata:
  name: inventory-aggregate
spec:
  template:
    spec:
      containers:
        - name: aggregate
          image: your-registry/inventory-aggregate:v1
          ports:
            - containerPort: 50051

        - name: angzarr
          image: ghcr.io/angzarr/angzarr:latest  # ~8MB
          env:
            - name: ANGZARR_SERVICE_ENDPOINT
              value: "localhost:50051"
            - name: ANGZARR_CONFIG
              value: "/etc/angzarr/config.yaml"
          volumeMounts:
            - name: config
              mountPath: /etc/angzarr
          resources:
            requests:
              memory: "32Mi"
              cpu: "50m"
            limits:
              memory: "128Mi"
              cpu: "200m"
          securityContext:
            readOnlyRootFilesystem: true
            runAsNonRoot: true
            allowPrivilegeEscalation: false
            capabilities:
              drop: ["ALL"]
```

For local development, use Kind (Kubernetes in Docker) with SQLite storage:

```bash
# Local dev with Kind cluster
just kind-create
just deploy
```

---

## Why Rust

The framework core is implemented in Rust. This choice is pragmatic, not ideological:

- **Memory safety without runtime cost**: No garbage collection pauses affecting tail latencies. No null pointer exceptions in production. Memory safety guarantees reduce the class of security vulnerabilities possible in the framework itself.
- **Minimal deployment footprint**: Sidecar container images are ~8MB. No runtime dependencies, no JVM, no interpreter. Distroless base images with only the Angzarr binary reduce attack surface to the minimum possible.
- **Predictable performance**: Latency-sensitive paths (command routing, event serialization) benefit from zero-cost abstractions and control over allocation.
- **Strong ecosystem for the domain**: `tonic` (gRPC), `prost` (protobuf), `sqlx`/`mongodb`/`bigtable` drivers, and `tokio` (async runtime) are mature and actively maintained.

Business logic runs in whatever language suits the domain and team. Rust proficiency is not required to use Angzarr.

---

## Trade-offs and Limitations

Angzarr optimizes for a specific architectural style. It is not universally applicable.

**When Angzarr fits well:**
- Domains with complex business rules benefiting from event sourcing's audit and temporal capabilities
- Organizations with polyglot environments or teams with varied language expertise
- Systems requiring independent scaling of command handling and query serving
- Projects where infrastructure lock-in is a concern

**When to consider alternatives:**
- Simple CRUD applications without event sourcing requirements
- Environments where gRPC is impractical (browser-only, constrained embedded systems)
- Teams preferring library-style frameworks with direct code integration
- Monolithic deployments where sidecar overhead is undesirable

**Current limitations:**
- Multi-tenancy patterns are possible but not first-class; tenant isolation is an application concern
- Event upcasting / schema evolution tooling is not yet implemented
- Multi-region event replication is roadmapped but not available

---

## Comparison to Alternatives

| Aspect | ⍼ Angzarr | AWS Lambda + Step Functions | Axon Framework | EventStoreDB |
|--------|-----------|----------------------------|----------------|--------------|
| **Event Store** | Pluggable (SQLite, MongoDB) | DynamoDB (you build) | Axon Server | Native |
| **Schema** | Protobuf-first, enforced | Application-defined | Java classes | JSON/binary |
| **Multi-Language** | Native (gRPC boundary) | Any (containers) | Java primary | Client libraries |
| **Saga Coordination** | Built-in | Step Functions | Built-in | You build |
| **Deployment** | K8s sidecar | Managed | Self-hosted/Cloud | Self-hosted/Cloud |
| **Vendor Lock-in** | None | AWS | Axon ecosystem | EventStore Ltd |
| **Footprint** | ~8MB sidecar | N/A (managed) | JVM | Database |

---

## Getting Started

```bash
# Clone the repository
git clone https://github.com/angzarr/angzarr
cd angzarr

# Build the framework
cargo build --release

# Create local Kubernetes cluster
just kind-create

# Deploy Angzarr + example services
just deploy

# View logs
just k8s-logs

# Send a test command
grpcurl -plaintext -d '{"cover":{"domain":"customer"}}' \
  localhost:50051 angzarr.BusinessCoordinator/Handle
```

See `examples/` for business logic implementations in Go, Python, and Rust.

Source: [https://github.com/angzarr/angzarr](https://github.com/angzarr/angzarr)

---

## License

SSPL-1.0 (Server Side Public License). See [LICENSE](../LICENSE) for details.

---

*Angzarr is under active development. API stability is not guaranteed before 1.0. Production deployment is not recommended at this time.*
