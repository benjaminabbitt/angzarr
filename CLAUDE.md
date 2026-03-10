## General Rules
I am not willing to trade speed for correctness.  Correctness is the priority.  Second priority is reducing human reviewer cognitive loading.

### Fix What You Find
This is a prototype. Claude is unreliable at finishing work. If you encounter broken things while working (broken links, failing tests, incomplete implementations), don't declare "not my problem" and move on. Instead:
1. Note the issue
2. Check with me about whether to fix it now or later
3. Propose a fix if appropriate

Leave the codebase better than you found it.

Keep the standalone mode as close to distributed mode as possible, excepting that it runs as a few, limited processes rather than on k8s and uses different bus transports and storage.  The implementations only differ where they absolutely must.  Similarly, keep distributed mode as close to standalone mode as possible.

Focus on readability and maintainability. The primary costs are developer and AI time. Priorities:
1. Understanding — minimize cognitive load, avoid foot-guns
2. Architectural correctness — make decisions that reduce future churn and enable ease of change
3. Performance — optimize later when bottlenecks are measured

Plan *everything* and run it by me prior to executing.  If it requires any meaningful decisions, ask.

## Examples
Examples are a cqrs-es system playing poker.
Player domain uses functional aggregates.
All other domains use object oriented.

### Definition of Done
**Nothing is "done" until tests prove it works.** Writing code without runnable tests is incomplete work. This means:
- Implementation code requires corresponding test code
- Tests must actually execute (not just exist as specifications)
- For Gherkin features: step definitions must be implemented and the test runner must pass
- "Tests pass" means running the actual test command, not just writing test files
- Mark todo items as "completed" only after tests run green

### Mutation Testing Validation
**Run mutation testing after EVERY new test is added.** Tests must catch mutations to be meaningful. A test that doesn't kill any mutants is a waste of code.

Workflow:
1. Write a test
2. Run mutation testing on the file
3. Verify the test kills at least one mutant
4. If no mutants killed, the test is not meaningful — improve it or delete it
5. Repeat for each new test

```bash
# Run mutation testing on the modified file
cargo mutants --in-place --timeout 120 -f <file> -- --features "sqlite test-utils"
```

**Exclusions:**
- Do NOT test gRPC/proto generated code (`src/proto/` or `*.pb.rs` files)
- DO test extension traits on proto types (`src/proto_ext/`)
- DO test any hand-written code that uses proto types

Target kill rates:
- Trait contracts (storage, bus, DLQ): 70%+
- Orchestration/framework code: 60%+
- Utility functions: 70%+

**Do not mark work complete until mutation testing validates the tests are meaningful.** 

### Enums
ese enum names, not integer representations, in code

## Tooling
### Helm
Use helm for all deployments.  Do not use kustomize.

### Python's Role
Python is to be used for support files and general scripting.  Things like manage secrets, initializing a registry, and waiting for grpc health checks.  The author prefers python for this role over shell.

### Vector Search (MCP)
A Qdrant-based MCP server provides semantic search over the codebase via `qdrant-find` and `qdrant-store` tools. Backed by local fastembed embeddings (no API key).

```bash
just qdrant-start  # Start Qdrant container (data in .vectors/qdrant-data/)
just reindex       # Rebuild the codebase index
just qdrant-stop   # Stop Qdrant container
```

Rebuild the index after significant changes with `just reindex`.

### Local Kubernetes: Kind
Use Kind for local development. Kind runs Kubernetes nodes as containers.

```bash
# Kind management
just kind status              # Check cluster status
just kind up                  # Create cluster and deploy infrastructure
just kind down                # Delete cluster

# Kubeconfig
export KUBECONFIG=~/.kube/config
```

### Skaffold (CRITICAL)
**ALL image builds and deployments MUST go through skaffold.** Never bypass with manual `podman build` + `helm upgrade` workflows.

Why: Kind nodes cache images by tag at the containerd level. If you push a new image with the same tag (e.g., `:latest`), the node continues serving the old cached image even after `kubectl rollout restart`. Skaffold uses content-addressable tags (git commit SHA), ensuring each build gets a unique tag with no cache collisions.

```bash
# CORRECT - always use skaffold
just deploy                    # Full deployment
just dev                       # Watch mode
skaffold run -f examples/rust/skaffold.yaml

# WRONG - will cause stale image issues
podman build -t ghcr.io/angzarr-io/myimage:latest ...
podman push ghcr.io/angzarr-io/myimage:latest
helm upgrade ...
kubectl set image ...
```

