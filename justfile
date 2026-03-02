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
REGISTRY := "ghcr.io/angzarr-io"
# Container runtime: prefer podman, fall back to docker
CONTAINER_CMD := `command -v podman 2>/dev/null || command -v docker 2>/dev/null`

mod client "client/justfile"
mod examples "examples/justfile"
mod images "build/images/justfile"
mod kind "deploy/kind/justfile"
mod tofu "deploy/tofu/justfile"

# Build images with skaffold (content-addressable tags)
# Outputs built image tags to build/images/build.json
[private]
_build-images:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{TOP}}/build/images"
    skaffold build --file-output=build.json
    echo "Built images:"
    jq -r '.builds[].tag' build.json

# Get image tag from skaffold build output
[private]
_image-tag IMAGE:
    #!/usr/bin/env bash
    BUILD_JSON="{{TOP}}/build/images/build.json"
    if [ ! -f "$BUILD_JSON" ]; then
        echo "Error: Build output not found. Run 'just _build-images' first." >&2
        exit 1
    fi
    jq -r ".builds[] | select(.imageName | contains(\"{{IMAGE}}\")) | .tag" "$BUILD_JSON"

# Run just target in container (or directly if already in devcontainer)
[private]
_container +ARGS: _build-images
    #!/usr/bin/env bash
    if [ "${DEVCONTAINER:-}" = "true" ]; then
        just --justfile "{{TOP}}/justfile.container" {{ARGS}}
    else
        IMAGE=$(just _image-tag angzarr-rust)
        {{CONTAINER_CMD}} run --rm --network=host \
            -v "{{TOP}}:/workspace:Z" \
            -v "{{TOP}}/justfile.container:/workspace/justfile:ro" \
            -w /workspace \
            -e CARGO_HOME=/workspace/.cargo-container \
            "$IMAGE" just {{ARGS}}
    fi

# Run just target in container with container socket access (for testcontainers)
[private]
_container-dind +ARGS: _build-images
    #!/usr/bin/env bash
    if [ "${DEVCONTAINER:-}" = "true" ]; then
        just --justfile "{{TOP}}/justfile.container" {{ARGS}}
    else
        IMAGE=$(just _image-tag angzarr-rust)
        # Find container socket (podman or docker)
        if command -v podman &>/dev/null; then
            SOCK="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/podman/podman.sock"
            SOCK_MSG="Start the podman socket with: systemctl --user start podman.socket"
        else
            SOCK="/var/run/docker.sock"
            SOCK_MSG="Ensure Docker daemon is running"
        fi
        if [ ! -S "$SOCK" ]; then
            echo "Error: Container socket not found at $SOCK"
            echo "$SOCK_MSG"
            exit 1
        fi
        {{CONTAINER_CMD}} run --rm --network=host \
            -v "{{TOP}}:/workspace:Z" \
            -v "{{TOP}}/justfile.container:/workspace/justfile:ro" \
            -v "$SOCK:/var/run/docker.sock:Z" \
            -w /workspace \
            -e CARGO_HOME=/workspace/.cargo-container \
            -e DOCKER_HOST=unix:///var/run/docker.sock \
            -e TESTCONTAINERS_RYUK_DISABLED=true \
            "$IMAGE" just {{ARGS}}
    fi

default:
    @just --list

# === Formatting ===

# Run command in language-specific CI image
[private]
_lang-container LANG +ARGS:
    #!/usr/bin/env bash
    if [ "${DEVCONTAINER:-}" = "true" ]; then
        {{ARGS}}
    else
        {{CONTAINER_CMD}} run --rm --network=host \
            -v "{{TOP}}:/workspace:Z" \
            -w /workspace \
            {{REGISTRY}}/angzarr-{{LANG}}:latest \
            {{ARGS}}
    fi

# Install and enable pre-commit hooks (lefthook only - formatters run in container)
hooks-install:
    @which lefthook > /dev/null || go install github.com/evilmartians/lefthook@latest
    lefthook install

# Format all code (runs in language-specific containers)
fmt-all: fmt fmt-python fmt-go fmt-csharp fmt-java fmt-cpp

