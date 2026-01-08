# Evented-rs development commands

set shell := ["bash", "-c"]

# Repository root
TOP := `git rev-parse --show-toplevel`

# Default recipe - show available commands
default:
    @just --list

# === Proto Generation ===

# Build the proto generation container
proto-container-build:
    podman build -t evented-proto:latest build/proto/

# Generate all proto files using container
proto-generate: proto-container-build
    podman run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        evented-proto:latest --all

# Generate only Rust protos
proto-rust: proto-container-build
    podman run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        evented-proto:latest --rust

# Generate only Python protos
proto-python: proto-container-build
    podman run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        evented-proto:latest --python

# Generate only Go protos
proto-go: proto-container-build
    podman run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        evented-proto:latest --go

# Clean generated proto files
proto-clean:
    rm -rf "{{TOP}}/generated"

# === Framework Build ===

# Build the project
build:
    cargo build

# Build release
build-release:
    cargo build --release

# Run tests
test:
    cargo test

# Run the server
run:
    cargo run --bin evented-server

# Check code
check:
    cargo check

# Format code
fmt:
    cargo fmt

# Lint code
lint:
    cargo clippy -- -D warnings

# Clean build artifacts
clean:
    cargo clean

# Initialize the database
init-db:
    mkdir -p data
    touch data/events.db

# === Examples ===

# Build all examples
examples-build: proto-generate
    cd examples && just build

# Test all examples
examples-test: proto-generate
    cd examples && just test

# Clean all examples
examples-clean:
    cd examples && just clean

# Build Python examples
examples-python: proto-generate
    cd examples && just build-python

# Build Go examples
examples-go: proto-generate
    cd examples && just build-go

# Build Rust examples
examples-rust:
    cd examples && just build-rust

# === Integration Tests ===

# Run integration tests (starts kind cluster and deploys)
integration-test: deploy
    @echo "Waiting for services to be ready..."
    @kubectl wait --for=condition=ready pod -l app=evented -n evented --timeout=120s
    cargo test --test docker_integration

# Run integration tests without starting services (assumes already running)
integration-test-only:
    cargo test --test docker_integration

# Run acceptance tests (in-memory, no containers needed)
acceptance-test:
    cargo test --test acceptance

# === Kubernetes/Helm ===

# Load image into minikube
k8s-load-minikube:
    minikube image load evented:latest

# Load image into kind
k8s-load-kind:
    kind load docker-image evented:latest

# Deploy to local k8s with Helm
k8s-deploy-helm:
    helm upgrade --install evented deploy/helm/evented \
        -f deploy/helm/evented/values-local.yaml \
        --create-namespace \
        --namespace evented

# Uninstall Helm release
k8s-undeploy-helm:
    helm uninstall evented --namespace evented || true
    kubectl delete namespace evented || true

# Deploy with Terraform (local)
k8s-deploy-tf:
    cd deploy/terraform/local && terraform init && terraform apply -auto-approve

# Destroy Terraform deployment
k8s-undeploy-tf:
    cd deploy/terraform/local && terraform destroy -auto-approve

# Port forward evented service
k8s-port-forward:
    kubectl port-forward -n evented svc/evented 1313:1313 1314:1314

# View k8s logs
k8s-logs:
    kubectl logs -n evented -l app.kubernetes.io/name=evented -f

# === Kind/Podman Development ===

# Image tag for local development
IMAGE_TAG := "dev"

# All application images
APP_IMAGES := "evented rs-customer rs-transaction rs-saga-loyalty rs-projector-receipt rs-projector-log-customer rs-projector-log-transaction"

# Infrastructure images
INFRA_IMAGES := "docker.io/library/rabbitmq:3.13-management-alpine docker.io/library/redis:7-alpine"

# Create Kind cluster for local development (idempotent)
kind-create:
    @KIND_EXPERIMENTAL_PROVIDER=podman kind get clusters 2>/dev/null | grep -q '^evented$' || \
        KIND_EXPERIMENTAL_PROVIDER=podman kind create cluster --config kind-config.yaml --name evented

# Delete Kind cluster
kind-delete:
    KIND_EXPERIMENTAL_PROVIDER=podman kind delete cluster --name evented

