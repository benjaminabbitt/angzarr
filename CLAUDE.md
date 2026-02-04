<!-- SCM:BEGIN -->
@.scm/context.md
<!-- SCM:END -->


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

Name sagas saga-{origin}-{destination}.

### Projectors
Accept events from a single domain, output to external systems, databases, event streams to external systems, files, etc.  May query other domain projections to enhance output.

Name projectors projector-{origin}-{distinguishing_feature}

### Process Managers
Accepts events across multiple domains, joins them together via the correlation ID. May emit commands to other domains.  Super sagas/aggregates.  These should generally be a state machine correlating events from multiple domains.  Are their own aggregate as well, with the domain being the correlation ID as root.

## Glossary
### Angzarr
- **Coordinator**: The angzarr support coordinator that abstracts functionality away from business logic code.  It's sometimes deployed as a sidecar container in a pod with its business logic.  Architecturally, it's a thin wrapper around library code that is also reused in angzarr standalone coordinator.   Variously, in the past, this may have been referred to as a sidecar, but that's not a precisely correct.
- **Events**: Domain specific events 

## Crate Organization
- Each saga is its own crate with focused, single-purpose translation logic
- Each projector in its own crate with focuse, single-purpose output logic
- Each aggregate in its own crate with focused, single-purpose business logic
- Each process manager in its own crate with a minimal bit of functionality that orchestrates cross-domain logic.  Used very sparingly.
- Never combine multiple source domain handlers in one crate deployed with env var switching
- More, smaller pieces over fewer, larger ones
- Aggregates, sagas, and projectors for the same domain are separate crates