# Format Python code (runs in angzarr-python container)
fmt-python:
    just _lang-container python black examples/python client/python scripts/
    just _lang-container python ruff check --fix --select I examples/python client/python scripts/

# Format Go code (runs in angzarr-go container)
fmt-go:
    just _lang-container go goimports -w examples/go client/go

# Format C# code (runs in angzarr-csharp container)
fmt-csharp:
    just _lang-container csharp csharpier format examples/csharp client/csharp

# Format Java code (runs in angzarr-java container)
fmt-java:
    just _lang-container java ./examples/java/gradlew -p examples/java spotlessApply

# Format C++ code (runs in angzarr-base container - has clang-format)
fmt-cpp:
    #!/usr/bin/env bash
    if [ "${DEVCONTAINER:-}" = "true" ]; then
        find examples/cpp client/cpp \( -name '*.cpp' -o -name '*.cc' -o -name '*.cxx' -o -name '*.hpp' -o -name '*.h' \) -exec clang-format -i {} +
    else
        {{CONTAINER_CMD}} run --rm --network=host \
            -v "$(git rev-parse --show-toplevel):/workspace:Z" \
            -w /workspace \
            {{REGISTRY}}/angzarr-base:latest \
            find examples/cpp client/cpp \( -name '*.cpp' -o -name '*.cc' -o -name '*.cxx' -o -name '*.hpp' -o -name '*.h' \) -exec clang-format -i {} +
    fi

# === Buf Schema Registry ===

# Run buf command in container (buf is installed in base image)
[private]
_buf +ARGS:
    #!/usr/bin/env bash
    if [ "${DEVCONTAINER:-}" = "true" ] || command -v buf &>/dev/null; then
        cd "{{TOP}}/proto" && buf {{ARGS}}
    else
        {{CONTAINER_CMD}} run --rm --network=host \
            -v "{{TOP}}:/workspace:Z" \
            -w /workspace/proto \
            {{REGISTRY}}/angzarr-base:latest \
            buf {{ARGS}}
    fi

# Build and validate protos with buf
buf-build:
    just _buf build

# Lint protos with buf
buf-lint:
    just _buf lint

# Push protos to Buf Schema Registry (requires: buf registry login)
buf-push:
    just _buf push

# Generate proto documentation (outputs to docs/docs/api/proto/)
# Uses podman locally, docker in CI (auto-detects)
buf-docs:
    #!/usr/bin/env bash
    set -euo pipefail
    # Auto-detect container runtime (prefer podman, fall back to docker)
    CONTAINER_CMD=${CONTAINER_CMD:-$(command -v podman 2>/dev/null || command -v docker 2>/dev/null)}
    if [ -z "$CONTAINER_CMD" ]; then
        echo "Error: neither podman nor docker found" >&2
        exit 1
    fi
    mkdir -p "{{TOP}}/docs/docs/api/proto"
    # List proto files (exclude health which is internal)
    PROTOS=$(find "{{TOP}}/proto" -name '*.proto' ! -path '*/health/*' -printf '%P\n' | sort)
    $CONTAINER_CMD run --rm \
        -v "{{TOP}}/proto:/protos:Z" \
        -v "{{TOP}}/docs/docs/api/proto:/out:Z" \
        docker.io/pseudomuto/protoc-gen-doc \
        --proto_path=/protos \
        --doc_opt=markdown,index.md \
        $PROTOS
    # Escape curly braces for MDX compatibility (handles google.api.http examples)
    python3 "{{TOP}}/build/proto/escape_mdx.py" "{{TOP}}/docs/docs/api/proto/index.md"
    # Fix anchors for Docusaurus compatibility (convert <a name=""> to heading IDs)
    python3 "{{TOP}}/build/proto/fix_anchors.py" "{{TOP}}/docs/docs/api/proto/index.md"
    # Add frontmatter for Docusaurus
    sed -i '1i ---\ntitle: Protocol Buffer API\ndescription: Auto-generated documentation for Angzarr protobuf definitions\n---\n' "{{TOP}}/docs/docs/api/proto/index.md"

# === gRPC Gateway ===