# === Builders (Parallel) ===

# Build all builders in parallel (~3 min instead of ~4 min)
builders:
    #!/usr/bin/env bash
    set -e
    echo "Building all builders in parallel..."
    podman build -t docker.io/library/evented-builder:{{IMAGE_TAG}} -f examples/docker/Dockerfile.builder . &
    PID_RUST=$!
    podman build -t docker.io/library/evented-builder-go:{{IMAGE_TAG}} -f examples/docker/Dockerfile.builder-go . &
    PID_GO=$!
    podman build -t docker.io/library/evented-builder-python:{{IMAGE_TAG}} -f examples/docker/Dockerfile.builder-python . &
    PID_PYTHON=$!
    echo "Waiting for builders: Rust=$PID_RUST Go=$PID_GO Python=$PID_PYTHON"
    wait $PID_RUST && echo "Rust builder done" || exit 1
    wait $PID_GO && echo "Go builder done" || exit 1
    wait $PID_PYTHON && echo "Python builder done" || exit 1
    echo "All builders complete"

# === Rust Images ===

# Build Rust builder image (compiles all binaries once)
builder-rust:
    podman build -t docker.io/library/evented-builder:{{IMAGE_TAG}} -f examples/docker/Dockerfile.builder .

# Alias for backwards compatibility
builder: builder-rust

# Build Rust runtime images from builder (fast - just copies binaries)
images-build-rust: builder-rust
    podman build -t docker.io/library/evented:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder:{{IMAGE_TAG}} \
        --build-arg BINARY=evented-server \
        -f examples/docker/Dockerfile.runtime .
    podman build -t docker.io/library/rs-customer:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder:{{IMAGE_TAG}} \
        --build-arg BINARY=customer-server \
        -f examples/docker/Dockerfile.runtime .
    podman build -t docker.io/library/rs-transaction:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder:{{IMAGE_TAG}} \
        --build-arg BINARY=transaction-server \
        -f examples/docker/Dockerfile.runtime .
    podman build -t docker.io/library/rs-saga-loyalty:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder:{{IMAGE_TAG}} \
        --build-arg BINARY=saga-loyalty-server \
        -f examples/docker/Dockerfile.runtime .
    podman build -t docker.io/library/rs-projector-receipt:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder:{{IMAGE_TAG}} \
        --build-arg BINARY=projector-receipt-server \
        -f examples/docker/Dockerfile.runtime .
    podman build -t docker.io/library/rs-projector-log-customer:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder:{{IMAGE_TAG}} \
        --build-arg BINARY=projector-log-customer-server \
        -f examples/docker/Dockerfile.runtime .
    podman build -t docker.io/library/rs-projector-log-transaction:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder:{{IMAGE_TAG}} \
        --build-arg BINARY=projector-log-transaction-server \
        -f examples/docker/Dockerfile.runtime .

# === Go Images ===

# Build Go builder image (compiles all binaries once)
builder-go:
    podman build -t docker.io/library/evented-builder-go:{{IMAGE_TAG}} -f examples/docker/Dockerfile.builder-go .

# Build Go runtime images from builder
images-build-go: builder-go
    podman build -t docker.io/library/go-customer:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-go:{{IMAGE_TAG}} \
        --build-arg BINARY=go-customer \
        -f examples/docker/Dockerfile.runtime-go .
    podman build -t docker.io/library/go-transaction:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-go:{{IMAGE_TAG}} \
        --build-arg BINARY=go-transaction \
        -f examples/docker/Dockerfile.runtime-go .
    podman build -t docker.io/library/go-saga-loyalty:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-go:{{IMAGE_TAG}} \
        --build-arg BINARY=go-saga-loyalty \
        -f examples/docker/Dockerfile.runtime-go .
    podman build -t docker.io/library/go-projector-receipt:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-go:{{IMAGE_TAG}} \
        --build-arg BINARY=go-projector-receipt \
        -f examples/docker/Dockerfile.runtime-go .
    podman build -t docker.io/library/go-projector-log-customer:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-go:{{IMAGE_TAG}} \
        --build-arg BINARY=go-projector-log-customer \
        -f examples/docker/Dockerfile.runtime-go .
    podman build -t docker.io/library/go-projector-log-transaction:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-go:{{IMAGE_TAG}} \
        --build-arg BINARY=go-projector-log-transaction \
        -f examples/docker/Dockerfile.runtime-go .