## Examples Projects
Examples for many common languages are provided.  This should encompass the vast majority of general purpose software development.

Each example directory should be largely self sufficient and know how to build and deploy itself.  A few exceptions:
1) They'll all require the angzarr base binaries/images.  They're implementing an angzarr application.
2) The gherkin files themselves are in the examples directory.  They are kept out of the language specific directories because they are applicable to all languages and should be kept DRY.  They're business speak.

## Port Standards

Angzarr uses consistent port numbering across all deployment modes.

### Infrastructure Ports
- **Command Handler Coordinator**: 1310 (NodePort: 31310) - per-domain command handling
- **Stream gRPC**: 1340 (NodePort: 31340) - event streaming
- **Topology REST API**: 9099 - topology visualization

### Business Logic Ports by Language
Each language gets a 100-port block for business logic:
- **Rust**: 500xx (order: 50035, inventory: 50025, fulfillment: 50055)
- **Go**: 502xx (order: 50203, inventory: 50204, fulfillment: 50205)
- **Python**: 503xx (order: 50303, inventory: 50304, fulfillment: 50305)

### K8s Testing
For acceptance tests against a deployed cluster:
```bash
# Set up port forwards for each domain's command handler coordinator
kubectl port-forward svc/angzarr-order 1310:1310 -n angzarr &
kubectl port-forward svc/angzarr-inventory 1310:1310 -n angzarr &

# Run acceptance tests against deployed cluster
ANGZARR_TEST_MODE=direct \
  ANGZARR_ORDER_ENDPOINT=http://localhost:1310 \
  ANGZARR_INVENTORY_ENDPOINT=http://localhost:1311 \
  cargo test --package e2e --test acceptance
```

Clients connect directly to per-domain command handler coordinators via K8s DNS (e.g., `angzarr-order.angzarr.svc.cluster.local:1310`).

## Testing

Four levels of testing:

**Never** write tests that just blank "pass"es

### Unit Tests

No external dependencies. Tests interact only with the system under test. Mock prior state where needed (e.g. `EventBook`). Direct invocation of domain logic.

- Angzarr core: separate `.test.rs` files alongside source
- Examples: `test_*_logic.py`, `*_test.go`, `*.test.rs`
- Run with: `cargo test --lib` or `just test`
- **Execution**: Continuous via bacon, pre-commit hooks

#### Test File Organization (Rust)

**Place tests in `.test.rs` files alongside source files.** This reduces context size when reading production code—AI and human reviewers see implementation without wading through test code.

```
src/
├── correlation.rs           # Production code
├── correlation.test.rs      # Tests for correlation.rs
├── validation.rs
├── validation.test.rs
└── mod.rs                   # Includes both via #[cfg(test)]
```

**Include pattern in parent module:**
```rust
// In mod.rs or lib.rs
pub mod correlation;
#[cfg(test)]
#[path = "correlation.test.rs"]
mod correlation_tests;
```

**Test file structure:**
```rust
// correlation.test.rs
//! Tests for correlation ID propagation.
//!
//! Why: Correlation IDs enable cross-domain tracing...

use super::*;

#[test]
fn test_fill_correlation_id_fills_empty() { ... }
```

**Benefits:**
- Production files stay focused on implementation
- Test files can be skipped when reading for understanding
- Still compiled conditionally via `#[cfg(test)]`
- Tests remain co-located (same directory) for discoverability

**Migration:** Existing inline `#[cfg(test)] mod tests` blocks should be migrated to `.test.rs` files when touched.

### Test Documentation: Always Explain WHY

Every test must document WHY it exists, not just WHAT it tests. A test without context is maintenance burden waiting to happen.

**Structure for test modules:**

```rust
#[cfg(test)]
mod tests {
    //! Module-level doc explaining the feature being tested and why it matters.
    //!
    //! Example: "Correlation IDs enable cross-domain tracing in saga and PM flows.
    //! Without proper propagation, observability breaks and PMs cannot correlate
    //! related events."

    /// Test-level doc explaining the specific scenario.
    ///
    /// Good: "Commands with empty correlation_id should receive the propagated
    /// value. This is the primary use case: saga/PM produces commands without
    /// setting correlation_id, and the framework fills it from the triggering event."
    ///
    /// Bad: "Test that fill_correlation_id fills empty correlation_id"
    #[test]
    fn test_fill_correlation_id_fills_empty() { ... }
}
```

