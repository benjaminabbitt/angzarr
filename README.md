# ⍼ Angzarr

A CQRS/Event Sourcing infrastructure framework in Rust.

## Overview

Angzarr provides the infrastructure layer for event-sourced systems:
- Event persistence with sequence validation
- Snapshot optimization for aggregate replay
- gRPC event distribution
- Projector and saga coordination

Business logic runs as external gRPC services, so teams can use whatever language fits their needs.

## Why Angzarr

CQRS/Event Sourcing delivers powerful benefits—complete audit trails, temporal queries, scalable read models—but the infrastructure complexity is prohibitive. Teams need expertise in event stores, snapshot optimization, distributed messaging, saga coordination, and concurrency control before writing a single line of business logic.

Angzarr inverts this equation: **business logic becomes the only thing you write**.

### The Core Insight

Business logic should be pure functions: `(state, command) → events`. When you strip away infrastructure concerns, what remains is exactly what junior developers and AI code generators excel at:

- Clear input/output contracts (protobuf schemas)
- No side effects to reason about
- No concurrency bugs possible (single aggregate, sequential events)
- Testable in isolation with simple assertions

### The Pit of Success

The architecture makes incorrect code difficult to write. A developer literally *cannot*:

| Mistake | Why It's Impossible |
|---------|---------------------|
| Introduce race conditions | Single aggregate processes commands sequentially |
| Corrupt database transactions | Angzarr manages all persistence |
| Create connection pool exhaustion | No direct database access |
| Accidentally expose internal state | State is reconstructed from events |
| Break other aggregates | Aggregates are isolated by design |

They write a function. It either returns the correct events or it doesn't. That's testable with unit tests against the handler alone.

### What Junior Devs and AIs Write

Every handler follows the same mechanical pattern:

```
1. GUARD:    Check preconditions against current state
2. VALIDATE: Check command field validity
3. COMPUTE:  Calculate derived values (pure math)
4. EMIT:     Return event(s) describing what happened
```

Here's the same business logic in three languages—notice the structural identity:

**Go** (24 lines of logic):
```go
func (l *DefaultCartLogic) HandleAddItem(state *CartState, productID, name string,
    quantity, unitPriceCents int32) (*examples.ItemAdded, error) {
    // GUARD
    if !state.Exists() {
        return nil, NewFailedPrecondition(ErrMsgCartNotFound)
    }
    if !state.IsActive() {
        return nil, NewFailedPrecondition(ErrMsgCartCheckedOut)
    }
    // VALIDATE
    if productID == "" {
        return nil, NewInvalidArgument(ErrMsgProductIDRequired)
    }
    if quantity <= 0 {
        return nil, NewInvalidArgument(ErrMsgQuantityPositive)
    }
    // COMPUTE + EMIT
    newSubtotal := state.SubtotalCents + (quantity * unitPriceCents)
    return &examples.ItemAdded{
        ProductId:      productID,
        Name:           name,
        Quantity:       quantity,
        UnitPriceCents: unitPriceCents,
        NewSubtotal:    newSubtotal,
        AddedAt:        timestamppb.Now(),
    }, nil
}
```

**Python** (25 lines of logic):
```python
def handle_add_item(command_book, command_any, state: CartState, seq: int, log):
    # GUARD
    if not state.exists():
        raise CommandRejectedError(errmsg.CART_NOT_FOUND)
    if not state.is_active():
        raise CommandRejectedError(errmsg.CART_CHECKED_OUT)

    cmd = domains.AddItem()
    command_any.Unpack(cmd)

    # VALIDATE
    if not cmd.product_id:
        raise CommandRejectedError(errmsg.PRODUCT_ID_REQUIRED)
    if cmd.quantity <= 0:
        raise CommandRejectedError(errmsg.QUANTITY_POSITIVE)

    # COMPUTE + EMIT
    new_subtotal = state.subtotal_cents + (cmd.quantity * cmd.unit_price_cents)
    event = domains.ItemAdded(
        product_id=cmd.product_id,
        name=cmd.name,
        quantity=cmd.quantity,
        unit_price_cents=cmd.unit_price_cents,
        new_subtotal=new_subtotal,
        added_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )
    # ... return EventBook with event
```

**Rust** (similar structure, ~40 lines with protobuf encoding)

### Error Messages are Pre-defined Constants

Handlers choose from a vocabulary of domain-specific errors:

```python
class errmsg:
    CART_NOT_FOUND = "Cart does not exist"
    CART_CHECKED_OUT = "Cart is already checked out"
    QUANTITY_POSITIVE = "Quantity must be positive"
    PRODUCT_ID_REQUIRED = "Product ID is required"
```

```go
const (
    ErrMsgCartNotFound      = "Cart does not exist"
    ErrMsgCartCheckedOut    = "Cart is already checked out"
    ErrMsgQuantityPositive  = "Quantity must be positive"
    ErrMsgProductIDRequired = "Product ID is required"
)
```

### Why This is AI-Friendly

LLMs excel at pattern matching from examples and pure transformations. They struggle with distributed system edge cases, concurrency reasoning, and infrastructure configuration. Angzarr puts all the hard stuff in a layer LLMs don't touch.

**Prompt to generate a new handler:**

> "Write a `RemoveItem` handler for the cart aggregate. It should:
> - Reject if cart doesn't exist
> - Reject if cart is checked out
> - Reject if item not in cart
> - Emit `ItemRemoved` event with product_id and new_subtotal
>
> Follow the same pattern as `AddItem`."

An LLM produces correct code because:
- **Clear contract**: Input and output types are schema-defined
- **Examples to follow**: Every handler has identical structure
- **No hidden state**: Functions are pure
- **Finite error space**: Pick from predefined constants
- **Testable assertion**: Given state X and command Y, expect event Z

### The Testing Story

Unit tests require no infrastructure mocking:

```python
def test_add_item_to_nonexistent_cart_rejected():
    state = CartState()  # empty, doesn't exist
    cmd = AddItem(product_id="sku-1", quantity=1, unit_price_cents=999)

    with pytest.raises(CommandRejectedError) as exc:
        handle_add_item(mock_book, cmd, state, 1, log)

    assert exc.value.args[0] == errmsg.CART_NOT_FOUND
```

```go
func TestAddItemToNonexistentCart(t *testing.T) {
    state := &CartState{} // empty
    _, err := logic.HandleAddItem(state, "sku-1", "Widget", 1, 999)

    assert.Equal(t, ErrMsgCartNotFound, err.Error())
}
```

No database setup. No container spinning. No network mocking. Pure function in, assertion out.

### What Angzarr Handles (So You Don't)

| Concern | Handled By |
|---------|------------|
| Database transactions | Angzarr |
| Optimistic concurrency | Angzarr |
| Event ordering | Angzarr |
| Retry logic | Angzarr |
| Network failures | Angzarr |
| Service discovery | Angzarr |
| Load balancing | Angzarr |
| State hydration | Angzarr |
| Snapshot management | Angzarr |
| Message serialization | Protobuf |
| Schema evolution | Protobuf |
| Deployment | DevOps/Helm |
| Scaling | K8s/DevOps |

### The Staffing Model

```
┌─────────────────────────────────────────────────────────┐
│                    SENIOR / DEVOPS                       │
│  - Schema design (protobuf)                             │
│  - Saga orchestration                                   │
│  - Infrastructure (helm, k8s, messaging)                │
│  - Cross-cutting concerns                               │
└─────────────────────────────────────────────────────────┘
                           │
                           │ defines contracts
                           ▼
┌─────────────────────────────────────────────────────────┐
│               JUNIOR DEVS / AI AGENTS                    │
│                                                          │
│   handle_create_cart()    handle_add_item()             │
│   handle_remove_item()    handle_checkout()             │
│   handle_apply_coupon()   handle_clear_cart()           │
│                                                          │
│   (pure functions, ~30 lines each, unit testable)       │
└─────────────────────────────────────────────────────────┘
```

The senior defines the **what** (schemas, aggregates, events). Juniors and AIs implement the **how** (business rules). Neither touches the **where** (infrastructure) during normal development.

### Deployment Options

Adapting Angzarr to your infrastructure requires a one-time DevOps effort:

| Environment | Effort |
|-------------|--------|
| AWS/GCP managed services | Configuration only |
| Existing Kubernetes cluster | Helm install to namespace |
| Custom infrastructure | Integrate storage/messaging backends |

Once deployed, business logic development requires zero infrastructure knowledge.

## Installation

### Helm Chart (Recommended)

```bash
# Add and install from OCI registry
helm install angzarr oci://ghcr.io/benjaminabbitt/charts/angzarr --version 0.1.0

# With custom values
helm install angzarr oci://ghcr.io/benjaminabbitt/charts/angzarr \
  --version 0.1.0 \
  --namespace angzarr \
  --create-namespace \
  -f values.yaml
```

