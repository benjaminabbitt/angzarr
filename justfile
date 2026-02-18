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
    podman build --network=host -t {{IMAGE}} -f "{{TOP}}/.devcontainer/Containerfile" "{{TOP}}/.devcontainer"

# Run just target in container (or directly if already in devcontainer)
[private]
_container +ARGS: _build-image
    #!/usr/bin/env bash
    if [ "${DEVCONTAINER:-}" = "true" ]; then
        just {{ARGS}}
    else
        podman run --rm --network=host \
            -v "{{TOP}}:/workspace:Z" \
            -v "{{TOP}}/justfile.container:/workspace/justfile:ro" \
            -w /workspace \
            -e CARGO_HOME=/workspace/.cargo-container \
            {{IMAGE}} just {{ARGS}}
    fi

# Run just target in container with podman socket access (for testcontainers)
[private]
_container-dind +ARGS: _build-image
    #!/usr/bin/env bash
    if [ "${DEVCONTAINER:-}" = "true" ]; then
        just {{ARGS}}
    else
        # Find podman socket
        PODMAN_SOCK="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/podman/podman.sock"
        if [ ! -S "$PODMAN_SOCK" ]; then
            echo "Error: Podman socket not found at $PODMAN_SOCK"
            echo "Start the podman socket with: systemctl --user start podman.socket"
            exit 1
        fi
        podman run --rm --network=host \
            -v "{{TOP}}:/workspace:Z" \
            -v "{{TOP}}/justfile.container:/workspace/justfile:ro" \
            -v "$PODMAN_SOCK:/run/podman/podman.sock:Z" \
            -w /workspace \
            -e CARGO_HOME=/workspace/.cargo-container \
            -e DOCKER_HOST=unix:///run/podman/podman.sock \
            -e TESTCONTAINERS_RYUK_DISABLED=true \
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

# Run interface contract tests (SQLite - no containers needed)
test-interfaces:
    just _container test-interfaces

# Run interface tests against PostgreSQL (uses testcontainers)
test-interfaces-postgres:
    just _container-dind test-interfaces-postgres

# Run interface tests against Redis (uses testcontainers)
test-interfaces-redis:
    just _container-dind test-interfaces-redis

# Run interface tests against all backends
test-interfaces-all:
    just _container test-interfaces
    just _container-dind test-interfaces-postgres
    just _container-dind test-interfaces-redis

# === Event Bus Tests ===

# Run AMQP/RabbitMQ bus tests (uses testcontainers)
test-bus-amqp:
    just _container-dind test-bus-amqp

# Run Kafka bus tests (uses testcontainers)
test-bus-kafka:
    just _container-dind test-bus-kafka

# Run GCP Pub/Sub bus tests (uses testcontainers)
test-bus-pubsub:
    just _container-dind test-bus-pubsub

# Run AWS SNS/SQS bus tests (uses testcontainers)
test-bus-sns-sqs:
    just _container-dind test-bus-sns-sqs

# Run all bus tests
test-bus-all:
    just _container-dind test-bus-amqp
    just _container-dind test-bus-kafka
    just _container-dind test-bus-pubsub
    just _container-dind test-bus-sns-sqs

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