# Run command in Go container (has Go, buf, protoc plugins)
[private]
_go +ARGS:
    #!/usr/bin/env bash
    if [ "${DEVCONTAINER:-}" = "true" ] || (command -v go &>/dev/null && command -v buf &>/dev/null); then
        eval {{ARGS}}
    else
        {{CONTAINER_CMD}} run --rm --network=host \
            -v "{{TOP}}:/workspace:Z" \
            -w /workspace \
            {{REGISTRY}}/angzarr-go:latest \
            sh -c {{ARGS}}
    fi

# Generate gRPC-Gateway and OpenAPI code from protos
gateway-gen:
    just _go "cd gateway && buf generate"

# Build gRPC-Gateway binary (for local testing)
gateway-build: gateway-gen
    just _go "cd gateway && go build -o /tmp/angzarr-grpc-gateway ."

# Run gRPC-Gateway locally (connects to local coordinator)
gateway-dev: gateway-gen
    just _go "cd gateway && go run . --grpc-target=localhost:1310"

# Build gRPC-Gateway container image
gateway-image: gateway-gen
    {{CONTAINER_CMD}} build -t ghcr.io/angzarr-io/angzarr-grpc-gateway:latest -f gateway/Containerfile .

# Build and push gRPC-Gateway container image (for CI)
gateway-image-push TAG="latest": gateway-gen
    #!/usr/bin/env bash
    set -euo pipefail
    IMAGE="ghcr.io/angzarr-io/angzarr-grpc-gateway"
    {{CONTAINER_CMD}} build -t "$IMAGE:{{TAG}}" -f gateway/Containerfile .
    {{CONTAINER_CMD}} tag "$IMAGE:{{TAG}}" "$IMAGE:latest"
    {{CONTAINER_CMD}} push "$IMAGE:{{TAG}}"
    {{CONTAINER_CMD}} push "$IMAGE:latest"

# Generate OpenAPI spec and copy to docs
openapi: gateway-gen
    mkdir -p "{{TOP}}/docs/static"
    cp "{{TOP}}/gateway/api/angzarr.swagger.json" "{{TOP}}/docs/static/openapi.json"
    @echo "OpenAPI spec generated at docs/static/openapi.json"

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

# === Storage Contract Tests ===
# =============================================================================
# Storage contract tests verify that storage implementations correctly fulfill
# their trait contracts (EventStore, SnapshotStore, PositionStore).
#
# WHY: Each backend has different consistency models, failure modes, and APIs.
# A passing contract test means the backend can be swapped transparently.
#
# Usage:
#   just storage test              # All backends
#   just storage sqlite test       # SQLite only (no containers)
#   just storage postgres test     # PostgreSQL only (testcontainers)
#   just storage redis test        # Redis only (testcontainers)
#   just storage immudb test       # ImmuDB only (testcontainers)
#   just storage nats test         # NATS JetStream only (testcontainers)
# =============================================================================

# Storage contract tests - run all backends or a specific one
storage *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    args="{{ARGS}}"
    if [[ "$args" == "test" ]] || [[ -z "$args" ]]; then
        # All backends - needs dind for testcontainers
        just _container-dind storage test
    elif [[ "$args" == "sqlite test" ]]; then
        # SQLite doesn't need containers
        just _container storage sqlite test
    else
        # Other backends need testcontainers
        just _container-dind storage $args
    fi

# === Bus Contract Tests ===
# =============================================================================
# Bus contract tests verify that event bus implementations correctly fulfill
# the EventBus trait contract: publish, subscribe, acknowledge, nack, and DLQ.
#
# WHY: Event buses are the nervous system of the distributed architecture.
# Different backends have wildly different delivery semantics and failure modes.
# A passing contract test means the backend can be swapped transparently.
#
# Usage:
#   just bus test                  # All backends
#   just bus channel test          # Channel only (no containers)
#   just bus amqp test             # RabbitMQ only (testcontainers)
#   just bus kafka test            # Kafka only (testcontainers)
#   just bus pubsub test           # GCP Pub/Sub only (testcontainers)
#   just bus sns-sqs test          # AWS SNS/SQS only (testcontainers)
#   just bus nats test             # NATS JetStream only (testcontainers)
# =============================================================================