**What to include:**
- Business context: What capability does this test protect?
- Failure mode: What breaks if this test fails?
- Edge case rationale: Why does this specific scenario matter?

**Section headers for large test files:**

```rust
// ============================================================================
// Selection::Sequences Tests
// ============================================================================

/// Projectors and sagas sometimes need specific events rather than a range...
#[test]
fn test_sequences_returns_sparse_events() { ... }

// ============================================================================
// Missing Cover Validation Tests
// ============================================================================

/// The cover contains domain and root_id which identify the aggregate...
#[test]
fn test_missing_cover_rejected() { ... }
```

### What to Test vs What to Skip

**Test with unit tests (pure logic):**
- Validation functions (`validate_domain`, `fill_correlation_id`)
- Data transformation functions (`base64_encode`, `extract_event_type`)
- State calculations and business rules
- Error message formatting

**Skip unit tests, rely on integration tests:**
- gRPC trait implementations (thin wrappers, need full stack)
- Database operations (need real connections)
- File I/O operations (need file system)
- Code that primarily orchestrates other components

**Rule of thumb:** If you need to mock more than one dependency, it's probably integration test territory.

### Mutation Testing Learnings

After running `cargo mutants`, you'll see patterns:

**Mutations that SHOULD be caught by unit tests:**
- Pure function return values (`replace X -> Y with Default::default()`)
- Conditional branches (`replace X != Y with true`)
- Iterator operations (`replace with vec![]`)

**Mutations that WON'T be caught without integration tests:**
- Async functions that call external services
- Functions that only log/trace (side-effect only)
- Trait implementations delegating to inner types

**Expected kill rates by code type:**

| Code Type | Target Kill Rate | Notes |
|-----------|------------------|-------|
| Pure utility functions | 80%+ | Should be near-perfect |
| Validation/guard logic | 70%+ | May miss some error paths |
| Trait contracts | 70%+ | Via Gherkin contract tests |
| Orchestration glue | 50-60% | Integration test territory |
| gRPC handlers | 30-40% | Thin wrappers, rely on integration |

**What to do with missed mutations:**
1. If pure logic: Add test to catch it
2. If needs mocking: Consider if integration test covers it
3. If side-effect only (logging): Accept the miss
4. If framework glue: Verify integration tests exercise the path

### Trivial Delegation Pattern

Single-line delegation functions that just forward to an inner type (optionally with error mapping) are excluded from unit testing and mutation testing via `#[trivial_delegation]`:

```rust
use crate::trivial_delegation;

#[trivial_delegation]
pub fn has_domain(&self, domain: &str) -> bool {
    self.router.has_handler(domain)
}

#[trivial_delegation]
pub async fn execute(&self, cmd: Command) -> Result<Response, Error> {
    self.inner.execute(cmd).await.map_err(Into::into)
}
```

**Effects:**
- `#[mutants::skip]` - excluded from mutation testing (always)
- `#[coverage(off)]` - excluded from coverage (nightly only, via `coverage_nightly` feature)

**When to use:**
- Single-line methods that delegate to an inner field
- Methods that only add error type conversion (`.map_err(Into::into)`)
- Trait implementations that wrap another implementation

**When NOT to use:**
- Functions with any branching logic
- Functions that extract/parse data before delegating
- Functions with multiple statements

**Running coverage on nightly:**
```bash
cargo +nightly llvm-cov --features coverage_nightly
```

**After adding `#[trivial_delegation]`, regenerate mutation exclusions:**
```bash
cargo xtask gen-mutants-exclude
```

This scans for `#[trivial_delegation]` attributes and updates `.cargo/mutants.toml` with regex patterns to exclude those functions from mutation testing.

These functions are tested implicitly via integration tests that exercise the full stack.

### Contract Tests

**Contract tests MUST break the build.** This diverges from Fowler's historical position that contract tests shouldn't fail builds because they test external dependencies outside your control. In the modern paradigm with testcontainers and hermetic builds, we control the dependency versions. Contract tests verify our code works with specific, pinned versions of backing services. A failing contract test means real breakage that must be fixed before merge.

**We also diverge from the testcontainers authors**, who classify these as "integration tests." Testcontainers enable **contract/interface testing**: verifying implementations fulfill trait contracts against real backing services. We use this more akin to unit testing: isolated verification of a single implementation against its contract, not full-stack integration. Full integration test suites belong in staging environments that mirror production. Ephemeral full-stack tests have value, but less so; you want your primary integration suite running against something production-like. Testcontainers excel at contract verification, not end-to-end integration.