# === Python Images ===

# Build Python builder image (creates venvs for all services)
builder-python:
    podman build -t docker.io/library/evented-builder-python:{{IMAGE_TAG}} -f examples/docker/Dockerfile.builder-python .

# Build Python runtime images from builder
images-build-python: builder-python
    podman build -t docker.io/library/py-customer:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-python:{{IMAGE_TAG}} \
        --build-arg SERVICE=customer \
        -f examples/docker/Dockerfile.runtime-python .
    podman build -t docker.io/library/py-transaction:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-python:{{IMAGE_TAG}} \
        --build-arg SERVICE=transaction \
        -f examples/docker/Dockerfile.runtime-python .
    podman build -t docker.io/library/py-saga-loyalty:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-python:{{IMAGE_TAG}} \
        --build-arg SERVICE=saga-loyalty \
        -f examples/docker/Dockerfile.runtime-python .
    podman build -t docker.io/library/py-projector-receipt:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-python:{{IMAGE_TAG}} \
        --build-arg SERVICE=projector-receipt \
        -f examples/docker/Dockerfile.runtime-python .
    podman build -t docker.io/library/py-projector-log-customer:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-python:{{IMAGE_TAG}} \
        --build-arg SERVICE=projector-log-customer \
        -f examples/docker/Dockerfile.runtime-python .
    podman build -t docker.io/library/py-projector-log-transaction:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-python:{{IMAGE_TAG}} \
        --build-arg SERVICE=projector-log-transaction \
        -f examples/docker/Dockerfile.runtime-python .

# === All Images ===

# Build all runtimes (assumes builders exist)
runtimes-rust:
    podman build -t docker.io/library/evented:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder:{{IMAGE_TAG}} \
        --build-arg BINARY=evented-server \
        -f examples/docker/Dockerfile.runtime .
    podman build -t docker.io/library/rs-customer:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder:{{IMAGE_TAG}} \
        --build-arg BINARY=customer-server \
        -f examples/docker/Dockerfile.runtime .
    podman build -t docker.io/library/rs-transaction:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder:{{IMAGE_TAG}} \
        --build-arg BINARY=transaction-server \
        -f examples/docker/Dockerfile.runtime .
    podman build -t docker.io/library/rs-saga-loyalty:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder:{{IMAGE_TAG}} \
        --build-arg BINARY=saga-loyalty-server \
        -f examples/docker/Dockerfile.runtime .
    podman build -t docker.io/library/rs-projector-receipt:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder:{{IMAGE_TAG}} \
        --build-arg BINARY=projector-receipt-server \
        -f examples/docker/Dockerfile.runtime .
    podman build -t docker.io/library/rs-projector-log-customer:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder:{{IMAGE_TAG}} \
        --build-arg BINARY=projector-log-customer-server \
        -f examples/docker/Dockerfile.runtime .
    podman build -t docker.io/library/rs-projector-log-transaction:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder:{{IMAGE_TAG}} \
        --build-arg BINARY=projector-log-transaction-server \
        -f examples/docker/Dockerfile.runtime .

runtimes-go:
    podman build -t docker.io/library/go-customer:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-go:{{IMAGE_TAG}} \
        --build-arg BINARY=go-customer \
        -f examples/docker/Dockerfile.runtime-go .
    podman build -t docker.io/library/go-transaction:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-go:{{IMAGE_TAG}} \
        --build-arg BINARY=go-transaction \
        -f examples/docker/Dockerfile.runtime-go .
    podman build -t docker.io/library/go-saga-loyalty:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-go:{{IMAGE_TAG}} \
        --build-arg BINARY=go-saga-loyalty \
        -f examples/docker/Dockerfile.runtime-go .
    podman build -t docker.io/library/go-projector-receipt:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-go:{{IMAGE_TAG}} \
        --build-arg BINARY=go-projector-receipt \
        -f examples/docker/Dockerfile.runtime-go .
    podman build -t docker.io/library/go-projector-log-customer:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-go:{{IMAGE_TAG}} \
        --build-arg BINARY=go-projector-log-customer \
        -f examples/docker/Dockerfile.runtime-go .
    podman build -t docker.io/library/go-projector-log-transaction:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-go:{{IMAGE_TAG}} \
        --build-arg BINARY=go-projector-log-transaction \
        -f examples/docker/Dockerfile.runtime-go .

