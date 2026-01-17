# Angzarr development commands

set shell := ["bash", "-c"]

# Repository root
TOP := `git rev-parse --show-toplevel`

# Import examples justfile as a module
mod examples "examples/justfile"

# Default recipe - show available commands
default:
    @just --list

# === Proto Generation ===

# Build the proto generation container
proto-container-build:
    podman build -t angzarr-proto:latest build/proto/

# Generate all proto files using container
proto-generate: proto-container-build
    podman run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        angzarr-proto:latest --all

# Generate only Rust protos
proto-rust: proto-container-build
    podman run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        angzarr-proto:latest --rust

# Generate only Python protos
proto-python: proto-container-build
    podman run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        angzarr-proto:latest --python

# Generate only Go protos
proto-go: proto-container-build
    podman run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        angzarr-proto:latest --go

# Generate only Ruby protos
proto-ruby: proto-container-build
    podman run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        angzarr-proto:latest --ruby

# Clean generated proto files
proto-clean:
    rm -rf "{{TOP}}/generated"

# === Framework Build ===

# Build the project
build:
    cargo build

# Build release (all binaries)
build-release: build-standalone build-sidecars build-infrastructure

# === Sidecar Binaries ===

# Build standalone binary (local development only, not containerized)
build-standalone:
    cargo build --release --bin angzarr-standalone --features "mode-standalone,amqp,mongodb"

# Build command sidecar binary
build-command:
    cargo build --release --bin angzarr-entity --features "mode-entity,mongodb"

# Build projector sidecar binary
build-projector:
    cargo build --release --bin angzarr-projector --features "mode-projector,mongodb"

# Build saga sidecar binary
build-saga:
    cargo build --release --bin angzarr-saga --features "mode-saga,mongodb"

# Build stream service binary (infrastructure)
build-stream:
    cargo build --release --bin angzarr-stream --features "mode-stream"

# Build gateway service binary (infrastructure)
build-gateway:
    cargo build --release --bin angzarr-gateway --features "mode-gateway"

# Build all sidecar binaries
build-sidecars: build-command build-projector build-saga

# Build all infrastructure service binaries
build-infrastructure: build-stream build-gateway

# === Sidecar Container Images ===

# Build command sidecar container image
container-build-command:
    podman build --target angzarr-entity -t angzarr-entity:latest .

# Build projector sidecar container image
container-build-projector:
    podman build --target angzarr-projector -t angzarr-projector:latest .

# Build saga sidecar container image
container-build-saga:
    podman build --target angzarr-saga -t angzarr-saga:latest .

# Build stream service container image (infrastructure)
container-build-stream:
    podman build --target angzarr-stream -t angzarr-stream:latest .

# Build gateway service container image (infrastructure)
container-build-gateway:
    podman build --target angzarr-gateway -t angzarr-gateway:latest .

# Build all sidecar container images
container-build-sidecars: container-build-command container-build-projector container-build-saga

# Build all infrastructure container images
container-build-infrastructure: container-build-stream container-build-gateway

# Run unit tests (no infrastructure required)
test:
    cargo test --lib

# Run integration tests (deploys Kind cluster, runs Rust example integration tests)
integration:
    cd "{{TOP}}/examples/rust" && just integration

# Run acceptance tests (deploys Kind cluster, runs Rust example BDD tests)
acceptance:
    cd "{{TOP}}/examples/rust" && just acceptance

# Run the standalone server (local development)
run:
    cargo run --bin angzarr-standalone --features "mode-standalone,amqp,mongodb"

# Check code
check:
    cargo check

# Format code
fmt:
    cargo fmt

# Lint code
lint:
    cargo clippy -- -D warnings

# Lint all Helm charts (main + all examples)
helm-lint:
    @echo "Linting main Helm chart..."
    helm lint "{{TOP}}/deploy/helm/angzarr"
    @echo "Linting example Helm charts..."
    just examples helm-lint

# Clean build artifacts
clean:
    cargo clean

# === Container Cache Management ===