Verify that storage and bus implementations correctly fulfill their trait contracts. Uses **testcontainers** to provision real databases/message brokers in Docker containers.

**All backends use macro-based tests** with shared test suites:
- Storage tests: `tests/storage_*.rs` (sqlite, postgres, redis, immudb, nats)
- Bus tests: `tests/bus_*.rs` (nats, amqp, kafka, pubsub, sns_sqs)
- Shared test suites: `tests/storage/*_tests.rs` and `tests/bus/*_tests.rs` define reusable test functions and macros

**Running tests:**
- SQLite (fast, in-memory): `cargo test --test storage_sqlite --features "sqlite test-utils"`
- PostgreSQL (testcontainers): `cargo test --test storage_postgres --features "postgres test-utils"`
- Bus tests require testcontainers: `cargo test --test bus_nats --features "nats test-utils"`

**Why macro-based (not Gherkin)?** Framework contract tests are Rust-internal. Gherkin's overhead (step definitions, feature files, harness) doesn't add value when the "business domain" is the framework itself. Macros provide ~90% less code with the same coverage. Gherkin remains valuable for examples/ and client/ where polyglot developer education justifies the overhead.

- **Execution**: CI/CD only (containers are slow), but runnable locally

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
- Run with: `just test-local` or `cargo test --test standalone_integration --features sqlite`
- **Execution**: CI/CD mandatory, manual local (takes time)

### Writing Documentation

Feature files and READMEs are living documentation. They explain *why*, not *how*. See [docs/documentation-guide.md](docs/documentation-guide.md) for patterns.

**Code Block Attribution**
Every code block in documentation must either:
1. Link to its source file via `title="path/to/file.ext"` in the code fence
2. Be marked as illustrative with `title="illustrative"` if not from a real file

Never create synthetic/fake code examples without express permission. Prefer real code:
1. Add `# docs:start:<name>` and `# docs:end:<name>` markers to real source files
2. Embed the real code in documentation using those markers
3. Reference actual file paths so readers can find the source

This ensures documentation stays in sync with the codebase and examples are always runnable.

### Acceptance Tests

Test **business behavior** through the full stack. Written in Gherkin, describing what the system does from a business perspective. Exercise real domain logic through sagas, process managers, and projectors.

#### Client Libraries: Unified Rust gRPC Harness

Client libraries (`client/{lang}/`) are tested with a **single Rust Gherkin harness** via gRPC:
- One source of truth for SDK contract testing
- Same tests validate all language implementations
- Tests exercise actual gRPC protocol

```bash
just test-client python    # Test Python client via Rust harness
just test-client go        # Test Go client via Rust harness
just test-clients          # Test all clients
```

#### Examples: Per-Language Gherkin Harnesses

Example implementations (`examples/{lang}/`) use **per-language test harnesses**:
- Demonstrative for non-polyglot developers
- Developers see Gherkin + step definitions in their language
- Educational code they can learn from

```bash
just examples python test  # behave (Python)
just examples go test      # godog (Go)
just examples rust test    # cucumber-rs (Rust)
just examples java test    # cucumber-junit5 (Java)
just examples csharp test  # SpecFlow (C#)
just examples cpp test     # cucumber-cpp (C++)
```

#### Location
- **Shared feature files**: `examples/features/unit/*.feature` (canonical)
- **Client harness**: `tests/client/` (Rust gRPC harness)
- **Example step definitions**: `examples/{lang}/features/steps/` (per-language)

#### Execution Modes
- **Standalone** (default): in-process `RuntimeBuilder` with SQLite + channel bus
- **Direct** (`ANGZARR_TEST_MODE=direct`): remote gRPC against deployed cluster

### Gherkin Authoring

Gherkin is business-readable specification, not test code. Describe **what** the system does and **why** it matters—never **how**.

**The litmus test:** "Will this wording change if the implementation changes?" If yes, abstract to behavior.

#### Declarative Over Imperative

```gherkin
# Wrong: UI choreography
When I click "Add to Cart"
And I click "Checkout"
And I fill in "Card Number" with "4111..."

# Right: Business intent
When I purchase the items in my cart
```

#### Given-When-Then Semantics

