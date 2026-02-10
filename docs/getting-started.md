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

Angzarr works with either **Podman** or **Docker** — they're fully compatible. All `docker` commands work identically with `podman`.

**Podman** is recommended because:
- Daemonless architecture (no background service required)
- Rootless by default (better security)
- No licensing concerns for commercial use

If you have Docker installed, everything works as-is. If you prefer Podman:

```bash
# Debian/Ubuntu
sudo apt install podman

# Fedora
sudo dnf install podman

# macOS
brew install podman
podman machine init && podman machine start

# Optional: alias docker to podman for muscle memory
alias docker=podman
```

Both Kind and Skaffold detect and use whichever runtime is available.

See [TOOLING.md](../TOOLING.md) for detailed setup instructions.

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

---

## Development Environment

### Option 1: Dev Container (Recommended)

The project includes a complete dev container configuration:

```bash
# VS Code
# 1. Install "Dev Containers" extension
# 2. Open project folder
# 3. Click "Reopen in Container" when prompted

# Or from command palette: "Dev Containers: Reopen in Container"
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
git clone https://github.com/yourorg/angzarr
cd angzarr

# Build the framework
just build

# Run unit tests
just test
```

### Deploy to Local Kubernetes

The fastest path to a running system:

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
| Gateway gRPC | 9084 | 30084 | Command gateway (the "angzarr port") |
| Stream gRPC | 1340 | 31340 | Event streaming |
| Topology REST | 9099 | - | Topology visualization API |

### Business Logic Ports

Each language gets a port block for business logic:

| Language | Range | Example Assignments |
|----------|-------|---------------------|
| Rust | 50050-50199 | order: 50035, inventory: 50025 |
| Go | 50200-50349 | order: 50203, inventory: 50204 |
| Python | 50400-50549 | order: 50303, inventory: 50304 |

See [port-conventions.md](port-conventions.md) for the full port scheme.

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

### Proto Generation

| Command | Description |
|---------|-------------|
| `just proto LANG` | Generate bindings for a language (rust, python, go) |
| `just proto-all` | Generate all language bindings |
| `just proto-clean` | Remove generated files |

### Cluster Management

| Command | Description |
|---------|-------------|
| `just cluster-create` | Create Kind cluster with local registry |
| `just cluster-status` | Show cluster and registry status |
| `just cluster-delete` | Delete Kind cluster |
| `just cluster-delete-all` | Delete cluster and registry |

### Deployment (Skaffold)

| Command | Description |
|---------|-------------|
| `just deploy` | Full deployment: cluster + infra + build + deploy |
| `just dev` | Watch mode: auto-rebuild on file changes |
| `just fresh-deploy` | Regenerate protos, bust caches, rebuild |
| `just nuke-deploy` | Tear down, delete caches, rebuild from scratch |
| `just framework-build` | Build only framework images |
| `just framework-dev` | Watch mode for framework only |

### Port Forwarding

| Command | Description |
|---------|-------------|
| `just port-forward-gateway` | Forward gateway to localhost:9084 |
| `just port-forward-topology` | Forward topology API to localhost:9099 |
| `just port-forward-grafana` | Forward Grafana to localhost:3000 |
| `just port-forward-cleanup` | Kill all angzarr port-forwards |

### Infrastructure

| Command | Description |
|---------|-------------|
| `just infra` | Deploy backing services (PostgreSQL, RabbitMQ) |
| `just infra-destroy` | Destroy backing services |
| `just secrets-init` | Initialize secrets |

### Testing

| Command | Description |
|---------|-------------|
| `just test` | Run unit tests |
| `just integration` | Run integration tests against deployed cluster |
| `just acceptance` | Run acceptance tests |

### Examples Submodule

Access example-specific commands with `just examples <command>`:

| Command | Description |
|---------|-------------|
| `just examples build` | Build all example services |
| `just examples test` | Test all examples |
| `just examples fmt` | Format all examples |
| `just examples lint` | Lint all examples |

---

## Debugging and Observability

### Logging

Angzarr uses [tracing](https://docs.rs/tracing) for structured logging. Control verbosity with `ANGZARR_LOG`:

```bash
# Debug level for angzarr, info for dependencies
ANGZARR_LOG=angzarr=debug just dev

# Trace all SQL queries
ANGZARR_LOG=sqlx=debug,angzarr=info just dev

# Full trace (verbose)
ANGZARR_LOG=trace just dev
```

### Inspecting gRPC Services

Use [grpcurl](https://github.com/fullstorydev/grpcurl) to interact with services:

```bash
# Port forward first
just port-forward-gateway

# List available services
grpcurl -plaintext localhost:9084 list

# Describe a service
grpcurl -plaintext localhost:9084 describe angzarr.CommandGateway
```

### Kubernetes Debugging

```bash
# Stream logs from all angzarr pods
kubectl logs -n angzarr -l app.kubernetes.io/part-of=angzarr -f

# Get pod status
kubectl get pods -n angzarr

# Describe pod for events/errors
kubectl describe pod -n angzarr -l app.kubernetes.io/name=angzarr-gateway
```

### Common Issues

| Symptom | Cause | Fix |
|---------|-------|-----|
| "Connection refused" on startup | Business logic service not running | Check pod logs, wait for ready |
| Events not persisting | Database not ready | Check infra pods, wait for PostgreSQL |
| Skaffold build fails | Registry not configured | Run `just skaffold-init` |
| Kind cluster issues | Stale state | Run `just cluster-delete-all && just cluster-create` |

---

## Standalone Mode

For development without Kubernetes:

```bash
# Build with standalone features (SQLite + Channel bus + UDS)
cargo build --features standalone

# Start the standalone server
cargo run --features standalone --bin angzarr_standalone
```

Standalone mode uses:
- **SQLite** for event storage (file-based or in-memory)
- **Channel event bus** for in-process pub/sub
- **Unix domain sockets** for gRPC transport

Socket files are created under `/tmp/angzarr/` by default.

---

## Next Steps

- [Command Handlers (Aggregates)](components/aggregate/aggregate.md) — Processing commands and emitting events
- [Projectors](components/projector/projectors.md) — Building read models
- [Sagas](components/saga/sagas.md) — Orchestrating workflows across aggregates
- [Process Managers](components/process-manager/process-manager.md) — Stateful multi-domain coordination
