<!-- SCM:BEGIN -->
@.scm/context.md
<!-- SCM:END -->

## General Rules
I am not willing to trade speed for correctness.  Correctness is the priority.  Second priority is reducing human reviewer cognitive loading.  

Keep the standalone mode as close to distributed mode as possible, excepting that it runs as a few, limited processes rather than on k8s and uses different bus transports and storage.  The implementations only differ where they absolutely must.  Similarly, keep distributed mode as close to standalone mode as possible.

## Tooling
### Helm
Use helm for all deployments.  Do not use kustomize.

### Python's Role
Python is to be used for support files and general scripting.  Things like manage secrets, initializing a registry, and waiting for grpc health checks.  The author prefers python for this role over shell.

### Skaffold
Use skaffold for all deployments. (this uses helm under the hood)

## Examples Projects
Examples for many common languages are provided.  This should encompass the vast majority of general purpose software development.

Each example directory should be largely self sufficient and know how to build and deploy itself.  A few exceptions:
1) They'll all require the angzarr base binaries/images.  They're implementing an angzarr application.
2) The gherkin files themselves are in the examples directory.  They are kept out of the language specific directories because they are applicable to all languages and should be kept DRY.  They're business speak.

## Port Standards

Angzarr uses consistent port numbering across all deployment modes.

### Infrastructure Ports
- **Gateway gRPC**: 9084 (the "angzarr port" - NodePort: 30084)
- **Gateway Monitoring**: 9085 (angzarr + 1)
- **Stream gRPC**: 1340 (NodePort: 31340)
- **Aggregate Coordinator**: 1310 (NodePort: 31310)
- **Topology REST API**: 9099

### Business Logic Ports by Language
Each language gets a 100-port block for business logic:
- **Rust**: 500xx (order: 50035, inventory: 50025, fulfillment: 50055)
- **Go**: 502xx (order: 50203, inventory: 50204, fulfillment: 50205)
- **Python**: 503xx (order: 50303, inventory: 50304, fulfillment: 50305)

### K8s Testing
For acceptance tests against a deployed cluster:
```bash
# Set up port forward for gateway (one-time, leave running)
kubectl port-forward svc/angzarr-gateway 9084:9084 -n angzarr &

# Run acceptance tests against gateway
ANGZARR_TEST_MODE=gateway ANGZARR_ENDPOINT=http://localhost:9084 cargo test --package e2e --test acceptance
```

The gateway is the single entry point for all client commands. No need to port-forward individual aggregates.

## Testing

Three levels of testing:

### Unit Tests

No external dependencies. Tests interact only with the system under test — no I/O, no concurrency, no infrastructure. Mock prior state where needed (e.g. `EventBook`). Direct invocation of domain logic.

- Angzarr core: inline `#[cfg(test)]` modules
- Examples: `test_*_logic.py`, `*_test.go`, inline Rust tests

### Integration Tests

Test angzarr **framework internals** — the machinery, not business logic. Prove the plumbing works using synthetic aggregates (`EchoAggregate`, `MultiEventAggregate`), not real domains.

What they cover:
- Event persistence and sequence numbering
- IPC event bus (named pipes, domain filtering)
- gRPC over UDS transport
- Channel bus pub/sub delivery
- Saga activation and cross-domain command routing
- Snapshot/recovery, lossy bus resilience, topology tracking

Uses `RuntimeBuilder` in-process with real SQLite, real channels, real named pipes.

- Location: `tests/standalone_integration/`
- Scope: Angzarr framework only, not example business domains

### Acceptance Tests

Test **business behavior** through the full stack. Written in Gherkin, describing what the system does from a business perspective. Exercise real domain logic (cart, order, customer, inventory, fulfillment) through sagas, process managers, and projectors.

- Location: `examples/rust/e2e/tests/` (runner), `examples/rust/e2e/tests/features/` (Gherkin)
- Same Gherkin feature files validate all language implementations (Rust, Python, Go)
- Two execution modes via `Backend` trait abstraction:
  - **Standalone** (default): in-process `RuntimeBuilder` with SQLite
  - **Gateway** (`ANGZARR_TEST_MODE=gateway`): remote gRPC against deployed system

## Proto
When using proto generated code, use extension traits to add functionality to the generated code.  Do not use free functions or explicit wrappers.

## Coordinators
### Aggregates
Business logic is implemented in aggregates.  Accept commands, emit events.

### Sagas
Orchestrate multiple aggregates.  Accept events from a single domain, emit commands in a different domain.  Single domains in and out.  There may be multiple sagas per aggregate, bridging to different domains.

Name sagas `saga-{source}-{target}`. Examples:
- `saga-order-fulfillment` (order events → fulfillment commands)
- `saga-fulfillment-inventory` (fulfillment events → inventory commands)

### Projectors
Accept events from a single domain, output to external systems, databases, event streams to external systems, files, etc.  May query other domain projections to enhance output.

Name projectors `projector-{source}-{feature}`. Examples:
- `projector-inventory-stock` (inventory events → stock level read model)
- `projector-order-web` (order events → web API cache)