| Keyword | Purpose | Example |
|---------|---------|---------|
| Given | Establish context (past state) | `Given a player with $500 in their bankroll` |
| When | Single triggering action | `When the player reserves $200 for the table` |
| Then | Verify business outcomes | `Then the player's available balance is $300` |

#### Business Language

| Technical (Avoid) | Business (Prefer) |
|-------------------|-------------------|
| API returns 201 | Order is confirmed |
| Database has record | Customer exists |
| Event is published | Notification is sent |
| State machine transitions | Hand progresses to showdown |

Exception: Framework tests (event stores, buses) use technical vocabulary—it's their domain.

#### One Scenario, One Behavior

Each scenario tests exactly one thing. Multiple When-Then pairs = multiple scenarios.

#### Feature Preambles

Open features with context explaining:
- **What** this capability enables
- **Why** it matters to the business
- **What breaks** if it doesn't work

```gherkin
Feature: Player fund reservation

  Players must reserve funds when joining a table. This ensures:
  - Players can cover their buy-in before sitting down
  - Funds are locked and cannot be double-spent across tables

  Without fund reservation, players could join multiple tables with
  the same bankroll, creating settlement disputes.
```

#### Error Cases Are First-Class

Don't just test happy paths. Business rules live in constraints:

```gherkin
Scenario: Cannot reserve more than available balance
  Given Alice has $500 available
  When Alice tries to reserve $600
  Then the request fails with "insufficient funds"
  And Alice's available balance remains $500
```

#### Anti-Patterns

- **UI steps**: "click", "fill in", "navigate to"
- **Technical assertions**: "database has row", "event published"
- **Conditional logic**: "if valid then X else Y" (use separate scenarios)
- **Vague outcomes**: "works correctly" (be specific)
- **Hardcoded test data**: Use meaningful descriptions

#### Cross-Domain Scenarios

Show saga/PM translations explicitly without exposing implementation:

```gherkin
Scenario: Order completion triggers fulfillment
  Given an order with items:
    | sku    | quantity |
    | WIDGET | 3        |
  When the order is completed
  Then a fulfillment request is created with:
    | sku    | quantity |
    | WIDGET | 3        |
```

## Proto
When using proto generated code, use extension traits to add functionality to the generated code.  Do not use free functions or explicit wrappers.

## Coordinators
### Aggregates
Business logic is implemented in aggregates.  Accept commands, emit events.

#### Handler Pattern: guard/validate/compute

All aggregate command handlers follow a three-function pattern that makes business logic **100% unit testable** without mocking frameworks or infrastructure:

```
guard(state) → Result<()>
    Check state preconditions (aggregate exists, correct phase, etc.)
    Pure function: state in, Result out

validate(cmd, state) → Result<ValidatedData>
    Validate command inputs against current state
    Returns validated/transformed data needed by compute
    Pure function: command + state in, Result out

compute(cmd, state, validated) → Event
    Build the resulting event from inputs
    Pure function: no side effects, deterministic output
    All business calculations happen here
```

The public `handle_*` function is thin orchestration:
1. Unpack protobuf command
2. Call guard → validate → compute
3. Pack event into EventBook

**Why this matters:**
- `guard()`, `validate()`, `compute()` are pure functions—call directly in tests
- No mocking required: pass state structs directly, assert on returned events
- Each function has single responsibility, testable in isolation
- Proto serialization tested separately from business logic
- Same pattern across all languages (Python, Go, Rust)

**Example test (Rust):**
```rust
#[test]
fn test_deposit_increases_bankroll() {
    let state = PlayerState { bankroll: 100, ..Default::default() };
    let cmd = DepositFunds { amount: 50 };

    let event = compute(&cmd, &state);

    assert_eq!(event.new_bankroll, 150);
}
```

**Example test (Python):**
```python
def test_deposit_increases_bankroll():
    state = PlayerState(bankroll=100)
    cmd = DepositFunds(amount=50)

    event = compute(cmd, state)

    assert event.new_bankroll == 150
```

#### Event Sourcing: The Any Boundary

Events cross a serialization boundary between business logic and the framework:

```
Business Logic                    Framework
─────────────────────────────────────────────────────
compute(cmd, state) → raw event
                      ↓
                      Any.Pack(event)  →  persist to EventBook
                      ↓
                      EventBook.pages[].event  (Any-wrapped)
                      ↓
build_state(state, events)  ←  extract events from pages
                      ↓
_apply_event(state, event_any)
                      ↓
                      event_any.Unpack(typed_event)
                      ↓
                      mutate state
```

