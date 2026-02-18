# Angzarr development commands
#
# Container Overlay Pattern:
# --------------------------
# This justfile uses an overlay pattern for container execution:
#
# 1. `justfile` (this file) - runs on the host, delegates to container
# 2. `justfile.container` - mounted over this file inside the container
#
# When running outside a devcontainer:
#   - Builds/uses local devcontainer image with `just` pre-installed
#   - Podman mounts justfile.container as /workspace/justfile
#
# When running inside a devcontainer (DEVCONTAINER=true):
#   - Commands execute directly via `just <target>`
#   - No container nesting

set shell := ["bash", "-c"]

TOP := `git rev-parse --show-toplevel`
IMAGE := "angzarr-dev"

mod client "client/justfile"
mod examples "examples/justfile"
mod tofu "deploy/tofu/justfile"

# Build the devcontainer image
[private]
_build-image:
    podman build -t {{IMAGE}} -f "{{TOP}}/.devcontainer/Containerfile" "{{TOP}}/.devcontainer"

# Run just target in container (or directly if already in devcontainer)
[private]
_container +ARGS: _build-image
    #!/usr/bin/env bash
    if [ "${DEVCONTAINER:-}" = "true" ]; then
        just {{ARGS}}
    else
        podman run --rm \
            -v "{{TOP}}:/workspace:Z" \
            -v "{{TOP}}/justfile.container:/workspace/justfile:ro" \
            -w /workspace \
            -e CARGO_HOME=/workspace/.cargo-container \
            {{IMAGE}} just {{ARGS}}
    fi

default:
    @just --list

# === Buf Schema Registry ===

# Build and validate protos with buf
buf-build:
    cd "{{TOP}}/proto" && buf build

# Lint protos with buf
buf-lint:
    cd "{{TOP}}/proto" && buf lint

# Push protos to Buf Schema Registry (requires: buf registry login)
buf-push:
    cd "{{TOP}}/proto" && buf push

# === Build ===

# Build the project (debug)
build:
    just _container build

# Build release binaries
build-release:
    just _container build-release

# Check code compiles
check:
    just _container check

# Format code
fmt:
    just _container fmt

# Lint code
lint:
    just _container lint

# Run unit tests
test:
    just _container test

# Clean build artifacts
clean:
    just _container clean

# Watch and check on save (host only - requires bacon)
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

# Nuke deploy: tear down existing deployment, bust all caches, rebuild and redeploy from scratch
nuke-deploy: _cluster-ready
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== Tearing down existing deployment ==="
    cd "{{TOP}}/examples/rust" && skaffold delete || true
    cd "{{TOP}}" && skaffold delete || true

    echo "=== Busting caches ==="
    rm -f ~/.skaffold/cache

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
