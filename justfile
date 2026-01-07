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

# === Docker ===

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

# === Kubernetes/Helm ===

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