**Key insight:** The framework stores events as opaque `Any` blobs—it doesn't know business types. Business logic must decode the `Any` because only it knows `PlayerRegistered`, `FundsDeposited`, etc.

**`build_state(state, events)`** takes Any-wrapped events:
- This matches what comes from EventBook (framework → business)
- `_apply_event` unpacks Any into typed events and mutates state
- Tests wrap raw events into Any before calling, mimicking production

**Full event sourcing test cycle:**
```python
def test_deposit_cycle():
    # 1. Start with state
    state = PlayerState(bankroll=100)
    cmd = DepositFunds(amount=50)

    # 2. compute() produces raw event
    event = compute(cmd, state)

    # 3. Wrap in Any (what framework does for persistence)
    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.googleapis.com/")

    # 4. build_state applies Any-wrapped events → new state
    new_state = build_state(state, [event_any])

    assert new_state.bankroll == 150
```

Tests mimic the production boundary exactly—no special test-only interfaces.

### Sagas
Domain translators. They translate the language of domain A to the language of domain B. Accept events from a single domain, emit commands to a different domain. Single domain in, single domain out. There may be multiple sagas per aggregate, bridging to different domains.

Sagas should contain extremely limited logic—just enough to map fields and construct the target command. If you find yourself adding conditionals or business rules, that logic likely belongs in the source aggregate.

**Sagas must be stateless.** Each event is processed independently with no memory of previous events. If you need to correlate events across multiple domains, use a Process Manager. If you need stateful decision-making, that logic belongs in an Aggregate. Statelessness enables:
- Horizontal scaling (any instance can handle any event)
- Simpler testing (no setup of prior state)
- Fault tolerance (replay events without side effects from stale state)

**Sagas must set sequences from destination state.** The framework fetches destination EventBooks before calling your saga's execute method. Your saga MUST:
1. Use destination state to make business decisions
2. Set `command.pages[0].sequence = destination.next_sequence()`

The framework intentionally does NOT auto-stamp sequences. This forces saga authors to engage with destination state before producing commands. Commands with wrong sequences are rejected.

Name sagas `saga-{source}-{target}`. Examples:
- `saga-order-fulfillment` (order events → fulfillment commands)
- `saga-fulfillment-inventory` (fulfillment events → inventory commands)

### Projectors
Accept events from a single domain, output to external systems, databases, event streams to external systems, files, etc.

Name projectors `projector-{source}-{feature}`. Examples:
- `projector-inventory-stock` (inventory events → stock level read model)
- `projector-order-web` (order events → web API cache)

### Process Managers
Accepts events across multiple domains, joins them together via the correlation ID. May emit commands to other domains. Super sagas/aggregates. These should generally be a state machine correlating events from multiple domains.

**PM as Aggregate:** Process managers are their own aggregate, with the correlation ID as aggregate root. The PM tracks the state of a specific cross-domain business process; the correlation ID identifies that process. Events without a correlation ID should not invoke PMs—the router guards against this.

**PMs must set sequences from destination state.** Same as sagas—use `destination.next_sequence()` when building commands. The framework validates sequences for optimistic concurrency.

### Event Design
Sagas and projectors operate only on the events they receive—no querying. If they lack information, enrich the event at the source aggregate.

Aggregates may query external systems when processing commands to gather information for decision-making—projections, third-party APIs, legacy systems. However, aggregates should only *read* from external systems, never *write*. Side effects to external systems belong in projectors, which react to committed events.

Keep events lean. Use IDs to reference immutable objects rather than embedding full data—if the object won't change, the ID suffices.

## Component Subscriptions

Components declare which domains/events they subscribe to via configuration:

### Configuration-Based Subscriptions

Subscriptions are configured via environment variable or config file—not derived from code:

**Environment Variable** (distributed mode):
```bash
# Format: domain:Type1,Type2;domain2:Type3
ANGZARR_SUBSCRIPTIONS="order:OrderCreated,OrderCompleted;inventory"
```

**Config File** (standalone mode):
```yaml
sagas:
  - domain: saga-order-fulfillment
    subscriptions: "order:OrderCompleted"
    command: ["./saga-order-fulfillment"]
```

- Empty types list means "all events from domain"
- Multiple domains separated by `;`
- Event types within a domain separated by `,`

### Target Type

A `Target` struct represents subscriptions:
```rust
pub struct Target {
    pub domain: String,      // Domain name
    pub types: Vec<String>,  // Event type names (empty = all)
}
```