### Container Images

All images are published to GitHub Container Registry with multi-arch support (amd64/arm64):

```bash
# Pull individual images
docker pull ghcr.io/benjaminabbitt/angzarr-aggregate:0.1.0
docker pull ghcr.io/benjaminabbitt/angzarr-projector:0.1.0
docker pull ghcr.io/benjaminabbitt/angzarr-saga:0.1.0
docker pull ghcr.io/benjaminabbitt/angzarr-stream:0.1.0
docker pull ghcr.io/benjaminabbitt/angzarr-gateway:0.1.0
docker pull ghcr.io/benjaminabbitt/angzarr-log:0.1.0
```

### OpenTofu/Terraform Modules

Deploy backing services (databases, messaging) with infrastructure-as-code:

```hcl
# Database (MongoDB or PostgreSQL)
module "database" {
  source = "github.com/benjaminabbitt/angzarr//deploy/tofu/modules/database?ref=v0.1.0"

  type      = "mongodb"  # or "postgresql"
  namespace = "angzarr"
  # Passwords auto-generated if not provided
}

# Messaging (RabbitMQ or Kafka)
module "messaging" {
  source = "github.com/benjaminabbitt/angzarr//deploy/tofu/modules/messaging?ref=v0.1.0"

  type      = "rabbitmq"  # or "kafka"
  namespace = "angzarr"
}

# Redis (caching/sessions)
module "redis" {
  source = "github.com/benjaminabbitt/angzarr//deploy/tofu/modules/redis?ref=v0.1.0"

  namespace = "angzarr"
}
```

## Documentation

- **[TOOLING.md](TOOLING.md)** — Development tools setup (just, bacon, mold, sccache, Kind)
- **[docs/](docs/)** — Architecture guides and pattern documentation
  - [CQRS and Event Sourcing Concepts](docs/cqrs-event-sourcing.md) — Background for newcomers
  - [Command Handlers (Aggregates)](docs/command-handlers.md) — Processing commands, emitting events
  - [Projectors](docs/projectors.md) — Building read models and performing side effects
  - [Sagas (Process Coordinators)](docs/sagas.md) — Orchestrating workflows across aggregates

## Getting Started

### Prerequisites

