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
just examples helm-lint
just proto generate
```

### Key Commands

| Command | Description |
|---------|-------------|
| `just build` | Build the framework |
| `just test` | Run unit tests |
| `just lint` | Run clippy |
| `just fmt` | Format code |
| `just check` | Fast compile check |
| `just proto generate` | Generate protobuf bindings |
| `just examples build` | Build all examples |
| `just deploy` | Deploy to Kind cluster |

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

## Proto Generation: buf + protoc

Protobuf bindings are generated for Rust, Go, and Python.

### Prerequisites

```bash
# Install buf (protobuf tooling)
# See: https://buf.build/docs/installation

# Install language-specific plugins
# Rust: prost-build (handled by build.rs)
# Go: protoc-gen-go, protoc-gen-go-grpc
# Python: grpcio-tools
```

### Usage

```bash
# Generate all language bindings
just proto generate

# Generate specific language
just proto rust
just proto go
just proto python

# Clean generated files
just proto clean
```

### File Locations

| Language | Generated Files |
|----------|-----------------|
| Rust | `src/proto/` (via build.rs) |
| Go | `examples/go/generated/` |
| Python | `examples/python/proto/` |

## Kubernetes: Kind + Helm

Local Kubernetes development uses Kind (Kubernetes in Docker) with Podman.

### Prerequisites

```bash
# Podman (Docker-compatible, no licensing issues)
# See: https://podman.io/getting-started/installation

# Kind
# See: https://kind.sigs.k8s.io/docs/user/quick-start/

# Helm
# See: https://helm.sh/docs/intro/install/

# kubectl
# See: https://kubernetes.io/docs/tasks/tools/
```

### Usage

```bash
# Create Kind cluster
just kind-create

# Deploy everything (build images, load, deploy)
just deploy

# Iterate (rebuild and redeploy)
just redeploy

# Tear down
just undeploy
just kind-delete
```

### Exposed Ports

| Port | Service |
|------|---------|
| 50051 | Aggregate (gRPC) |
| 50052 | Event query (gRPC) |
| 50053 | Gateway (gRPC streaming) |
| 50054 | Event stream (gRPC subscription) |
| 5672 | RabbitMQ AMQP |
| 15672 | RabbitMQ Management UI |
| 6379 | Redis |

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

### Installing Angzarr

```bash
# Install from OCI registry
helm install angzarr oci://ghcr.io/benjaminabbitt/charts/angzarr --version 0.1.0

# With custom values
helm install angzarr oci://ghcr.io/benjaminabbitt/charts/angzarr \
  --version 0.1.0 \
  --namespace angzarr \
  --create-namespace \
  -f values.yaml
```

### Chart Structure

```
deploy/helm/
├── angzarr/              # Main Angzarr chart
│   ├── Chart.yaml
│   ├── values.yaml
│   └── templates/
└── examples/             # Example application charts
    ├── cart/
    ├── customer/
    ├── order/
    └── ...
```

### Common Commands

```bash
# Lint charts before deploying
just examples helm-lint

# Install/upgrade a release
helm upgrade --install angzarr ./deploy/helm/angzarr -n angzarr --create-namespace

# List releases
helm list -n angzarr

# Check release status
helm status angzarr -n angzarr

# Rollback to previous release
helm rollback angzarr -n angzarr

# Uninstall
helm uninstall angzarr -n angzarr

# Template locally (debug without deploying)
helm template angzarr ./deploy/helm/angzarr
```

### Values Customization

Override defaults in `values.yaml`:

```yaml
# values.yaml
replicaCount: 3

storage:
  type: postgres
  postgres:
    host: my-postgres.example.com

messaging:
  type: kafka
  kafka:
    brokers: kafka-1:9092,kafka-2:9092
```

Apply with:
```bash
helm upgrade --install angzarr ./deploy/helm/angzarr -f values.yaml
```

## IDE Integration

### VS Code

Recommended extensions:
- `rust-analyzer` - Rust language support
- `Even Better TOML` - TOML syntax highlighting
- `vscode-proto3` - Protobuf syntax highlighting
- `crates` - Cargo.toml dependency management

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

Ensure buf is installed and in your PATH:
```bash
buf --version
```

### Kind cluster issues

See [COMMON_PROBLEMS.md](COMMON_PROBLEMS.md) for cgroup delegation, port conflicts, and cleanup issues.
