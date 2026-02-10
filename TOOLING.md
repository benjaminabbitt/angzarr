# Development Tooling

This document covers the development tools used in Angzarr and how to get the most out of them.

## Command Runner: just

[just](https://github.com/casey/just) is the command runner for all project tasks. If you're familiar with Makefiles, `just` will feel familiar—it uses a similar syntax but is purpose-built for running commands rather than building files.

### Installation

```bash
# macOS
brew install just

# Arch Linux
pacman -S just

# Debian/Ubuntu
sudo apt install just

# Cargo (any platform)
cargo install just
```

### Usage

```bash
# List all available commands
just

# Run a specific command
just build

# Commands can have submodules
just examples build
just tofu apply-auto local
```

### Key Commands

| Command | Description |
|---------|-------------|
| `just build` | Build the framework (includes proto generation) |
| `just test` | Run unit tests |
| `just lint` | Run clippy |
| `just fmt` | Format code |
| `just check` | Fast compile check |
| `just proto rust` | Generate Rust protobuf bindings |
| `just deploy` | Full deployment to Kind cluster |
| `just dev` | Watch mode with auto-redeploy |

View any `justfile` in the repository to see what commands do—they're self-documenting with comments.

## Background Checker: bacon

[Bacon](https://github.com/Canop/bacon) is a background Rust code checker that watches for file changes and runs cargo commands automatically. It provides instant feedback without manually re-running builds.

### Why bacon over cargo watch

- **Smarter output**: Only shows errors/warnings, not successful build noise
- **Keyboard shortcuts**: Switch between check/build/test/clippy without restarting
- **Job configuration**: Project-specific jobs defined in `bacon.toml`

### Installation

```bash
cargo install bacon
```

### Usage

```bash
# Start bacon with default job (check)
bacon

# Start with a specific job
bacon clippy
bacon test
bacon build
```

### Keybindings

While bacon is running, press these keys to switch jobs:

| Key | Job |
|-----|-----|
| `c` | check |
| `b` | build |
| `t` | test |
| `l` | clippy |
| `f` | fmt |
| `d` | doc |
| `p` | proto |
| `s` | bin-standalone |

### Project Jobs

Defined in `bacon.toml`:

| Job | Description |
|-----|-------------|
| `check` | Fast compile check (default) |
| `build` | Full release build with standalone features |
| `test` | Run unit tests |
| `clippy` | Lint with warnings as errors |
| `fmt` | Format code |
| `doc` | Build documentation |
| `proto` | Regenerate protobuf bindings |

**Example-specific jobs:**

| Job | Description |
|-----|-------------|
| `ex-cart` | Check cart aggregate example |
| `ex-customer` | Check customer aggregate example |
| `ex-order` | Check order aggregate example |
| `ex-rust` | Check all Rust examples |
| `ex-go` | Build Go examples |
| `ex-python` | Lint Python examples |

**Binary jobs:**

| Job | Description |
|-----|-------------|
| `bin-standalone` | Build standalone binary |
| `bin-aggregate` | Build aggregate sidecar |
| `bin-saga` | Build saga sidecar |
| `bin-projector` | Build projector sidecar |
| `bin-gateway` | Build gateway |
| `bin-stream` | Build event stream |

### Typical Workflow

1. Start `bacon` in a terminal—leave it running
2. Edit code in your editor
3. Bacon automatically rebuilds on save
4. Fix errors as they appear
5. Press `t` to run tests, `l` for clippy

This tight feedback loop catches errors seconds after you introduce them, before context-switching away from the problem.

## Fast Builds: mold + sccache

Rust compilation can be slow. Two tools dramatically improve build times:

### mold (Fast Linker)

[mold](https://github.com/rui314/mold) is a high-performance linker that's 50-80% faster than the default linker.

**Installation:**
```bash
# Debian/Ubuntu
sudo apt install mold clang

# Fedora
sudo dnf install mold clang

# Arch Linux
pacman -S mold clang
```

**Configuration:** Already set up in `.cargo/config.toml`:
```toml
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
```

No action needed—cargo uses mold automatically on Linux.

### sccache (Compilation Cache)

[sccache](https://github.com/mozilla/sccache) caches compiled artifacts. Rebuilds that hit the cache are near-instant.

**Installation:**
```bash
cargo install sccache
```

**Configuration:** Add to your shell profile (`~/.bashrc` or `~/.zshrc`):
```bash
export RUSTC_WRAPPER=sccache
```

**Verify it's working:**
```bash
sccache --show-stats
```

### Expected Speedups

| Tool | Improvement |
|------|-------------|
| mold | 50-80% faster linking |
| sccache | Near-instant rebuilds on cache hits |
| Both | First build same speed, subsequent builds dramatically faster |

## Proto Generation

Protobuf bindings are generated for Rust, Go, and Python using a containerized build.

### Usage

```bash
# Generate bindings for a specific language
just proto rust
just proto go
just proto python

# Generate all language bindings
just proto-all

# Clean generated files
just proto-clean
```

### How It Works

Proto generation runs in a container (`angzarr-proto:latest`) that contains all necessary protoc plugins. The container is built automatically on first use:

```bash
just proto-container  # Build the proto container
```

### File Locations

| Language | Generated Files |
|----------|-----------------|
| Rust | `generated/rust/` → copied to `examples/rust/common/src/proto/` |
| Go | `generated/go/` |
| Python | `generated/python/` |

## Kubernetes Development: Skaffold + Kind

Local Kubernetes development uses Skaffold with Kind (Kubernetes in Docker) and Podman.

### Prerequisites

```bash
# Podman (Docker-compatible, no licensing issues)
# See: https://podman.io/getting-started/installation

# Kind
# See: https://kind.sigs.k8s.io/docs/user/quick-start/

# Skaffold
# See: https://skaffold.dev/docs/install/

# Helm
# See: https://helm.sh/docs/intro/install/

# kubectl
# See: https://kubernetes.io/docs/tasks/tools/
```

### One-Time Setup

```bash
# Configure Podman and Skaffold for the local registry
just skaffold-init
```

This configures:
- Podman to trust the local registry (insecure for localhost:5001)
- Skaffold default repository to point to the local registry

### Cluster Lifecycle

```bash
# Create Kind cluster with local registry
just cluster-create

# Check cluster status
just cluster-status

# Delete cluster (keeps registry)
just cluster-delete

# Delete cluster AND registry
just cluster-delete-all
```

### Deployment Commands

```bash
# Full deployment: cluster + infra + build + deploy
just deploy

# Watch mode: auto-rebuild and redeploy on changes
just dev

# Fresh deploy: regenerate protos, bust caches
just fresh-deploy

# Nuclear option: tear down everything, rebuild from scratch
just nuke-deploy
```

### How Skaffold Works

Skaffold handles the full development loop:

1. **Build**: Builds container images using Podman/Buildah
2. **Tag**: Uses content-addressable tags (git SHA) to avoid cache issues
3. **Push**: Pushes to the local registry (localhost:5001)
4. **Deploy**: Applies Helm charts to the Kind cluster

The `just dev` command runs Skaffold in watch mode—file changes trigger automatic rebuilds and redeployments.

### Port Forwarding

```bash
# Forward gateway to localhost:9084
just port-forward-gateway

# Forward topology API to localhost:9099
just port-forward-topology

# Forward Grafana to localhost:3000
just port-forward-grafana

# Kill all port-forwards
just port-forward-cleanup
```

### Infrastructure (OpenTofu)

Backing services are deployed via OpenTofu modules:

```bash
# Deploy PostgreSQL and RabbitMQ
just infra

# Destroy backing services
just infra-destroy
```

## Package Manager: Helm

[Helm](https://helm.sh/) is the package manager for Kubernetes. Angzarr uses Helm charts for all deployments.

### Installation

```bash
# macOS
brew install helm

# Debian/Ubuntu
curl https://baltocdn.com/helm/signing.asc | gpg --dearmor | sudo tee /usr/share/keyrings/helm.gpg > /dev/null
sudo apt-get install apt-transport-https --yes
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/helm.gpg] https://baltocdn.com/helm/stable/debian/ all main" | sudo tee /etc/apt/sources.list.d/helm-stable-debian.list
sudo apt-get update
sudo apt-get install helm

# From script
curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
```

### Chart Structure

```
deploy/helm/
├── angzarr/              # Main Angzarr chart
│   ├── Chart.yaml
│   ├── values.yaml       # Default values
│   ├── values-rust.yaml  # Rust example values
│   ├── values-go.yaml    # Go example values
│   ├── values-python.yaml
│   └── templates/
└── observability/        # Grafana, Prometheus, etc.
```

### Common Commands

```bash
# Lint charts before deploying
just examples helm-lint

# List releases
helm list -n angzarr

# Check release status
helm status angzarr -n angzarr

# Template locally (debug without deploying)
helm template angzarr ./deploy/helm/angzarr
```

## Dev Container

The project includes a VS Code dev container with all tools pre-installed.

### Features

- Rust toolchain with rust-analyzer
- Docker-in-Docker for Kind
- kubectl and Helm
- sccache pre-configured
- VS Code extensions pre-installed

### Usage

1. Install VS Code "Dev Containers" extension
2. Open the project folder
3. Click "Reopen in Container" when prompted

Or from command palette: "Dev Containers: Reopen in Container"

### Customization

The dev container configuration is in `.devcontainer/`:
- `devcontainer.json` - Container settings and extensions
- `Dockerfile` - Base image and tool installation
- `post-create.sh` - Post-creation setup script

## IDE Integration

### VS Code

Recommended extensions (pre-installed in dev container):
- `rust-analyzer` - Rust language support
- `Even Better TOML` - TOML syntax highlighting
- `crates` - Cargo.toml dependency management
- `vscode-lldb` - Debugger

### JetBrains (RustRover/CLion)

- Rust plugin is built-in for RustRover
- For CLion, install the Rust plugin

### Neovim

Recommended plugins:
- `rust-tools.nvim` or `rustaceanvim` - Rust support
- `nvim-lspconfig` with `rust-analyzer`

## Troubleshooting

### mold not found

```
error: linker `clang` not found
```

Install clang: `sudo apt install clang`

### sccache not being used

Verify `RUSTC_WRAPPER` is set:
```bash
echo $RUSTC_WRAPPER
# Should output: sccache
```

### bacon shows stale errors

Press `Esc` to clear and `Enter` to re-run the current job.

### Proto generation fails

The proto container builds automatically. If it fails:
```bash
just proto-container  # Rebuild the container
```

### Skaffold build fails

Ensure registry is configured:
```bash
just skaffold-init
```

### Kind cluster issues

Common fixes:
```bash
# Reset cluster
just cluster-delete-all && just cluster-create

# Check cluster status
just cluster-status

# Check pods
kubectl get pods -A
```

### Image not updating after rebuild

Skaffold uses content-addressable tags (git SHA), so this shouldn't happen. If it does:
```bash
# Force rebuild without cache
just fresh-deploy

# Or nuclear option
just nuke-deploy
```