# Bus contract tests - run all backends or a specific one
bus *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    args="{{ARGS}}"
    if [[ "$args" == "test" ]] || [[ -z "$args" ]]; then
        # All backends - needs dind for testcontainers
        just _container-dind bus test
    elif [[ "$args" == "channel test" ]]; then
        # Channel doesn't need containers
        just _container bus channel test
    else
        # Other backends need testcontainers
        just _container-dind bus $args
    fi

# === Aggregate Contract Tests ===

# Run all contract tests (storage + bus)
# WHY: Complete validation before release. The "did we break anything?" check.
test-contract:
    just _container-dind test-contract

# Run all local tests (no running K8s cluster required)
# =============================================================================
# Fast validation suite using in-memory backends (no containers needed).
# Includes: unit tests, storage (SQLite), bus (channel), clients, examples.
#
# WHY: Quick feedback loop during development. Run this before committing.
# =============================================================================
test-local:
    @echo "═══════════════════════════════════════════════════════════════════"
    @echo "=== Core Unit Tests ==="
    @echo "═══════════════════════════════════════════════════════════════════"
    just test
    @echo ""
    @echo "═══════════════════════════════════════════════════════════════════"
    @echo "=== Storage Contract Tests (SQLite) ==="
    @echo "═══════════════════════════════════════════════════════════════════"
    just storage sqlite test
    @echo ""
    @echo "═══════════════════════════════════════════════════════════════════"
    @echo "=== Bus Contract Tests (Channel) ==="
    @echo "═══════════════════════════════════════════════════════════════════"
    just bus channel test
    @echo ""
    @echo "═══════════════════════════════════════════════════════════════════"
    @echo "=== Client Library Tests ==="
    @echo "═══════════════════════════════════════════════════════════════════"
    just client test-all
    @echo ""
    @echo "═══════════════════════════════════════════════════════════════════"
    @echo "=== Examples Unit Tests ==="
    @echo "═══════════════════════════════════════════════════════════════════"
    just examples test-unit
    @echo ""
    @echo "═══════════════════════════════════════════════════════════════════"
    @echo "=== All Local Tests Complete ==="
    @echo "═══════════════════════════════════════════════════════════════════"

# Run all local tests including testcontainers (requires podman socket)
# =============================================================================
# Complete validation suite testing ALL storage and bus backends.
#
# WHY: Pre-merge validation. Ensures changes haven't broken any backend.
# Takes longer but provides confidence across all deployment targets.
#
# Storage: SQLite, PostgreSQL, Redis, ImmuDB, NATS
# Bus: Channel, AMQP, Kafka, Pub/Sub, SNS/SQS, NATS
# =============================================================================
test-full: test-local
    @echo ""
    @echo "═══════════════════════════════════════════════════════════════════"
    @echo "=== All Contract Tests (testcontainers) ==="
    @echo "═══════════════════════════════════════════════════════════════════"
    just test-contract
    @echo ""
    @echo "═══════════════════════════════════════════════════════════════════"
    @echo "=== All Tests Complete ==="
    @echo "═══════════════════════════════════════════════════════════════════"

# === Cross-Language Client Tests ===
# Unified Rust Gherkin harness tests client libraries via gRPC.
# One source of truth for SDK contract testing across all languages.

# Test a specific language's client library via Rust gRPC harness
# Usage: just test-client python [--tags=@aggregate]
test-client LANG *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "═══════════════════════════════════════════════════════════════════"
    echo "=== Testing {{LANG}} client via Rust gRPC harness ==="
    echo "═══════════════════════════════════════════════════════════════════"

    # Start the language client gRPC server in background
    just client {{LANG}} serve &
    SERVER_PID=$!
    trap "kill $SERVER_PID 2>/dev/null || true" EXIT

    # Wait for server to be ready
    sleep 2

    # Run Rust Gherkin harness against the client
    ANGZARR_CLIENT_LANG={{LANG}} cargo test --test client {{ARGS}}

    echo ""
    echo "=== {{LANG}} client tests complete ==="