### Process Managers
Accepts events across multiple domains, joins them together via the correlation ID. May emit commands to other domains.  Super sagas/aggregates.  These should generally be a state machine correlating events from multiple domains.  Are their own aggregate as well, with the domain being the correlation ID as root.

## Component Descriptors

Components self-describe via `ComponentDescriptor`:
- `name`: Unique identifier (follows naming conventions below)
- `component_type`: aggregate, saga, projector, process_manager
- `inputs`: Domains/event types consumed (subscriptions)
- `outputs`: Domains/command types produced

### Auto-Derivation

Event and command types are auto-derived from router registrations—never manually configured:

```rust
// Saga: .on() registers input event types, .sends() registers output command types
EventRouter::new("saga-order-fulfillment", "order")
    .sends("fulfillment", "CreateShipment")  // output: fulfillment domain, CreateShipment command
    .on("OrderCompleted", handle_completed)  // input: order domain, OrderCompleted event

// Aggregate: .on() registers handled command types
CommandRouter::new("inventory")
    .on("InitializeStock", handle_init)      // input: inventory domain, InitializeStock command
    .on("ReserveStock", handle_reserve)
```

The descriptor is built from these registrations—no separate configuration needed.

### Target Type

A unified `Target` message represents both inputs and outputs:
```protobuf
message Target {
  string domain = 1;      // Domain name
  repeated string types = 2;  // Event or command type names
}
```

- For inputs: domain I subscribe to, event types I consume
- For outputs: domain I send to, command types I emit

## Topology

The topology graph is built **declaratively from descriptors**, not from runtime observation:

- **Nodes**: Created when components register their descriptors
- **Edges**: Created from descriptor inputs (subscriptions) and outputs (commands)
- **Metrics**: Updated from event observation (counts, last seen)

Graph structure comes only from descriptors. Runtime events update metrics on existing edges but never create new ones.

### Visualization

Topology serves a REST API for Grafana Node Graph panel:
- Nodes show: component name, type, event count, last event
- Edges show: source→target, event/command types, throughput

## Glossary

### Component Types
- **Aggregate (agg)**: Domain logic. Commands in, events out. Single domain. The source of truth.
- **Saga (sag)**: Domain bridge. Events from one domain in, commands to another domain out. Stateless translation.
- **Projector (prj)**: Read model builder. Events in, external output (DB, API, files). Query-optimized views.
- **Process Manager (pmg)**: Multi-domain orchestrator. Events from multiple domains in, commands out. Stateful correlation via correlation ID.

### Core Concepts
- **Domain**: A bounded context representing a distinct business capability. Contains aggregates with cohesive behavior. Events/commands are namespaced by domain (e.g., `order`, `inventory`, `fulfillment`). Each domain owns its data and logic—cross-domain communication happens only via events and commands through sagas.

### Angzarr
- **Coordinator**: The angzarr support coordinator that abstracts functionality away from business logic code. Deployed as sidecar container with business logic. Thin wrapper around library code reused in standalone mode.
- **Events**: Domain-specific facts that have occurred. Immutable. Named in past tense (OrderCreated, StockReserved).
- **Commands**: Requests to perform actions. Named imperatively (CreateOrder, ReserveStock).
- **Descriptor**: Self-description of a component's inputs, outputs, and type. Published to event bus for topology discovery.
- **Target**: A domain + list of message types. Used for both subscriptions (inputs) and command destinations (outputs).
- **Correlation ID**: Links related events across domains. Flows through sagas/PMs to trace business transactions. 

## Crate Organization
- Each saga is its own crate with focused, single-purpose translation logic
- Each projector in its own crate with focused, single-purpose output logic
- Each aggregate in its own crate with focused, single-purpose business logic
- Each process manager in its own crate with a minimal bit of functionality that orchestrates cross-domain logic.  Used very sparingly.
- Never combine multiple source domain handlers in one crate deployed with env var switching
- More, smaller pieces over fewer, larger ones
- Aggregates, sagas, and projectors for the same domain are separate crates

## Common Pitfalls

### Naming Collisions
Component names must be globally unique across all component types. A saga named "order-fulfillment" will collide with a process manager named "order-fulfillment" in the topology graph.

**Wrong:**
- Saga: "order-fulfillment", PM: "order-fulfillment" → collision
- Projector: "inventory", Aggregate: "inventory" → collision

**Correct:**
- Saga: "saga-order-fulfillment", PM: "order-fulfillment"
- Projector: "projector-inventory", Aggregate: "inventory"

### Descriptor Publishing
All coordinator binaries must publish their component's descriptor to the event bus for topology discovery. If a component doesn't appear in the topology, check:
1. Does the coordinator call `publish_descriptors()`?
2. Does the business logic implement `GetDescriptor` RPC?
3. Is the descriptor name unique?

### Proto Field Unification
When proto messages share structure, unify them. Separate `Subscription` (event types) and `CommandTarget` (command types) were unified into a single `Target` with a `types` field. Duplication leads to:
- Inconsistent APIs
- Double maintenance burden
- Confusion about which type to use where