# Getting Started

## Prerequisites

- Rust 1.70+
- [Podman](https://podman.io/) or Docker (for Kubernetes development)
- [Kind](https://kind.sigs.k8s.io/) - local Kubernetes clusters
- [just](https://github.com/casey/just) - command runner (see below)
- [bacon](https://github.com/Canop/bacon) - background code checker (see below)
- [Helm](https://helm.sh/) - Kubernetes package manager
- [mold](https://github.com/rui314/mold) - fast linker (recommended)
- [sccache](https://github.com/mozilla/sccache) - compilation cache (recommended)
- grpcurl (optional, for debugging)

See [TOOLING.md](../TOOLING.md) for detailed setup instructions and usage guides.

### Fast Build Setup (Recommended)

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

---

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

---

## Build and Run

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

### Clone and Build

```bash
git clone https://github.com/yourorg/angzarr
cd angzarr

# Build the framework
just build

# Run tests to verify setup
just test
```

### Run In-Memory (Development)

The fastest path to experimentation -- no external dependencies:

```bash
# Start the server with mock storage (in-memory)
just run

# Or run acceptance tests directly
cargo test --test acceptance
```

### Standalone Mode (Local Multi-Process)

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
- `storage.type: mongodb` (tested) or [`postgres`](../src/storage/postgres/README.md) (implemented, untested)
- `messaging.type: amqp` (tested) or [`kafka`](../src/bus/kafka/README.md) (implemented, untested)
- `transport.type: tcp`

The architecture remains identical -- only the transport and infrastructure change.

### Run with Kubernetes (Production-Like)

For realistic multi-service deployments:

```bash
# Deploy Angzarr + dependencies (RabbitMQ, Redis) to Kind cluster
just deploy

# Watch logs
just k8s-logs
```

---

## Create Your First Domain

Create an entity in your preferred language. Example in Python:

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

---

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
- Test full end-to-end flow: commands -> business logic -> events -> projectors
- Includes streaming tests for angzarr-gateway and angzarr-stream services
- Requires `just deploy` first (or use `just integration-test` to deploy and test)
- Feature files: `tests/integration/features/*.feature`
- Projector logs show actual events: `kubectl logs -n angzarr -l app=rs-projector-log-customer`

**Streaming Tests** (in `tests/integration/features/streaming.feature`)
- Test round-trip event streaming via angzarr-gateway
- Verify correlation ID propagation across events
- Test stream timeout behavior for non-matching subscriptions
- Requires angzarr-stream and angzarr-gateway services running

---

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

For Kind/Podman infrastructure issues (cgroup delegation, port conflicts, cluster cleanup), see the troubleshooting section in [TOOLING.md](../TOOLING.md#troubleshooting).

---

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
