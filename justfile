# Angzarr development commands

set shell := ["bash", "-c"]

TOP := `git rev-parse --show-toplevel`
export RUSTC_WRAPPER := `command -v sccache || true`

mod examples "examples/justfile"
mod tofu "deploy/tofu/justfile"

default:
    @just --list

# === Proto Generation ===

# Build the proto generation container
proto-container:
    podman build -t angzarr-proto:latest "{{TOP}}/build/proto/"

# Generate proto files for a language (rust, python, go)
proto LANG: proto-container
    #!/usr/bin/env bash
    set -euo pipefail
    podman run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        angzarr-proto:latest --{{LANG}}
    if [ "{{LANG}}" = "rust" ]; then
        cp "{{TOP}}/generated/rust/examples/examples.rs" \
           "{{TOP}}/examples/rust/common/src/proto/examples.rs"
    fi

# Generate all proto files
proto-all: proto-container
    podman run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        angzarr-proto:latest --all
    cp "{{TOP}}/generated/rust/examples/examples.rs" \
       "{{TOP}}/examples/rust/common/src/proto/examples.rs"

# Clean generated proto files
proto-clean:
    rm -rf "{{TOP}}/generated"

# === Build ===

# Build the project (debug, includes proto generation)
build:
    just proto rust
    cargo build

# Build release binaries
build-release:
    cargo build --release

# Check code compiles
check:
    cargo check

# Format code
fmt:
    cargo fmt

# Lint code
lint:
    cargo clippy -- -D warnings

# Run unit tests
test:
    cargo test --lib

# Clean build artifacts
clean:
    cargo clean

# Watch and check on save
watch:
    bacon

# === K8s Cluster ===

# Create Kind cluster with local registry
cluster-create:
    uv run "{{TOP}}/scripts/kind-with-registry.py"

# Show cluster status
cluster-status:
    uv run "{{TOP}}/scripts/kind-with-registry.py" status

# Delete Kind cluster
cluster-delete:
    uv run "{{TOP}}/scripts/kind-with-registry.py" delete

# Delete Kind cluster and registry
cluster-delete-all:
    uv run "{{TOP}}/scripts/kind-with-registry.py" delete-all

# === Port Forwarding ===

# Kill all angzarr-related port-forwards
port-forward-cleanup:
    @pkill -f "kubectl.*port-forward.*angzarr" || true

# Start gateway port-forward (9084)
port-forward-gateway: port-forward-cleanup
    @kubectl port-forward --address 127.0.0.1 -n angzarr svc/angzarr-gateway 9084:9084 &
    @echo "Gateway available at localhost:9084"

# Start topology port-forward (9099)
port-forward-topology: port-forward-cleanup
    @kubectl port-forward --address 127.0.0.1 -n angzarr svc/angzarr-topology 9099:9099 &
    @echo "Topology API available at localhost:9099"

# Start Grafana port-forward (3000)
port-forward-grafana:
    @pkill -f "kubectl.*port-forward.*grafana" || true
    @kubectl port-forward --address 127.0.0.1 -n observability svc/observability-grafana 3000:80 &
    @echo "Grafana available at localhost:3000"

# === Infrastructure ===

# Deploy local backing services (PostgreSQL, RabbitMQ)
infra:
    just tofu init local
    just tofu apply-auto local

# Destroy local backing services
infra-destroy:
    just tofu destroy-auto local

# Initialize secrets
secrets-init:
    uv run "{{TOP}}/scripts/manage_secrets.py" init

# === Skaffold ===

# One-time setup: configure Podman and Skaffold for local registry
skaffold-init:
    @uv run "{{TOP}}/scripts/configure_podman_registry.py"
    @uv run "{{TOP}}/scripts/configure_skaffold.py"

# Build framework images (angzarr sidecars)
framework-build: _skaffold-ready
    skaffold build

# Watch and rebuild framework images on change
framework-dev: _cluster-ready
    skaffold dev

# === Deploy (Orchestration) ===

# Full deployment: create cluster, build, deploy
deploy: _cluster-ready
    cd "{{TOP}}/examples/rust" && skaffold run
    @echo "Waiting for gateway..."
    @uv run "{{TOP}}/scripts/wait-for-grpc-health.py" --timeout 180 --interval 5 \
        localhost:9084 || true
    @kubectl get pods -n angzarr

# Watch and redeploy examples on change
dev: _cluster-ready
    cd "{{TOP}}/examples/rust" && skaffold dev

# Fresh deploy: regenerate protos, bust caches, rebuild
fresh-deploy: _cluster-ready
    just proto rust
    rm -f ~/.skaffold/cache
    cd "{{TOP}}/examples/rust" && BUILDAH_LAYERS=false skaffold run --cache-artifacts=false --force
    @echo "Waiting for services..."
    @uv run "{{TOP}}/scripts/wait-for-grpc-health.py" --timeout 180 --interval 5 \
        localhost:9084 || true
    @kubectl get pods -n angzarr

# Nuke deploy: tear down existing deployment, bust all caches, rebuild and redeploy from scratch
nuke-deploy: _cluster-ready
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== Tearing down existing deployment ==="
    cd "{{TOP}}/examples/rust" && skaffold delete || true
    cd "{{TOP}}" && skaffold delete || true

    echo "=== Busting caches ==="
    rm -f ~/.skaffold/cache

    echo "=== Regenerating protos (skipped if container build fails) ==="
    just proto rust || echo "Proto generation skipped (existing files used)"

    echo "=== Building and deploying (no cache) ==="
    cd "{{TOP}}/examples/rust" && BUILDAH_LAYERS=false skaffold run --cache-artifacts=false --force

    echo "=== Waiting for services ==="
    uv run "{{TOP}}/scripts/wait-for-grpc-health.py" --timeout 180 --interval 5 \
        localhost:9084 || true
    kubectl get pods -n angzarr

# Run integration tests
integration: _cluster-ready
    cd "{{TOP}}/examples/rust" && just integration

# Run acceptance tests
acceptance: _cluster-ready
    cd "{{TOP}}/examples/rust" && just acceptance

# === Internal Helpers ===

_cluster-ready:
    just cluster-create
    just secrets-init
    just infra

_skaffold-ready:
    just skaffold-init
    just cluster-create