- Rust 1.70+
- [Podman](https://podman.io/) or Docker (for Kubernetes development)
- [Kind](https://kind.sigs.k8s.io/) - local Kubernetes clusters
- [just](https://github.com/casey/just) - command runner (see below)
- [bacon](https://github.com/Canop/bacon) - background code checker (see below)
- [Helm](https://helm.sh/) - Kubernetes package manager
- [mold](https://github.com/rui314/mold) - fast linker (recommended)
- [sccache](https://github.com/mozilla/sccache) - compilation cache (recommended)
- grpcurl (optional, for debugging)

See [TOOLING.md](TOOLING.md) for detailed setup instructions and usage guides.

#### Fast Build Setup (Recommended)

Install mold and sccache for significantly faster builds:

```bash
# Debian/Ubuntu
sudo apt install mold clang

# Fedora
sudo dnf install mold clang

# macOS (mold not available, use default linker)
# The .cargo/config.toml will use default linker on macOS

# Install sccache
cargo install sccache

# Enable sccache (add to ~/.bashrc or ~/.zshrc)
export RUSTC_WRAPPER=sccache
```

**Expected speedups:**
- mold linker: 50-80% faster linking
- sccache: Near-instant rebuilds on cache hits
- `just build-fast`: Uses fast-dev profile (no debug info, max parallelism)

### About `just`

This project uses [just](https://github.com/casey/just) as its command runner. If you're familiar with Makefiles, `just` will feel familiar - it uses a similar syntax but is purpose-built for running commands rather than building files. Justfiles are easy to read even without prior experience.

**Install just:**
```bash
# macOS
brew install just

# Arch Linux
pacman -S just

# Cargo (any platform)
cargo install just
```

**Basic usage:**
```bash
# List all available commands
just

# Run a specific command
just build

# Commands can have submodules
just examples build
just examples helm-lint
```

View any `justfile` in the repository to see what commands do - they're self-documenting with comments.

### About `bacon`

[Bacon](https://github.com/Canop/bacon) is a background Rust code checker that watches for file changes and runs cargo commands automatically. It provides instant feedback without manually re-running builds.

**Why bacon over `cargo watch`:**
- Smarter output: only shows errors/warnings, not successful build noise
- Keyboard shortcuts: switch between check/build/test/clippy without restarting
- Job configuration: project-specific jobs in `bacon.toml`

**Install bacon:**
```bash
cargo install bacon
```

**Basic usage:**
```bash
# Start bacon with default job (check)
bacon

# Start with a specific job
bacon clippy
bacon test

# Switch jobs with keybindings while running:
#   c = check
#   b = build
#   t = test
#   l = clippy
#   f = fmt
```

**Project jobs** (defined in `bacon.toml`):

| Job | Description |
|-----|-------------|
| `check` | Fast compile check (default) |
| `build` | Full release build with standalone features |
| `test` | Run unit tests |
| `clippy` | Lint with warnings as errors |
| `fmt` | Format code |
| `proto` | Regenerate protobuf bindings |
| `ex-cart` | Check cart example |
| `ex-rust` | Check all Rust examples |
| `bin-standalone` | Build standalone binary |

**Typical workflow:**
1. Start `bacon` in a terminal - leave it running
2. Edit code in your editor
3. Bacon automatically rebuilds on save
4. Fix errors as they appear
5. Press `t` to run tests, `l` for clippy

This tight feedback loop catches errors seconds after you introduce them, before context-switching away from the problem.

### 1. Clone and Build

```bash
git clone https://github.com/yourorg/angzarr
cd angzarr

# Build the framework
just build

# Run tests to verify setup
just test
```

### 2. Run In-Memory (Development)

The fastest path to experimentation—no external dependencies:

```bash
# Start the server with mock storage (in-memory)
just run

# Or run acceptance tests directly
cargo test --test acceptance
```

### 3. Standalone Mode (Local Multi-Process)

For development that mirrors production architecture without external infrastructure:

```bash
# Build with standalone features (SQLite + Channel bus + UDS)
cargo build --features standalone

# Copy the standalone config template
cp config.standalone.yaml config.yaml

# Start services (each in separate terminals)
cargo run --features standalone --bin angzarr-aggregate
cargo run --features standalone --bin angzarr-stream
cargo run --features standalone --bin angzarr-log
```

Standalone mode uses:
- **SQLite** for event storage (in-memory by default, or file-based)
- **Channel event bus** for in-process pub/sub (replaces AMQP/Kafka)
- **Unix domain sockets** for gRPC transport (replaces TCP ports)

Socket files are created under `/tmp/angzarr/` by default:
- `gateway.sock` - Command gateway
- `aggregate.sock` - Aggregate sidecar
- `stream.sock` - Event streaming
- `log.sock` - Logging projector

**Pivoting to production**: Change `config.yaml` settings to use:
- `storage.type: postgres` (or mongodb)
- `messaging.type: amqp` (or kafka)
- `transport.type: tcp`

The architecture remains identical—only the transport and infrastructure change.

### 4. Run with Kubernetes (Production-Like)

For realistic multi-service deployments:

```bash
# Deploy Angzarr + dependencies (RabbitMQ, Redis) to Kind cluster
just deploy

# Watch logs
just k8s-logs
```

### 4. Create Your First Domain

Create a entity in your preferred language. Example in Python:

```python
# examples/python/customer/customer_logic.py
class CustomerLogic:
    def handle(self, contextual_command):
        state = self._rebuild_state(contextual_command.events)
        command = contextual_command.command

        # Validate and emit events
        if command.type_url.endswith("CreateCustomer"):
            return self._handle_create_customer(command, state)
        # ... other commands

    def _handle_create_customer(self, command, state):
        if state.name:
            raise ValueError("Customer already exists")
        return EventBook(pages=[CustomerCreated(name=command.name, email=command.email)])
```

Register it in `config.yaml`:

```yaml
business_logic:
  - domain: customer
    address: localhost:50052
```

## Architecture

Business logic lives in external services called via gRPC. Angzarr handles:
- **EventStore**: Persist and query events (MongoDB, PostgreSQL, EventStoreDB)
- **SnapshotStore**: Optimize replay with snapshots
- **EventBus**: Distribute events to projectors/sagas
- **CommandHandler**: Orchestrate command processing
- **ProjectorCoordinator**: Route events to read model builders
- **SagaCoordinator**: Route events to cross-aggregate workflows
- **EventStream**: Stream filtered events to subscribers by correlation ID
- **CommandGateway**: Forward commands and stream back resulting events

### Binaries

Angzarr provides five binaries in two categories:

**Sidecars** (run alongside your business logic in the same pod):
| Binary | Purpose |
|--------|---------|
| `angzarr-aggregate` | Command handling, event persistence, snapshot management |
| `angzarr-projector` | AMQP subscription, event routing to projector services |
| `angzarr-saga` | AMQP subscription, event routing to saga services |

**Standalone Infrastructure** (central services, not sidecars):
| Binary | Purpose |
|--------|---------|
| `angzarr-gateway` | Client entry point, command routing, event streaming to clients |
| `angzarr-stream` | Infrastructure projector for correlation-based event filtering |

### Streaming Infrastructure

For clients that need real-time event streaming, Angzarr provides two standalone infrastructure services:

```
┌────────┐     ┌──────────────┐     ┌─────────────────┐     ┌─────────────────┐
│ Client │────▶│ angzarr-gateway│────▶│ angzarr-aggregate │────▶│ Business Logic  │
└────────┘     └──────────────┘     └─────────────────┘     └─────────────────┘
    ▲                 │                      │
    │                 │                      │ publishes events
    │                 │                      ▼
    │                 │              ┌─────────────────┐
    │                 │              │     AMQP        │
    │                 │              └────────┬────────┘
    │                 │                       │
    │                 │              ┌────────▼────────┐
    │                 │              │angzarr-projector│ (sidecar)
    │                 │              └────────┬────────┘
    │                 │                       │ Projector gRPC
    │                 │              ┌────────▼────────┐
    │                 └─────────────▶│  angzarr-stream │
    │                 registers      │  (projector)    │
    │                 correlation ID └────────┬────────┘
    │                                         │ only forwards if
    │                                         │ correlation ID matches
    └─────────────────────────────────────────┘
              streams matching events back
```

**angzarr-stream** - An infrastructure projector for event streaming:
- Implements the Projector gRPC interface, receiving events from angzarr-projector sidecar
- Maintains a registry of correlation IDs with active subscribers
- Forwards events only to subscribers who registered interest in that correlation ID
- Drops events with no matching subscribers (no storage/buffering)
- Also exposes EventStream gRPC interface for gateway subscriptions

**angzarr-gateway** - Standalone service that:
- Receives commands via `CommandGateway.Execute`
- Generates a deterministic correlation ID (UUIDv5 from command body) if not provided
- Registers interest with angzarr-stream for that correlation ID
- Forwards command to angzarr-aggregate
- Streams resulting events back to the client

#### Usage

```bash
# Build the streaming services
cargo build --release --bin angzarr-stream
cargo build --release --bin angzarr-gateway
```

Configuration via environment variables:

**angzarr-stream:**
| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | Port for gRPC services (Projector + EventStream) | 50051 |

**angzarr-gateway:**
| Variable | Description | Default |
|----------|-------------|---------|
| `COMMAND_ADDRESS` | angzarr-aggregate service address | Required |
| `STREAM_ADDRESS` | angzarr-stream service address | Required |
| `GRPC_PORT` | Port for CommandGateway gRPC service | 1316 |
| `STREAM_TIMEOUT_SECS` | Timeout for event stream | 30 |

#### gRPC API

```protobuf
// Subscribe to events matching a correlation ID
service EventStream {
  rpc Subscribe (EventStreamFilter) returns (stream EventBook) {}
}

message EventStreamFilter {
  string correlation_id = 1;  // Required
}

// Send command and receive resulting events
service CommandGateway {
  rpc Execute (CommandBook) returns (stream EventBook) {}
}
```

## Development Experience

CQRS/Event Sourcing systems are notoriously complex—event stores, snapshot optimization, distributed event routing, projection rebuilds, saga coordination, and concurrency control create significant infrastructure burden. Angzarr absorbs this complexity so you write only business logic.

### What You Write

**Entities** — Pure functions that validate commands against current state and emit events:

```python
def handle_create_customer(command, state):
    if state.name:
        raise CommandRejectedError("Customer already exists")
    if not command.name:
        raise CommandRejectedError("Customer name is required")

    return CustomerCreated(name=command.name, email=command.email)
```

**Projectors** — React to events and build read models:

```python
class ReceiptProjector:
    def project(self, event_book):
        for page in event_book.pages:
            if page.event.type_url.endswith("TransactionCompleted"):
                return Receipt(formatted_text=self._build_receipt(event_book))
        return None  # No projection if transaction not complete
```

**Sagas** — Cross-aggregate workflows that react to events and emit commands:

```python
class LoyaltyPointsSaga:
    def handle(self, event_book):
        for page in event_book.pages:
            if page.event.type_url.endswith("TransactionCompleted"):
                completed = TransactionCompleted.FromString(page.event.value)
                return [AddLoyaltyPoints(
                    customer_id=completed.customer_id,
                    points=completed.loyalty_points_earned
                )]
        return []
```

### What Angzarr Handles

- Event persistence with sequence validation and optimistic concurrency
- Snapshot creation and optimized aggregate replay
- Event distribution to projectors and sagas via gRPC/AMQP
- Command routing to the correct business logic service
- Projection coordination and event delivery guarantees

### Language Agnostic

Since Angzarr communicates via gRPC, each component can be written in whatever language makes sense. Data scientists might write projectors in Python while backend engineers implement entities in Go or Java—they interoperate seamlessly.

The `examples/` directory provides working implementations in several languages with all the gRPC/protobuf boilerplate handled. Copy an example, write your business logic, and deploy. If you find the examples can be improved, contributions are welcome.

### Behavior-Driven Development

Acceptance tests use Gherkin syntax for executable specifications:

```gherkin
Feature: Command Handling

  Scenario: Handle command with existing history
    Given prior events for aggregate "order-456" in domain "orders":
      | sequence | event_type   |
      | 0        | OrderCreated |
      | 1        | ItemAdded    |
    When I send an "AddItem" command for aggregate "order-456"
    Then the business logic receives the command with 2 prior events
    And 3 events total exist for aggregate "order-456"
```

Tests run via `cargo test` using [cucumber-rs](https://github.com/cucumber-rs/cucumber). Feature files in `tests/acceptance/features/` document system behavior and serve as living documentation.

## CLI Reference

All commands use [just](https://github.com/casey/just). Run `just` with no arguments to see available commands.

### Development Workflow

| Command | Description |
|---------|-------------|
| `just build` | Build the framework |
| `just build-release` | Build optimized release binary |
| `just build-stream` | Build angzarr-stream binary |
| `just build-gateway` | Build angzarr-gateway binary |
| `just test` | Run all unit tests |
| `just acceptance-test` | Run Gherkin acceptance tests (no containers) |
| `just run` | Start the Angzarr server |
| `just check` | Fast compile check without building |
| `just fmt` | Format code with rustfmt |
| `just lint` | Run clippy lints |

### Proto Generation

| Command | Description |
|---------|-------------|
| `just proto-generate` | Generate all language bindings (Rust, Go, Python) |
| `just proto-rust` | Generate Rust bindings only |
| `just proto-go` | Generate Go bindings only |
| `just proto-python` | Generate Python bindings only |
| `just proto-clean` | Remove generated files |

### Examples

Examples use a submodule - access them with `just examples <command>`:

| Command | Description |
|---------|-------------|
| `just examples build` | Build all example services |
| `just examples test` | Test all examples |
| `just examples fmt` | Format all examples |
| `just examples lint` | Lint all examples |
| `just examples helm-lint` | Lint all Helm charts |
| `just examples build-rust` | Build Rust examples only |
| `just examples build-go` | Build Go examples only |
| `just examples build-python` | Build Python examples only |

### Kubernetes/Kind

| Command | Description |
|---------|-------------|
| `just kind-create` | Create local Kind cluster |
| `just kind-delete` | Delete Kind cluster |
| `just deploy` | Build images, load into Kind, and deploy |
| `just redeploy` | Rebuild and redeploy (faster iteration) |
| `just undeploy` | Remove deployment |
| `just k8s-logs` | Stream Angzarr pod logs |
| `just k8s-port-forward` | Forward gRPC ports to localhost |

### Testing

| Command | Description |
|---------|-------------|
| `just test` | Run unit tests |
| `just acceptance-test` | Run in-memory acceptance tests (no containers) |
| `just integration-test` | Deploy to Kind and run integration tests |
| `just integration-test-only` | Run integration tests against already-running cluster |
| `just integration-test-streaming` | Run only streaming integration tests |
| `just integration-test-e2e` | Run only end-to-end integration tests (no streaming) |

#### Test Types

**Acceptance Tests** (`just acceptance-test`)
- Run entirely in-memory using mock storage and stub services
- Fast, no external dependencies
- Test core framework logic: command handling, event persistence, snapshots
- Feature files: `tests/acceptance/features/*.feature`

**Integration Tests** (`just integration-test-only`)
- Run against deployed Kubernetes pods via gRPC
- Test full end-to-end flow: commands → business logic → events → projectors
- Includes streaming tests for angzarr-gateway and angzarr-stream services
- Requires `just deploy` first (or use `just integration-test` to deploy and test)
- Feature files: `tests/integration/features/*.feature`
- Projector logs show actual events: `kubectl logs -n angzarr -l app=rs-projector-log-customer`

**Streaming Tests** (in `tests/integration/features/streaming.feature`)
- Test round-trip event streaming via angzarr-gateway
- Verify correlation ID propagation across events
- Test stream timeout behavior for non-matching subscriptions
- Requires angzarr-stream and angzarr-gateway services running

## Debugging and Observability

### Logging

Angzarr uses [tracing](https://docs.rs/tracing) for structured logging. Control verbosity with `ANGZARR_LOG`:

```bash
# Default: info level
just run

# Debug level for angzarr, info for dependencies
ANGZARR_LOG=angzarr=debug just run

# Trace all SQL queries
ANGZARR_LOG=sqlx=debug,angzarr=info just run

# Full trace (verbose)
ANGZARR_LOG=trace just run
```

Log output is structured JSON in production, human-readable in development:

```
2024-01-15T10:30:45.123Z  INFO angzarr: Starting angzarr server
2024-01-15T10:30:45.456Z  INFO angzarr: Storage: mock (in-memory)
2024-01-15T10:30:45.789Z  INFO angzarr: Entity listening on 0.0.0.0:1313
```

### Inspecting gRPC Services

Use [grpcurl](https://github.com/fullstorydev/grpcurl) to interact with services:

```bash
# List available services
grpcurl -plaintext localhost:1313 list

# Describe a service
grpcurl -plaintext localhost:1313 describe angzarr.BusinessCoordinator

# Send a command
grpcurl -plaintext -d '{
  "command": {
    "domain": "customer",
    "aggregate_id": "cust-001",
    "type_url": "CreateCustomer",
    "payload": "..."
  }
}' localhost:1313 angzarr.BusinessCoordinator/Handle
```

### Event Store Inspection

Query events directly via the EventQuery service:

```bash
# Get all events for an aggregate
grpcurl -plaintext -d '{
  "domain": "customer",
  "aggregate_id": "cust-001"
}' localhost:1314 angzarr.EventQuery/GetEvents

# Get events since a specific sequence
grpcurl -plaintext -d '{
  "domain": "customer",
  "aggregate_id": "cust-001",
  "from_sequence": 5
}' localhost:1314 angzarr.EventQuery/GetEvents
```

### Kubernetes Debugging

```bash
# Stream logs from Angzarr pods
just k8s-logs

# Get pod status
kubectl get pods -n angzarr

# Describe pod for events/errors
kubectl describe pod -n angzarr -l app.kubernetes.io/name=angzarr

# Port forward for local debugging
just k8s-port-forward
# Then use grpcurl against localhost:1313
```

### Common Issues

| Symptom | Cause | Fix |
|---------|-------|-----|
| "Connection refused" on startup | Business logic service not running | Start your domain service first |
| "Failed to connect to projector" | Projector not reachable | Check projector address in config.yaml |
| Events not persisting | Database path not writable | Ensure `data/` directory exists with write permissions |
| AMQP connection failed | RabbitMQ not running | Start RabbitMQ via Kind cluster or Docker |

For Kind/Podman infrastructure issues (cgroup delegation, port conflicts, cluster cleanup), see [COMMON_PROBLEMS.md](COMMON_PROBLEMS.md).

## Local Kubernetes Development

For local development with Kubernetes, Angzarr uses Kind (Kubernetes in Docker) with Podman.

### Prerequisites

All tooling is open-source and burdensome-license-free, so corporate users face no licensing risks:

- **Podman** - Container runtime (Docker-compatible, no Docker Desktop license)
- **Kind** - Local Kubernetes clusters using containers as nodes
- **kubectl** - Kubernetes CLI

### Setup

```bash
# Build images, create Kind cluster, load images, and deploy
just deploy

# For subsequent changes, use redeploy (faster)
just redeploy
```

### Just Commands

| Command | Description |
|---------|-------------|
| `just kind-create` | Create Kind cluster with port mappings for Angzarr services |
| `just kind-delete` | Delete the Kind cluster |
| `just deploy` | Full deployment: build images, load into Kind, apply manifests |
| `just deploy-with-ingress` | Full deployment with nginx-ingress controller |
| `just redeploy` | Rebuild and redeploy (faster iteration) |
| `just undeploy` | Remove deployment from cluster |
| `just ingress-install` | Install nginx-ingress controller for Kind |
| `just ingress-status` | Check ingress controller status |

### gRPC Client Helpers

| Command | Description |
|---------|-------------|
| `just grpc-list-command` | List gRPC services on entity |
| `just grpc-list-gateway` | List gRPC services on gateway |
| `just grpc-list-stream` | List gRPC services on event stream |
| `just grpc-describe-command` | Describe BusinessCoordinator service |
| `just grpc-describe-gateway` | Describe CommandGateway service |
| `just grpc-describe-stream` | Describe EventStream service |
| `just grpc-query-events DOMAIN UUID` | Query events for an aggregate |
| `just grpc-example-command DOMAIN UUID` | Send command via entity |
| `just grpc-example-gateway DOMAIN UUID` | Send command via gateway with streaming |
| `just grpc-subscribe-stream CORRELATION_ID` | Subscribe to events by correlation ID |

### Exposed Ports

The Kind cluster exposes these services to localhost via NodePort:

| Port | Service |
|------|---------|
| 50051 | Angzarr entity (gRPC) |
| 50052 | Angzarr event query (gRPC) |
| 50053 | Angzarr gateway (gRPC streaming) |
| 50054 | Angzarr stream (gRPC event subscription) |
| 5672 | RabbitMQ AMQP |
| 15672 | RabbitMQ Management UI |
| 6379 | Redis |

### Ingress Endpoints

When using `just deploy-with-ingress`, gRPC services are also available via nginx-ingress:

| Host | Service |
|------|---------|
| command.angzarr.local:80 | Entity |
| query.angzarr.local:80 | Event query |
| gateway.angzarr.local:80 | Command gateway (streaming) |
| stream.angzarr.local:80 | Event stream subscription |

Add to `/etc/hosts`:
```
127.0.0.1 command.angzarr.local query.angzarr.local gateway.angzarr.local stream.angzarr.local angzarr.local
```

Use grpcurl with ingress:
```bash
grpcurl -plaintext command.angzarr.local:80 list
grpcurl -plaintext gateway.angzarr.local:80 angzarr.CommandGateway/Execute
grpcurl -plaintext stream.angzarr.local:80 angzarr.EventStream/Subscribe
```

## Roadmap

Features to reach parity with mature frameworks like Axon:

### Aggregate Framework
- [ ] In-process aggregate hosting (entitys co-located with framework)
- [ ] Aggregate lifecycle management (creation, loading, snapshotting)
- [ ] Aggregate annotations/macros for ergonomic handler definition

### Event Upcasting
- [ ] Transform old event versions to current schema during replay
- [ ] Upcaster chain registration and execution
- [ ] Schema versioning metadata on events

### Automatic Snapshotting
- [ ] Configurable snapshot triggers (every N events, time-based)
- [ ] Snapshot scheduling policies
- [ ] Background snapshot workers

### Deadline Management
- [ ] Schedule future triggers within aggregates/sagas
- [ ] Deadline cancellation
- [ ] Persistent deadline storage with leader election

### Distributed Command Routing
- [ ] Route commands to correct node in clustered deployment
- [ ] Consistent hashing for aggregate affinity
- [ ] Service discovery integration (Consul, Kubernetes)

### Projection Management
- [ ] Replay tokens for tracking processor position
- [ ] Reset/rebuild projections from event store
- [ ] Projection status monitoring and catch-up metrics

### Subscription Queries
- [ ] Live query updates pushed to clients
- [ ] Combine initial result with streaming updates
- [ ] Query registration and lifecycle management

### Production Event Store
- [ ] PostgreSQL backend
- [ ] Event store clustering/replication
- [ ] Compaction and archival policies

### Tooling
- [ ] Admin UI for event store inspection
- [ ] Projection status dashboard
- [ ] Event replay/debugging tools

## License

SSPL (Server Side Public License)

---

## Known Limitations

### Skaffold

We would prefer to use [Skaffold](https://skaffold.dev/) for local Kubernetes development, as it provides file watching and automatic rebuilds. However, Skaffold's Kind integration uses `kind load docker-image` which doesn't work with Podman—it expects images in the Docker daemon. Until Skaffold adds native Podman+Kind support (using `kind load image-archive`), we use a custom `just deploy` workflow that handles the `podman save` → `kind load image-archive` process.