# Test all client libraries via Rust gRPC harness
test-clients:
    @echo "═══════════════════════════════════════════════════════════════════"
    @echo "=== Cross-Language Client Tests (All Languages) ==="
    @echo "═══════════════════════════════════════════════════════════════════"
    @echo ""
    just test-client rust || true
    @echo ""
    just test-client python || true
    @echo ""
    just test-client go || true
    @echo ""
    just test-client java || true
    @echo ""
    just test-client csharp || true
    @echo ""
    just test-client cpp || true
    @echo ""
    @echo "═══════════════════════════════════════════════════════════════════"
    @echo "=== All Cross-Language Client Tests Complete ==="
    @echo "═══════════════════════════════════════════════════════════════════"

# Clean build artifacts
clean:
    just _container clean

# === Coverage ===
# Uses cargo-llvm-cov for accurate line/branch coverage.
# Local tests (cov-*) run without docker socket.
# Contract tests (cov-contract-*, cov-full-*) require docker socket for testcontainers.

# Run unit tests with coverage
cov-unit:
    just _container cov-unit

# Run standalone integration tests with coverage
cov-integration:
    just _container cov-integration

# Run interface/Gherkin tests with coverage
cov-gherkin:
    just _container cov-gherkin

# Run all local tests with coverage (unit + integration + gherkin)
cov:
    just _container cov

# Quick terminal summary of coverage (all local tests)
cov-summary:
    just _container cov-summary

# Generate HTML coverage report (all local tests)
cov-html:
    just _container cov-html

# Generate LCOV format for CI integration
cov-lcov:
    just _container cov-lcov

# --- Contract Test Coverage (requires docker socket for testcontainers) ---

# Run PostgreSQL contract tests with coverage
cov-contract-postgres:
    just _container-dind cov-contract-postgres

# Run Redis contract tests with coverage
cov-contract-redis:
    just _container-dind cov-contract-redis

# Run AMQP bus contract tests with coverage
cov-contract-amqp:
    just _container-dind cov-contract-amqp

# Run Kafka bus contract tests with coverage
cov-contract-kafka:
    just _container-dind cov-contract-kafka

# Run all contract tests with combined coverage
cov-contracts:
    just _container-dind cov-contracts

# --- Full Coverage (all test types, requires docker socket) ---

# Run all tests with combined coverage (local + contracts)
cov-full:
    just _container-dind cov-full

# Full coverage with HTML report
cov-full-html:
    just _container-dind cov-full-html

# Full coverage summary
cov-full-summary:
    just _container-dind cov-full-summary

# Watch and check on save (host only - requires bacon)
watch:
    bacon

# === K8s Cluster ===

# Create Kind cluster
cluster-create:
    #!/usr/bin/env bash
    if kind get clusters 2>/dev/null | grep -q "^angzarr$"; then
        echo "Cluster 'angzarr' already exists"
    else
        kind create cluster --config "{{TOP}}/kind-config.yaml" --name angzarr
    fi

# Show cluster status
cluster-status:
    @kubectl cluster-info --context kind-angzarr 2>/dev/null || echo "Cluster not running"
    @echo ""
    @kubectl get nodes -o wide 2>/dev/null || true

# Delete Kind cluster
cluster-delete:
    kind delete cluster --name angzarr

# === Port Forwarding ===

# Kill all angzarr-related port-forwards
port-forward-cleanup:
    @pkill -f "kubectl.*port-forward.*angzarr" || true

# Start gateway port-forward (9084)
port-forward-gateway: port-forward-cleanup
    @kubectl port-forward --address 127.0.0.1 -n angzarr svc/angzarr-gateway 9084:9084 &
    @echo "Gateway available at localhost:9084"

# Start Grafana port-forward (3000)
port-forward-grafana:
    @pkill -f "kubectl.*port-forward.*grafana" || true
    @kubectl port-forward --address 127.0.0.1 -n observability svc/observability-grafana 3000:80 &
    @echo "Grafana available at localhost:3000"

# === Operators ===