runtimes-python:
    podman build -t docker.io/library/py-customer:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-python:{{IMAGE_TAG}} \
        --build-arg SERVICE=customer \
        -f examples/docker/Dockerfile.runtime-python .
    podman build -t docker.io/library/py-transaction:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-python:{{IMAGE_TAG}} \
        --build-arg SERVICE=transaction \
        -f examples/docker/Dockerfile.runtime-python .
    podman build -t docker.io/library/py-saga-loyalty:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-python:{{IMAGE_TAG}} \
        --build-arg SERVICE=saga-loyalty \
        -f examples/docker/Dockerfile.runtime-python .
    podman build -t docker.io/library/py-projector-receipt:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-python:{{IMAGE_TAG}} \
        --build-arg SERVICE=projector-receipt \
        -f examples/docker/Dockerfile.runtime-python .
    podman build -t docker.io/library/py-projector-log-customer:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-python:{{IMAGE_TAG}} \
        --build-arg SERVICE=projector-log-customer \
        -f examples/docker/Dockerfile.runtime-python .
    podman build -t docker.io/library/py-projector-log-transaction:{{IMAGE_TAG}} \
        --build-arg BUILDER_IMAGE=docker.io/library/evented-builder-python:{{IMAGE_TAG}} \
        --build-arg SERVICE=projector-log-transaction \
        -f examples/docker/Dockerfile.runtime-python .

# Build all images with parallel builders (~3 min total)
images-build-all: builders runtimes-rust runtimes-go runtimes-python

# Alias: build only Rust images (default for now)
images-build: images-build-rust

# Pull infrastructure images
images-pull-infra:
    @for img in {{INFRA_IMAGES}}; do podman pull "$img" || true; done

# Load application images into kind cluster
images-load: images-build
    @for img in {{APP_IMAGES}}; do \
        "{{TOP}}/scripts/kind-load-images.sh" evented "docker.io/library/$img:{{IMAGE_TAG}}"; \
    done

# Load infrastructure images into kind cluster
images-load-infra: images-pull-infra
    @for img in {{INFRA_IMAGES}}; do \
        "{{TOP}}/scripts/kind-load-images.sh" evented "$img"; \
    done

# Apply k8s manifests with correct image tags
k8s-apply:
    @kubectl kustomize "{{TOP}}/k8s/overlays/dev" | \
        sed 's|evented:latest|evented:{{IMAGE_TAG}}|g' | \
        sed 's|rs-customer:latest|rs-customer:{{IMAGE_TAG}}|g' | \
        sed 's|rs-transaction:latest|rs-transaction:{{IMAGE_TAG}}|g' | \
        sed 's|rs-saga-loyalty:latest|rs-saga-loyalty:{{IMAGE_TAG}}|g' | \
        sed 's|rs-projector-receipt:latest|rs-projector-receipt:{{IMAGE_TAG}}|g' | \
        sed 's|rs-projector-log-customer:latest|rs-projector-log-customer:{{IMAGE_TAG}}|g' | \
        sed 's|rs-projector-log-transaction:latest|rs-projector-log-transaction:{{IMAGE_TAG}}|g' | \
        kubectl apply -f -

# Full deployment: create cluster, build, load, and apply
deploy: kind-create images-load images-load-infra k8s-apply
    @echo "Waiting for pods to be ready..."
    @kubectl wait --for=condition=ready pod -l app=evented -n evented --timeout=180s || true
    @kubectl get pods -n evented

# Delete deployment
undeploy:
    kubectl delete namespace evented --ignore-not-found=true

# Rebuild and redeploy (faster iteration)
redeploy: images-load k8s-apply
    @kubectl rollout restart deployment -n evented
    @echo "Waiting for rollout..."
    @kubectl rollout status deployment/evented -n evented --timeout=120s || true
    @kubectl get pods -n evented
