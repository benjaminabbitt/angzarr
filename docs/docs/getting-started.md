---
sidebar_position: 3
---

# Getting Started

## Prerequisites

- Rust 1.70+
- Container runtime: [Podman](https://podman.io/) or [Docker](https://www.docker.com/)
- [Kind](https://kind.sigs.k8s.io/) - local Kubernetes clusters
- [Skaffold](https://skaffold.dev/) - Kubernetes development workflow
- [just](https://github.com/casey/just) - command runner
- [Helm](https://helm.sh/) - Kubernetes package manager
- [uv](https://docs.astral.sh/uv/) - Python package manager (for scripts)
- [mold](https://github.com/rui314/mold) - fast linker (recommended)
- [sccache](https://github.com/mozilla/sccache) - compilation cache (recommended)
- grpcurl (optional, for debugging)

### Container Runtime: Podman or Docker

Angzarr works with either **Podman** or **Docker** — they're fully compatible.

**Podman** is recommended because:
- Daemonless architecture (no background service required)
- Rootless by default (better security)
- No licensing concerns for commercial use

```bash
# Debian/Ubuntu
sudo apt install podman

# Fedora
sudo dnf install podman

# macOS
brew install podman
podman machine init && podman machine start

# Optional: alias docker to podman
alias docker=podman
```

### Fast Build Setup (Recommended)

Install mold and sccache for significantly faster builds:

```bash
# Debian/Ubuntu
sudo apt install mold clang

# Fedora
sudo dnf install mold clang

# Install sccache
cargo install sccache

# Enable sccache (add to ~/.bashrc or ~/.zshrc)
export RUSTC_WRAPPER=sccache
```

**Expected speedups:**
- mold linker: 50-80% faster linking
- sccache: Near-instant rebuilds on cache hits

---

## Development Environment

### Option 1: Dev Container (Recommended)

The project includes a complete dev container configuration:

```bash
# VS Code
# 1. Install "Dev Containers" extension
# 2. Open project folder
# 3. Click "Reopen in Container" when prompted
```

The dev container includes:
- Rust toolchain with rust-analyzer
- Container runtime (Docker-in-Docker) for Kind
- kubectl and Helm
- sccache pre-configured
- All VS Code extensions pre-installed

### Option 2: Local Setup

```bash
# Install just
cargo install just

# Install other tools (see Prerequisites above)
```

---

## Quick Start

### Clone and Build

```bash
git clone https://github.com/angzarr/angzarr
cd angzarr

# Build the framework
just build

# Run unit tests
just test
```

### Deploy to Local Kubernetes

```bash
# Full deployment: create cluster, build images, deploy via Skaffold
just deploy

# Watch pods come up
kubectl get pods -n angzarr -w
```

This creates a Kind cluster with a local registry, builds all images via Skaffold, and deploys the Rust example application.

### Development Workflow

```bash
# Watch mode: auto-rebuild and redeploy on file changes
just dev

# Or for faster iteration without file watching:
just deploy  # After making changes
```

### Clean Slate

```bash
# Tear down and rebuild everything from scratch
just nuke-deploy
```

---

## Port Standards

### Infrastructure Ports

| Service | Port | NodePort | Description |
|---------|------|----------|-------------|
| Aggregate Coordinator | 1310 | 31310 | Command handling per domain |
| Stream gRPC | 1340 | 31340 | Event streaming |
| Topology REST | 9099 | - | Topology visualization API |

### Business Logic Ports

Each language gets a port block:

| Language | Range | Example Assignments |
|----------|-------|---------------------|
| Rust | 50050-50199 | order: 50080, inventory: 50070 |
| Go | 50200-50349 | order: 50203, inventory: 50204 |
| Python | 50400-50549 | order: 50403, inventory: 50404 |

See [Port Conventions](./reference/port-conventions) for the full scheme.

---

## CLI Reference

All commands use [just](https://github.com/casey/just). Run `just` with no arguments to see available commands.

### Core Commands

| Command | Description |
|---------|-------------|
| `just build` | Build the framework (includes proto generation) |
| `just check` | Fast compile check |
| `just test` | Run unit tests |
| `just fmt` | Format code |
| `just lint` | Run clippy lints |
| `just watch` | Start bacon (background code checker) |

### Cluster Management

| Command | Description |
|---------|-------------|
| `just cluster-create` | Create Kind cluster with local registry |
| `just cluster-status` | Show cluster and registry status |
| `just cluster-delete` | Delete Kind cluster |
| `just nuke-deploy` | Delete cluster, caches, rebuild from scratch |

### Deployment (Skaffold)

| Command | Description |
|---------|-------------|
| `just deploy` | Full deployment: cluster + infra + build + deploy |
| `just dev` | Watch mode: auto-rebuild on file changes |
| `just fresh-deploy` | Regenerate protos, bust caches, rebuild |

### Testing

| Command | Description |
|---------|-------------|
| `just test` | Run unit tests |
| `just integration` | Run integration tests |
| `just acceptance` | Run acceptance tests |

---

## Debugging and Observability

### Logging

Angzarr uses [tracing](https://docs.rs/tracing) for structured logging:

```bash
# Debug level for angzarr, info for dependencies
ANGZARR_LOG=angzarr=debug just dev

# Trace all SQL queries
ANGZARR_LOG=sqlx=debug,angzarr=info just dev
```

### Inspecting gRPC Services

```bash
# Port forward an aggregate service
kubectl port-forward svc/angzarr-order 1310:1310 -n angzarr &

# List available services
grpcurl -plaintext localhost:1310 list

# Describe a service
grpcurl -plaintext localhost:1310 describe angzarr.AggregateCoordinator
```

### Common Issues

| Symptom | Cause | Fix |
|---------|-------|-----|
| "Connection refused" | Business logic not running | Check pod logs, wait for ready |
| Events not persisting | Database not ready | Check infra pods |
| Skaffold build fails | Registry not configured | Run `just skaffold-init` |
| Kind cluster issues | Stale state | Run `just nuke-deploy` |

---

## Standalone Mode

For development without Kubernetes:

```bash
# Build with standalone features
cargo build --features standalone

# Start the standalone server
cargo run --features standalone --bin angzarr_standalone
```

Standalone mode uses:
- **SQLite** for event storage
- **Channel event bus** for in-process pub/sub
- **Unix domain sockets** for gRPC transport

---

## Configuration Reference

### Storage Backends

| Backend | Feature Flag | Use Case |
|---------|--------------|----------|
| **SQLite** | `sqlite` | Standalone dev, testing |
| **PostgreSQL** | `postgres` | Production |
| **Redis** | `redis` | High-throughput |
| **EventStoreDB** | `eventstoredb` | Event-native storage |

```yaml
storage:
  type: postgres
  connection_string: "postgres://user:pass@localhost:5432/angzarr"
```

### Messaging Backends

| Backend | Feature Flag | Use Case |
|---------|--------------|----------|
| **AMQP** | `amqp` | Production (RabbitMQ) |
| **Kafka** | `kafka` | High-throughput |
| **Channel** | `channel` | In-process, standalone |

```yaml
messaging:
  type: amqp
  url: "amqp://guest:guest@localhost:5672"
```

---

## Next Steps

- **[Architecture](./architecture)** — Core concepts and patterns
- **[Aggregates](./components/aggregate)** — Command handling
- **[Testing](./operations/testing)** — Three-level testing strategy
- **[Tooling](./tooling/just)** — Development tools in depth