# Install all Kubernetes operators (CloudNativePG, Strimzi, RabbitMQ)
operators: _cluster-ready
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== Installing operators ==="
    # Helm-based operators (CloudNativePG, Strimzi)
    helm dependency update "{{HELM_K8S}}/operators"
    helm upgrade --install angzarr-operators "{{HELM_K8S}}/operators" \
        -n operators --create-namespace --wait
    # RabbitMQ operator (no official Helm chart)
    just operators-rabbitmq
    echo "=== Operators installed ==="
    kubectl get pods -n operators
    kubectl get pods -n rabbitmq-system

# Install RabbitMQ Cluster Operator (no official Helm chart)
operators-rabbitmq:
    kubectl apply -f https://github.com/rabbitmq/cluster-operator/releases/download/v2.12.0/cluster-operator.yml

# Uninstall all operators
operators-delete:
    kubectl delete -f https://github.com/rabbitmq/cluster-operator/releases/download/v2.12.0/cluster-operator.yml || true
    helm uninstall angzarr-operators -n operators || true

# === Infrastructure ===

HELM_K8S := TOP + "/deploy/k8s/helm"

# Deploy lightweight infrastructure for CI (no operators, single-replica)
infra-ci:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== Deploying CI infrastructure ==="
    kubectl create namespace angzarr --dry-run=client -o yaml | kubectl apply -f -
    # Redis (simple)
    helm upgrade --install angzarr-redis "{{HELM_K8S}}/redis" \
        -n angzarr --set auth.password=angzarr --wait --timeout 2m
    # RabbitMQ (simple, no operator)
    helm upgrade --install angzarr-mq "{{HELM_K8S}}/rabbitmq-simple" \
        -n angzarr --set auth.password=angzarr --wait --timeout 2m
    echo "=== CI Infrastructure deployed ==="
    kubectl get pods -n angzarr

# Deploy infrastructure to angzarr namespace (requires operators installed first)
infra: _cluster-ready
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== Deploying infrastructure ==="
    # PostgreSQL (CloudNativePG)
    helm upgrade --install angzarr-db "{{HELM_K8S}}/postgres" \
        -n angzarr --create-namespace \
        --set auth.password=angzarr --wait
    # RabbitMQ
    helm upgrade --install angzarr-mq "{{HELM_K8S}}/rabbitmq" \
        -n angzarr \
        --set auth.password=angzarr --wait
    echo "=== Infrastructure deployed ==="
    kubectl get pods -n angzarr

# Deploy infrastructure with Kafka (alternative to RabbitMQ)
infra-kafka: _cluster-ready
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== Deploying infrastructure (Kafka) ==="
    # PostgreSQL (CloudNativePG)
    helm upgrade --install angzarr-db "{{HELM_K8S}}/postgres" \
        -n angzarr --create-namespace \
        --set auth.password=angzarr --wait
    # Kafka (Strimzi)
    helm upgrade --install angzarr-kafka "{{HELM_K8S}}/kafka" \
        -n angzarr --wait --timeout 5m
    echo "=== Infrastructure deployed ==="
    kubectl get pods -n angzarr

# Deploy Redis (optional - for snapshot store)
infra-redis:
    helm upgrade --install angzarr-redis "{{HELM_K8S}}/redis" \
        -n angzarr --create-namespace \
        --set auth.password=angzarr --wait

# Deploy NATS (optional - alternative event bus)
infra-nats:
    helm dependency update "{{HELM_K8S}}/nats"
    helm upgrade --install angzarr-nats "{{HELM_K8S}}/nats" \
        -n angzarr --create-namespace --wait

# Destroy infrastructure
infra-destroy:
    helm uninstall angzarr-nats -n angzarr || true
    helm uninstall angzarr-redis -n angzarr || true
    helm uninstall angzarr-kafka -n angzarr || true
    helm uninstall angzarr-mq -n angzarr || true
    helm uninstall angzarr-db -n angzarr || true

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

# === Vector Search ===

# Start Qdrant container for semantic search
qdrant-start:
    @mkdir -p "{{TOP}}/.vectors/qdrant-data"
    @{{CONTAINER_CMD}} start qdrant 2>/dev/null || \
        {{CONTAINER_CMD}} run -d --name qdrant \
            -p 6333:6333 -p 6334:6334 \
            -v "{{TOP}}/.vectors/qdrant-data:/qdrant/storage:Z" \
            docker.io/qdrant/qdrant:latest
    @echo "Qdrant running at http://127.0.0.1:6333"

