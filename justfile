# Evented-rs development commands

set shell := ["bash", "-c"]

# Repository root
TOP := `git rev-parse --show-toplevel`

# Python venv site-packages
python_venv := TOP / "examples/python/.venv"
python_site_packages := `find examples/python/.venv/lib -maxdepth 2 -type d -name site-packages 2>/dev/null | head -1`

# Default recipe - show available commands
default:
    @just --list

# Sync proto from Go evented
sync-proto:
    cp ../evented/proto/evented/evented.proto proto/evented/

# Proto container targets

# Build the proto generation container
proto-container-build:
    docker build -t evented-proto:latest build/proto/

# Generate all proto files using container
proto-generate: proto-container-build
    docker run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        evented-proto:latest --all

# Generate only Rust protos
proto-rust: proto-container-build
    docker run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        evented-proto:latest --rust

# Generate only Python protos
proto-python: proto-container-build
    docker run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        evented-proto:latest --python

# Generate only Go protos
proto-go: proto-container-build
    docker run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        evented-proto:latest --go

# Copy generated protos to their destinations
proto-install: proto-generate
    #!/usr/bin/env bash
    set -euo pipefail
    # Rust - copy to src/evented.rs location (tonic generates at build time, but we can pre-generate)
    mkdir -p "{{TOP}}/src/generated"
    cp -r "{{TOP}}/generated/rust/"* "{{TOP}}/src/generated/" 2>/dev/null || true
    # Python - copy to examples/python
    mkdir -p "{{TOP}}/examples/python/evented"
    cp -r "{{TOP}}/generated/python/evented/"* "{{TOP}}/examples/python/evented/" 2>/dev/null || true
    # Go - copy to examples/golang/business/proto
    mkdir -p "{{TOP}}/examples/golang/business/proto/evented"
    cp -r "{{TOP}}/generated/go/evented/"* "{{TOP}}/examples/golang/business/proto/evented/" 2>/dev/null || true
    echo "Proto files installed to language-specific locations"

# Clean generated proto files
proto-clean:
    rm -rf "{{TOP}}/generated"
    rm -rf "{{TOP}}/src/generated"

# Build the project
build:
    cargo build

# Build release
build-release:
    cargo build --release

# Run tests
test:
    cargo test

# Run acceptance tests
test-acceptance:
    cargo test --test acceptance

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

# Generate proto code (happens automatically via build.rs)
proto:
    cargo build

# Initialize the database
init-db:
    mkdir -p data
    touch data/events.db

# Build with Python support
build-python:
    cargo build --features python

# Setup Python example (proto + deps)
setup-python:
    cd examples/python && just setup

# Run Python tests with proper venv
test-python: setup-python
    #!/usr/bin/env bash
    set -euo pipefail
    SITE_PACKAGES=$(find "{{TOP}}/examples/python/.venv/lib" -maxdepth 2 -type d -name site-packages | head -1)
    export PYTHONPATH="{{TOP}}/examples/python:${SITE_PACKAGES}"
    cargo test --test acceptance --features python

# Run all tests (Rust + Python)
test-all: test test-python

# Setup Go example
setup-go:
    cd examples/golang && just rebuild

# Run Go acceptance tests
test-go: setup-go
    cargo test --test acceptance --features go-ffi

# Run Python discount calculator tests
test-discounts-python: setup-python
    #!/usr/bin/env bash
    set -euo pipefail
    SITE_PACKAGES=$(find "{{TOP}}/examples/python/.venv/lib" -maxdepth 2 -type d -name site-packages | head -1)
    export PYTHONPATH="{{TOP}}/examples/python:${SITE_PACKAGES}"
    export CUCUMBER_TAGS="python"
    cargo test --test acceptance --features python

# Run Go discount calculator tests
test-discounts-go: setup-go
    CUCUMBER_TAGS="go-ffi" cargo test --test acceptance --features go-ffi

# Run all discount calculator tests (Python + Go)
test-discounts: setup-python setup-go
    #!/usr/bin/env bash
    set -euo pipefail
    SITE_PACKAGES=$(find "{{TOP}}/examples/python/.venv/lib" -maxdepth 2 -type d -name site-packages | head -1)
    export PYTHONPATH="{{TOP}}/examples/python:${SITE_PACKAGES}"
    cargo test --test acceptance --features python,go-ffi

# Full setup
setup-all: build setup-python setup-go

# Docker targets

# Build Docker image
docker-build:
    docker build -t evented:latest .

# Run with Docker Compose
docker-up:
    docker compose up -d

# Stop Docker Compose
docker-down:
    docker compose down

# View Docker Compose logs
docker-logs:
    docker compose logs -f

# Clean Docker volumes
docker-clean:
    docker compose down -v

# Rebuild and restart Docker
docker-restart: docker-build
    docker compose down
    docker compose up -d

# Kubernetes/Helm targets

# Load image into minikube
k8s-load-minikube: docker-build
    minikube image load evented:latest

# Load image into kind
k8s-load-kind: docker-build
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