# Show podman disk usage and cache status
cache-status:
    @echo "=== Podman System Disk Usage ==="
    podman system df
    @echo ""
    @echo "=== Build Cache Volumes ==="
    podman volume ls --filter name=buildah

# Prune unused images (keeps cache mounts)
cache-prune:
    @echo "Pruning dangling images..."
    podman image prune -f
    @echo "Pruning stopped containers..."
    podman container prune -f
    @echo ""
    podman system df

# Prune old images (older than 24h)
cache-prune-old:
    @echo "Pruning images older than 24 hours..."
    podman image prune -f --filter "until=24h"
    @echo ""
    podman system df

# Aggressive prune - removes all unused images and build caches
cache-prune-all:
    @echo "WARNING: This will remove ALL unused images and build caches."
    @echo "Press Ctrl+C within 5 seconds to cancel..."
    @sleep 5
    podman system prune -af --volumes
    @echo ""
    podman system df

# Initialize the database
init-db:
    mkdir -p data
    touch data/events.db

# === Kubernetes/Helm ===

# Load image into minikube
k8s-load-minikube:
    minikube image load angzarr:latest

# Load image into kind
k8s-load-kind:
    kind load docker-image angzarr:latest

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
    kubectl port-forward -n angzarr svc/evented 1313:1313 1314:1314

# View k8s logs
k8s-logs:
    kubectl logs -n angzarr -l app.kubernetes.io/name=evented -f

# === Skaffold Development ===

# One-time setup: configure Podman and Skaffold for local registry
skaffold-init:
    @echo "Configuring Podman for local registry..."
    @uv run "{{TOP}}/scripts/configure_podman_registry.py"
    @echo "Configuring Skaffold for Kind..."
    @uv run "{{TOP}}/scripts/configure_skaffold.py"
    @echo ""
    @echo "Setup complete. Run 'just skaffold-dev' to start developing."

# Check if Podman and Skaffold are configured
skaffold-check:
    @echo "Checking Podman registry configuration..."
    @uv run "{{TOP}}/scripts/configure_podman_registry.py" --check || true
    @echo "Checking Skaffold configuration..."
    @uv run "{{TOP}}/scripts/configure_skaffold.py" --check || true

# Build and deploy with skaffold (recommended workflow)
skaffold-dev: skaffold-init kind-create-registry secrets-init
    skaffold dev --profile local

# Build and deploy once with skaffold
skaffold-run: skaffold-init kind-create-registry secrets-init
    skaffold run --profile local

# Build only (no deploy)
skaffold-build:
    skaffold build

# Delete skaffold deployment
skaffold-delete:
    skaffold delete --profile local

# Render skaffold manifests (dry-run)
skaffold-render:
    skaffold render --profile local

# === Kind/Podman Development ===

# Image tag for local development
IMAGE_TAG := "dev"

# Local registry settings
REGISTRY_NAME := "kind-registry"
REGISTRY_PORT := "5001"

# Infrastructure images
INFRA_IMAGES := "docker.io/library/mongo:7 docker.io/library/rabbitmq:3.13-management-alpine docker.io/library/redis:7-alpine"

# Ingress controller images
INGRESS_IMAGES := "registry.k8s.io/ingress-nginx/controller:v1.12.0 registry.k8s.io/ingress-nginx/kube-webhook-certgen:v1.4.4"

# Create Kind cluster for local development (idempotent) - uses tar loading
kind-create:
    @KIND_EXPERIMENTAL_PROVIDER=podman kind get clusters 2>/dev/null | grep -q '^angzarr$' || \
        KIND_EXPERIMENTAL_PROVIDER=podman kind create cluster --config kind-config.yaml --name angzarr
    @kubectl config use-context kind-angzarr 2>/dev/null || true

# Create Kind cluster with local registry (faster image loading)
kind-create-registry:
    uv run "{{TOP}}/scripts/kind-with-registry.py"

# Show Kind cluster and registry status
kind-status:
    uv run "{{TOP}}/scripts/kind-with-registry.py" status