# Stop Qdrant container
qdrant-stop:
    @{{CONTAINER_CMD}} stop qdrant 2>/dev/null || true

# Rebuild vector index for semantic codebase search (uses containerized Qdrant)
reindex: qdrant-start
    uv run "{{TOP}}/scripts/index_codebase.py" --url http://127.0.0.1:6333

# === Claude Code LSP Setup ===

# Install all supported language servers and Claude Code plugins
lsp-all: lsp-rust lsp-python lsp-go lsp-cpp lsp-java lsp-csharp
    @echo "All language servers and Claude Code plugins installed"

# Install Rust language server (rust-analyzer) and plugin
lsp-rust:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== Installing rust-analyzer ==="
    if command -v rustup &>/dev/null; then
        rustup component add rust-analyzer
    else
        echo "rustup not found, trying cargo install..."
        cargo install rust-analyzer
    fi
    echo "=== Installing Claude Code plugin ==="
    claude mcp add-from-claude-marketplace rust-analyzer-lsp || \
        echo "Plugin may already be installed or claude CLI not available"

# Install Python language server (pyright) and plugin
lsp-python:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== Installing pyright ==="
    if command -v npm &>/dev/null; then
        npm install -g pyright
    elif command -v pip &>/dev/null; then
        pip install pyright
    else
        echo "Error: npm or pip required to install pyright" >&2
        exit 1
    fi
    echo "=== Installing Claude Code plugin ==="
    claude mcp add-from-claude-marketplace pyright-lsp || \
        echo "Plugin may already be installed or claude CLI not available"

# Install Go language server (gopls) and plugin
lsp-go:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== Installing gopls ==="
    go install golang.org/x/tools/gopls@latest
    echo "=== Installing Claude Code plugin ==="
    claude mcp add-from-claude-marketplace gopls-lsp || \
        echo "Plugin may already be installed or claude CLI not available"

# Install C/C++ language server (clangd) and plugin
lsp-cpp:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== Installing clangd ==="
    if command -v apt &>/dev/null; then
        sudo apt install -y clangd
    elif command -v brew &>/dev/null; then
        brew install llvm
    elif command -v dnf &>/dev/null; then
        sudo dnf install -y clang-tools-extra
    elif command -v pacman &>/dev/null; then
        sudo pacman -S clang
    else
        echo "Error: Could not detect package manager. Install clangd manually." >&2
        exit 1
    fi
    echo "=== Installing Claude Code plugin ==="
    claude mcp add-from-claude-marketplace clangd-lsp || \
        echo "Plugin may already be installed or claude CLI not available"

# Install Java language server (jdtls) and plugin
lsp-java:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== Installing jdtls ==="
    if command -v brew &>/dev/null; then
        brew install jdtls
    elif command -v apt &>/dev/null; then
        # jdtls not in apt; guide user to manual install
        echo "jdtls not available via apt. Install manually from:"
        echo "https://download.eclipse.org/jdtls/snapshots/"
        echo "Or use VS Code's Java extension which bundles jdtls"
    else
        echo "Install jdtls manually from: https://download.eclipse.org/jdtls/snapshots/"
    fi
    echo "=== Installing Claude Code plugin ==="
    claude mcp add-from-claude-marketplace jdtls-lsp || \
        echo "Plugin may already be installed or claude CLI not available"

# Install C# language server (csharp-ls) and plugin
lsp-csharp:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== Installing csharp-ls ==="
    dotnet tool install --global csharp-ls || \
        dotnet tool update --global csharp-ls
    echo "=== Installing Claude Code plugin ==="
    claude mcp add-from-claude-marketplace csharp-lsp || \
        echo "Plugin may already be installed or claude CLI not available"

# === Internal Helpers ===

_cluster-ready:
    just cluster-create
    just secrets-init
    just operators
    just infra

_skaffold-ready:
    just skaffold-init
    just cluster-create