- For sagas: domain to listen for, event types to process
- For PMs: multiple domains with their event filters

## Topology

The topology graph is built from **runtime event observation**:

- **Nodes**: Created when components process their first event
- **Edges**: Inferred from event flow between components
- **Metrics**: Updated from event observation (counts, last seen)

Graph structure emerges from actual event traffic.

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
- **Events**: Domain-specific facts that have occurred. Immutable. Named in past tense (OrderCreated, StockReserved). Events enter the system via commands (validated) or as facts (injected directly).
- **Commands**: Requests to perform actions. Sequenced, validated, can be rejected. Named imperatively (CreateOrder, ReserveStock). Produce events when accepted.
- **Facts**: Events injected directly into an aggregate, bypassing command validation. Represent external realities the aggregate must accept (e.g., "hand says it's your turn"). Cannot be rejected. Sequenced and persisted like any other event.
- **Notifications**: Unsequenced coordination messages. NOT persisted to the event store. Used for framework-level coordination (e.g., `RejectionNotification` for compensation routing). Not events.

  **Entry paths for events:**
  | Path | Sequenced | Validated | Can Reject |
  |------|-----------|-----------|------------|
  | Command → Event | Yes | Yes | Yes |
  | Fact (direct injection) | Yes | No | No |

  **Notifications** are not events—they are transient coordination messages.

- **Target**: A domain + list of event types. Used for subscriptions (inputs) to filter which events a component receives.
- **Correlation ID**: Identifies a cross-domain business process. Not a domain entity ID (like `order_id` or `game_id`)—it's the identifier for the workflow/transaction that spans domains. Stable across all events in that process. Flows through sagas/PMs. For PMs, the correlation ID serves as the aggregate root.

  **Propagation rules:**
  - Client must provide correlation_id on the initial command if cross-domain tracking is needed
  - Framework does NOT auto-generate correlation_id—if not provided, it stays empty
  - Once set, angzarr propagates correlation_id through sagas, PMs, and resulting commands
  - PMs require correlation_id—events without one are skipped (guarded at router level) 

## Project Layout

Organize example projects by domain. Each domain gets its own directory containing its aggregate and outbound sagas.

### Directory Structure
```
examples/{lang}/
├── {domain}/
│   ├── agg/              # Domain aggregate
│   └── saga-{target}/    # Saga: this domain → target domain
├── pmg-{name}/           # Process managers (peers to domains)
├── prj-{name}/           # Projectors (peers to domains)
└── tests/
```

### Placement Rules

| Component | Location | Naming |
|-----------|----------|--------|
| Aggregate | `{domain}/agg/` | Binary: `agg-{domain}` |
| Saga | `{source}/saga-{target}/` | Binary: `saga-{source}-{target}` |
| Process Manager | `pmg-{name}/` | Peer to domains |
| Projector | `prj-{name}/` | Peer to domains |

### Example: Poker Domain
```
examples/rust/
├── player/
│   └── agg/                    # Player aggregate
├── table/
│   ├── agg/                    # Table aggregate
│   ├── saga-hand/              # Table events → Hand commands
│   └── saga-player/            # Table events → Player commands
├── hand/
│   ├── agg/                    # Hand aggregate
│   ├── saga-table/             # Hand events → Table commands
│   └── saga-player/            # Hand events → Player commands
├── pmg-hand-flow/              # Cross-domain hand orchestration
├── prj-output/                 # Multi-domain projector
└── tests/
```

### Rationale
- **Sagas live with source domain**: A saga translates FROM its source domain TO another. Grouping by source keeps related translation logic together.
- **Process managers are peers**: PMs correlate events across multiple domains—no single domain owns them.
- **Projectors are peers**: Multi-domain projectors don't belong to any single domain.

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

### Proto Field Unification
When proto messages share structure, unify them. Separate `Subscription` (event types) and `CommandTarget` (command types) were unified into a single `Target` with a `types` field. Duplication leads to:
- Inconsistent APIs
- Double maintenance burden
- Confusion about which type to use where

<!-- rtk-instructions v2 -->
# RTK (Rust Token Killer) - Token-Optimized Commands

## Golden Rule

**Always prefix commands with `rtk`**. If RTK has a dedicated filter, it uses it. If not, it passes through unchanged. This means RTK is always safe to use.