# Delete Kind cluster (keeps registry for reuse)
kind-delete:
    uv run "{{TOP}}/scripts/kind-with-registry.py" delete

# Delete Kind cluster and registry
kind-delete-all:
    uv run "{{TOP}}/scripts/kind-with-registry.py" delete-all

# === Local Registry Operations ===

# Push image to local registry (faster than kind load)
registry-push IMAGE:
    podman tag {{IMAGE}} localhost:{{REGISTRY_PORT}}/{{IMAGE}}
    podman push localhost:{{REGISTRY_PORT}}/{{IMAGE}} --tls-verify=false

# Push all Rust images to local registry
registry-push-rust:
    @for img in evented:{{IMAGE_TAG}} angzarr-stream:{{IMAGE_TAG}} angzarr-gateway:{{IMAGE_TAG}} \
        rs-customer:{{IMAGE_TAG}} rs-transaction:{{IMAGE_TAG}} rs-saga-loyalty:{{IMAGE_TAG}} \
        rs-projector-receipt:{{IMAGE_TAG}} rs-projector-log-customer:{{IMAGE_TAG}} rs-projector-log-transaction:{{IMAGE_TAG}}; do \
        podman tag "docker.io/library/$img" "localhost:{{REGISTRY_PORT}}/$img" && \
        podman push "localhost:{{REGISTRY_PORT}}/$img" --tls-verify=false; \
    done

# === Infrastructure Images ===

# Pull infrastructure images
images-pull-infra:
    @for img in {{INFRA_IMAGES}}; do podman pull "$img" || true; done

# Load infrastructure images into kind cluster
images-load-infra: images-pull-infra
    @for img in {{INFRA_IMAGES}}; do \
        "{{TOP}}/scripts/kind-load-images.sh" angzarr "$img"; \
    done

# Pull ingress controller images
images-pull-ingress:
    @for img in {{INGRESS_IMAGES}}; do podman pull "$img" || true; done

# Load ingress images into kind cluster
images-load-ingress: images-pull-ingress
    @for img in {{INGRESS_IMAGES}}; do \
        "{{TOP}}/scripts/kind-load-images.sh" angzarr "$img"; \
    done

# Full deployment: create cluster, build, and deploy via skaffold (recommended)
deploy: kind-create-registry secrets-init
    cd "{{TOP}}/examples/rust" && skaffold run
    @echo "Waiting for core services via gRPC health..."
    @uv run "{{TOP}}/scripts/wait-for-grpc-health.py" --timeout 180 --interval 5 \
        localhost:50051 localhost:50052 || true
    @kubectl get pods -n angzarr

# Delete deployment
undeploy:
    cd "{{TOP}}/examples/rust" && skaffold delete || true
    kubectl delete namespace angzarr --ignore-not-found=true

# Rebuild and redeploy via skaffold (handles incremental builds)
redeploy:
    cd "{{TOP}}/examples/rust" && skaffold run
    @kubectl get pods -n angzarr

# Redeploy a single Rust service (e.g., just redeploy-service customer)
redeploy-service SERVICE:
    @echo "Building {{SERVICE}}..."
    podman build --target {{SERVICE}} -t docker.io/library/rs-{{SERVICE}}:{{IMAGE_TAG}} \
        -f examples/rust/Containerfile "{{TOP}}/examples/rust"
    podman tag docker.io/library/rs-{{SERVICE}}:{{IMAGE_TAG}} localhost:{{REGISTRY_PORT}}/rs-{{SERVICE}}:{{IMAGE_TAG}}
    podman push localhost:{{REGISTRY_PORT}}/rs-{{SERVICE}}:{{IMAGE_TAG}} --tls-verify=false
    @kubectl rollout restart deployment/rs-{{SERVICE}} -n angzarr 2>/dev/null || true
    @kubectl rollout status deployment/rs-{{SERVICE}} -n angzarr --timeout=60s 2>/dev/null || true