**Important**: Even in command chains with `&&`, use `rtk`:
```bash
# ❌ Wrong
git add . && git commit -m "msg" && git push

# ✅ Correct
rtk git add . && rtk git commit -m "msg" && rtk git push
```

## RTK Commands by Workflow

### Build & Compile (80-90% savings)
```bash
rtk cargo build         # Cargo build output
rtk cargo check         # Cargo check output
rtk cargo clippy        # Clippy warnings grouped by file (80%)
rtk tsc                 # TypeScript errors grouped by file/code (83%)
rtk lint                # ESLint/Biome violations grouped (84%)
rtk prettier --check    # Files needing format only (70%)
rtk next build          # Next.js build with route metrics (87%)
```

### Test (90-99% savings)
```bash
rtk cargo test          # Cargo test failures only (90%)
rtk vitest run          # Vitest failures only (99.5%)
rtk playwright test     # Playwright failures only (94%)
rtk test <cmd>          # Generic test wrapper - failures only
```

### Git (59-80% savings)
```bash
rtk git status          # Compact status
rtk git log             # Compact log (works with all git flags)
rtk git diff            # Compact diff (80%)
rtk git show            # Compact show (80%)
rtk git add             # Ultra-compact confirmations (59%)
rtk git commit          # Ultra-compact confirmations (59%)
rtk git push            # Ultra-compact confirmations
rtk git pull            # Ultra-compact confirmations
rtk git branch          # Compact branch list
rtk git fetch           # Compact fetch
rtk git stash           # Compact stash
rtk git worktree        # Compact worktree
```

Note: Git passthrough works for ALL subcommands, even those not explicitly listed.

### GitHub (26-87% savings)
```bash
rtk gh pr view <num>    # Compact PR view (87%)
rtk gh pr checks        # Compact PR checks (79%)
rtk gh run list         # Compact workflow runs (82%)
rtk gh issue list       # Compact issue list (80%)
rtk gh api              # Compact API responses (26%)
```

### JavaScript/TypeScript Tooling (70-90% savings)
```bash
rtk pnpm list           # Compact dependency tree (70%)
rtk pnpm outdated       # Compact outdated packages (80%)
rtk pnpm install        # Compact install output (90%)
rtk npm run <script>    # Compact npm script output
rtk npx <cmd>           # Compact npx command output
rtk prisma              # Prisma without ASCII art (88%)
```

### Files & Search (60-75% savings)
```bash
rtk ls <path>           # Tree format, compact (65%)
rtk read <file>         # Code reading with filtering (60%)
rtk grep <pattern>      # Search grouped by file (75%)
rtk find <pattern>      # Find grouped by directory (70%)
```

### Analysis & Debug (70-90% savings)
```bash
rtk err <cmd>           # Filter errors only from any command
rtk log <file>          # Deduplicated logs with counts
rtk json <file>         # JSON structure without values
rtk deps                # Dependency overview
rtk env                 # Environment variables compact
rtk summary <cmd>       # Smart summary of command output
rtk diff                # Ultra-compact diffs
```

### Infrastructure (85% savings)
```bash
rtk docker ps           # Compact container list
rtk docker images       # Compact image list
rtk docker logs <c>     # Deduplicated logs
rtk kubectl get         # Compact resource list
rtk kubectl logs        # Deduplicated pod logs
```

### Network (65-70% savings)
```bash
rtk curl <url>          # Compact HTTP responses (70%)
rtk wget <url>          # Compact download output (65%)
```

### Meta Commands
```bash
rtk gain                # View token savings statistics
rtk gain --history      # View command history with savings
rtk discover            # Analyze Claude Code sessions for missed RTK usage
rtk proxy <cmd>         # Run command without filtering (for debugging)
rtk init                # Add RTK instructions to CLAUDE.md
rtk init --global       # Add RTK to ~/.claude/CLAUDE.md
```

## Token Savings Overview

| Category | Commands | Typical Savings |
|----------|----------|-----------------|
| Tests | vitest, playwright, cargo test | 90-99% |
| Build | next, tsc, lint, prettier | 70-87% |
| Git | status, log, diff, add, commit | 59-80% |
| GitHub | gh pr, gh run, gh issue | 26-87% |
| Package Managers | pnpm, npm, npx | 70-90% |
| Files | ls, read, grep, find | 60-75% |
| Infrastructure | docker, kubectl | 85% |
| Network | curl, wget | 65-70% |

Overall average: **60-90% token reduction** on common development operations.
<!-- /rtk-instructions -->