# Helm upgrade (use after registry-push-rust)
helm-upgrade:
    helm upgrade --install angzarr "{{TOP}}/deploy/helm/angzarr" \
        -f "{{TOP}}/deploy/helm/angzarr/values-local.yaml" \
        --namespace angzarr --create-namespace

# Uninstall helm release
helm-uninstall:
    helm uninstall angzarr --namespace angzarr || true

# === Secrets Management ===

# Generate and store secure secrets (idempotent - won't overwrite existing)
secrets-init:
    uv run "{{TOP}}/scripts/manage_secrets.py" init

# Force regenerate all secrets (credential rotation)
secrets-rotate:
    uv run "{{TOP}}/scripts/manage_secrets.py" rotate

# Show current secrets (from K8s secret store, masked)
secrets-show:
    uv run "{{TOP}}/scripts/manage_secrets.py" show

# Show current secrets (full values revealed)
secrets-reveal:
    uv run "{{TOP}}/scripts/manage_secrets.py" show --reveal

# Check if secrets exist
secrets-check:
    uv run "{{TOP}}/scripts/manage_secrets.py" check

# Sync secrets to target namespace (for Bitnami charts without ESO)
secrets-sync *ARGS:
    uv run "{{TOP}}/scripts/manage_secrets.py" sync {{ARGS}}

# === External Secrets Operator ===

# Install External Secrets Operator
eso-install:
    helm repo add external-secrets https://charts.external-secrets.io || true
    helm repo update
    helm upgrade --install external-secrets external-secrets/external-secrets \
        --namespace external-secrets \
        --create-namespace \
        --set installCRDs=true \
        --wait

# Full ESO setup (install + generate secrets)
eso-setup: eso-install secrets-init

# Check ESO status
eso-status:
    @kubectl get pods -n external-secrets 2>/dev/null || echo "ESO not installed"
    @echo "---"
    @kubectl get secretstores,externalsecrets -n angzarr 2>/dev/null || echo "No ESO resources in evented namespace"

# === Service Discovery / DNS ===

# List all services with their DNS names
svc-list:
    @echo "Services in evented namespace:"
    @kubectl get svc -n angzarr -o custom-columns='NAME:.metadata.name,DNS:.metadata.annotations.evented\.io/dns-name,CLUSTER-IP:.spec.clusterIP,PORTS:.spec.ports[*].port'

# Test DNS resolution from within the cluster
svc-dns-test SERVICE:
    @kubectl run dns-test --rm -it --restart=Never --image=busybox:1.36 -n angzarr -- nslookup {{SERVICE}}.angzarr.svc.cluster.local

# Show service endpoints
svc-endpoints:
    @kubectl get endpoints -n angzarr

# === Ingress Controller ===

# Install nginx-ingress controller for Kind
ingress-install:
    kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/main/deploy/static/provider/kind/deploy.yaml
    @echo "Waiting for ingress controller to be ready..."
    kubectl wait --namespace ingress-nginx \
        --for=condition=ready pod \
        --selector=app.kubernetes.io/component=controller \
        --timeout=120s

# Check ingress status
ingress-status:
    @kubectl get pods -n ingress-nginx
    @echo "---"
    @kubectl get ingress -n angzarr

# === gRPC Health Checks ===

# Check gRPC health of all core services
grpc-health-check:
    @uv run "{{TOP}}/scripts/wait-for-grpc-health.py" --timeout 10 --interval 1 \
        localhost:50051 localhost:50052 localhost:50053 localhost:50054

# Check gRPC health of command handler only
grpc-health-command:
    @grpcurl -plaintext -d '{"service": ""}' localhost:50051 grpc.health.v1.Health/Check

# Check gRPC health of query service only
grpc-health-query:
    @grpcurl -plaintext -d '{"service": ""}' localhost:50052 grpc.health.v1.Health/Check

# Check gRPC health of gateway only
grpc-health-gateway:
    @grpcurl -plaintext -d '{"service": ""}' localhost:50053 grpc.health.v1.Health/Check

# Check gRPC health of stream only
grpc-health-stream:
    @grpcurl -plaintext -d '{"service": ""}' localhost:50054 grpc.health.v1.Health/Check

# === gRPC Client Helpers ===

# List available gRPC services via command handler
grpc-list-command:
    grpcurl -plaintext localhost:50051 list

# List available gRPC services via gateway
grpc-list-gateway:
    grpcurl -plaintext localhost:50053 list

# List available gRPC services via stream
grpc-list-stream:
    grpcurl -plaintext localhost:50054 list

# Describe BusinessCoordinator service
grpc-describe-command:
    grpcurl -plaintext localhost:50051 describe angzarr.BusinessCoordinator

# Describe CommandProxy service
grpc-describe-gateway:
    grpcurl -plaintext localhost:50053 describe angzarr.CommandProxy

# Describe EventStream service
grpc-describe-stream:
    grpcurl -plaintext localhost:50054 describe angzarr.EventStream

# Send a command via command handler (example)
grpc-example-command DOMAIN AGGREGATE_ID:
    @echo "Sending command to {{DOMAIN}}/{{AGGREGATE_ID}}..."
    grpcurl -plaintext -d '{"cover": {"domain": "{{DOMAIN}}", "root": {"value": "{{AGGREGATE_ID}}"}}, "pages": []}' \
        localhost:50051 angzarr.BusinessCoordinator/Handle

# Send a command via gateway with streaming response (example)
grpc-example-gateway DOMAIN AGGREGATE_ID:
    @echo "Sending command via gateway to {{DOMAIN}}/{{AGGREGATE_ID}} with streaming..."
    grpcurl -plaintext -d '{"cover": {"domain": "{{DOMAIN}}", "root": {"value": "{{AGGREGATE_ID}}"}}, "pages": [], "correlation_id": ""}' \
        localhost:50053 angzarr.CommandProxy/Execute

# Subscribe to events by correlation ID (example)
grpc-subscribe-stream CORRELATION_ID:
    @echo "Subscribing to events with correlation ID {{CORRELATION_ID}}..."
    grpcurl -plaintext -d '{"correlation_id": "{{CORRELATION_ID}}"}' \
        localhost:50054 angzarr.EventStream/Subscribe

# Query events for an aggregate
grpc-query-events DOMAIN AGGREGATE_ID:
    @echo "Querying events for {{DOMAIN}}/{{AGGREGATE_ID}}..."
    grpcurl -plaintext -d '{"domain": "{{DOMAIN}}", "root": {"value": "{{AGGREGATE_ID}}"}, "lower_bound": 0, "upper_bound": 0}' \
        localhost:50052 angzarr.EventQuery/GetEvents

# === Full Setup with Ingress ===

# Full deployment with ingress controller
deploy-with-ingress: kind-create-registry images-load-ingress ingress-install secrets-init
    cd "{{TOP}}/examples/rust" && skaffold run
    @echo "Waiting for all services via gRPC health..."
    @uv run "{{TOP}}/scripts/wait-for-grpc-health.py" --timeout 180 --interval 5 \
        localhost:50051 localhost:50052 localhost:50053 localhost:50054
    @kubectl get pods -n angzarr
    @echo ""
    @echo "Services available:"
    @echo "  Command (NodePort): localhost:50051"
    @echo "  Query (NodePort):   localhost:50052"
    @echo "  Gateway (NodePort): localhost:50053"
    @echo "  Stream (NodePort):  localhost:50054"
    @echo ""
    @echo "Ingress endpoints (add to /etc/hosts: 127.0.0.1 command.angzarr.local gateway.angzarr.local query.angzarr.local stream.angzarr.local angzarr.local):"
    @echo "  Command: command.angzarr.local:80"
    @echo "  Gateway: gateway.angzarr.local:80"
    @echo "  Query:   query.angzarr.local:80"
    @echo "  Stream:  stream.angzarr.local:80"
    @echo ""
    @echo "Example grpcurl commands:"
    @echo "  just grpc-list-command"
    @echo "  just grpc-list-gateway"
    @echo "  just grpc-list-stream"
    @echo "  just grpc-query-events customer <uuid>"
