# Angzarr development commands

set shell := ["bash", "-c"]

# Repository root
TOP := `git rev-parse --show-toplevel`

# Import justfile modules
mod examples "examples/justfile"
mod terraform "deploy/terraform/justfile"
mod grpc "scripts/grpc.justfile"

# Default recipe - show available commands
default:
    @just --list

# === Documentation ===

# Render documentation from templates (updates LOC counts, etc.)
docs:
    @uv run "{{TOP}}/scripts/render_docs.py"

# Check if documentation is up to date (for CI)
docs-check:
    @uv run "{{TOP}}/scripts/render_docs.py" --check

# Show what documentation would be updated
docs-dry-run:
    @uv run "{{TOP}}/scripts/render_docs.py" --dry-run

# Show example LOC stats
docs-loc:
    @uv run "{{TOP}}/scripts/count_example_loc.py" --format markdown

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
    @echo "Syncing generated Rust protos to examples/rust/common..."
    cp "{{TOP}}/generated/rust/examples/examples.rs" "{{TOP}}/examples/rust/common/src/proto/examples.rs"

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

# Clean generated proto files
proto-clean:
    rm -rf "{{TOP}}/generated"

# === Framework Build ===

# Build the project
build: proto-rust
    cargo build

# Build release (all binaries)
build-release: build-sidecars build-infrastructure

# === Sidecar Binaries ===

# Build command sidecar binary
build-command:
    cargo build --release --bin angzarr-entity --features "mode-entity,mongodb"

# Build projector sidecar binary
build-projector:
    cargo build --release --bin angzarr-projector --features "mode-projector,mongodb"

# Build saga sidecar binary
build-saga:
    cargo build --release --bin angzarr-saga --features "mode-saga,mongodb"

# Build stream service binary (infrastructure projector)
build-stream:
    cargo build --release --bin angzarr-stream

# Build gateway service binary (infrastructure)
build-gateway:
    cargo build --release --bin angzarr-gateway

# Build all sidecar binaries
build-sidecars: build-command build-projector build-saga

# Build all infrastructure service binaries
build-infrastructure: build-stream build-gateway

# === Sidecar Container Images ===
# Multi-stage build - each target builds required stages and uses layer caching

# Build aggregate sidecar container image
container-build-aggregate:
    podman build --target angzarr-aggregate -t localhost:{{REGISTRY_PORT}}/angzarr-aggregate:latest .

# Build projector sidecar container image
container-build-projector:
    podman build --target angzarr-projector -t localhost:{{REGISTRY_PORT}}/angzarr-projector:latest .

# Build saga sidecar container image
container-build-saga:
    podman build --target angzarr-saga -t localhost:{{REGISTRY_PORT}}/angzarr-saga:latest .

# Build stream service container image
container-build-stream:
    podman build --target angzarr-stream -t localhost:{{REGISTRY_PORT}}/angzarr-stream:latest .

# Build gateway service container image
container-build-gateway:
    podman build --target angzarr-gateway -t localhost:{{REGISTRY_PORT}}/angzarr-gateway:latest .

# Build all sidecar container images
container-build-sidecars: container-build-aggregate container-build-projector container-build-saga

# Build all infrastructure container images
container-build-infrastructure: container-build-stream container-build-gateway

# Build all container images
container-build-all: container-build-sidecars container-build-infrastructure

# Run unit tests (no infrastructure required)
test:
    cargo test --lib

# Run integration tests
# 1. Creates Kind cluster with local registry
# 2. Deploys backing services via Terraform/Helm (PostgreSQL, RabbitMQ)
# 3. Delegates to examples/rust for Skaffold deployment and test execution
integration: kind-create-registry infra-local
    cd "{{TOP}}/examples/rust" && just integration

# Run acceptance tests (BDD cucumber tests)
# 1. Creates Kind cluster with local registry
# 2. Deploys backing services via Terraform/Helm (PostgreSQL, RabbitMQ)
# 3. Delegates to examples/rust for Skaffold deployment and test execution
acceptance: kind-create-registry infra-local
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

# === Registry Image Lifecycle ===

# Show registry status (image count per repository)
registry-status:
    uv run "{{TOP}}/scripts/registry_cleanup.py" status

# List all images in registry
registry-list:
    uv run "{{TOP}}/scripts/registry_cleanup.py" list

# Clean all sha256 tags (keep named tags like 'latest')
registry-clean-sha256:
    uv run "{{TOP}}/scripts/registry_cleanup.py" clean-sha256

# Delete ALL images from registry (full reset)
registry-clean-all:
    @echo "WARNING: Deleting ALL registry images in 5 seconds..."
    @sleep 5
    uv run "{{TOP}}/scripts/registry_cleanup.py" clean-all

# Run garbage collection (reclaim disk after deletes)
registry-gc:
    uv run "{{TOP}}/scripts/registry_cleanup.py" gc

# Clean sha256 tags + GC (typical cleanup)
registry-prune: registry-clean-sha256 registry-gc

# Dry run - show what clean-sha256 would delete
registry-clean-dry:
    uv run "{{TOP}}/scripts/registry_cleanup.py" clean-sha256 --dry-run

# === Infrastructure Shortcuts (Backing Services Only) ===
# These targets deploy ONLY backing services (databases, messaging).
# Application services are deployed via Skaffold in examples/ directories.
# For terraform primitives, use: just terraform <command>

# Deploy local backing services (PostgreSQL, RabbitMQ via Helm charts)
infra-local: secrets-init
    just terraform init local
    just terraform apply-auto local

# Destroy local infrastructure
infra-local-destroy:
    just terraform destroy-auto local

# Deploy staging infrastructure
infra-staging:
    just terraform init staging
    just terraform apply staging

# Destroy staging infrastructure
infra-staging-destroy:
    just terraform destroy staging

# Deploy production infrastructure
infra-prod:
    just terraform init prod
    just terraform apply prod

# Destroy production infrastructure (requires confirmation)
infra-prod-destroy:
    just terraform destroy prod

# Port forward evented service
k8s-port-forward:
    kubectl port-forward -n angzarr svc/evented 1313:1313 1314:1314

# View k8s logs
k8s-logs:
    kubectl logs -n angzarr -l app.kubernetes.io/name=evented -f

# === Skaffold Development ===
#
# UNIFIED WORKFLOW (with lefthook):
#   1. Make changes
#   2. Commit (lefthook triggers skaffold run automatically)
#   3. Done!
#
# MANUAL WORKFLOW:
#   just examples dev rust    - watch mode, rebuilds on file change
#   just examples run rust    - build and deploy once
#
# All images use sha256 content-based tags:
#   - New content = new tag = K8s pulls fresh image (even with IfNotPresent)
#   - Same content = same tag = skaffold cache hit (no rebuild)
#
# Setup lefthook: lefthook install

# One-time setup: configure Podman and Skaffold for local registry
skaffold-init:
    @echo "Configuring Podman for local registry..."
    @uv run "{{TOP}}/scripts/configure_podman_registry.py"
    @echo "Configuring Skaffold for Kind..."
    @uv run "{{TOP}}/scripts/configure_skaffold.py"
    @echo ""
    @echo "Setup complete!"

# Check if Podman and Skaffold are configured
skaffold-check:
    @echo "Checking Podman registry configuration..."
    @uv run "{{TOP}}/scripts/configure_podman_registry.py" --check || true
    @echo "Checking Skaffold configuration..."
    @uv run "{{TOP}}/scripts/configure_skaffold.py" --check || true

# === Framework (Angzarr Sidecars) ===
# Use these when you change angzarr core code (src/, proto/, Cargo.toml)
# Build order: angzarr-builder first (cargo work), then 5 final images in parallel

# Build angzarr framework images only (builder + 5 final images)
framework-build: skaffold-init kind-create-registry
    @echo "Building angzarr framework images..."
    @echo "  1. angzarr-builder (compiles all binaries)"
    @echo "  2. 5 final images in parallel (just copy binaries)"
    skaffold build
    @echo ""
    @echo "Framework images built. Now run 'just examples dev' for business logic."

# Watch and rebuild framework images on change
framework-dev: skaffold-init kind-create-registry secrets-init infra-local
    @echo "Starting framework dev loop..."
    @echo "NOTE: For business logic changes, use 'just examples dev' in another terminal."
    skaffold dev

# Build and deploy once with skaffold (framework only)
skaffold-run: skaffold-init kind-create-registry secrets-init
    skaffold run

# Delete skaffold deployment
skaffold-delete:
    skaffold delete || true

# Render skaffold manifests (dry-run)
skaffold-render:
    skaffold render

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
deploy: kind-create-registry infra-local secrets-init
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

# === Reliable Deployment (Cache-Busting) ===
# Use these targets when skaffold's incremental builds fail to pick up changes.

# Clear all build caches (skaffold artifact cache + podman layer cache)
cache-clear:
    @echo "Clearing skaffold cache..."
    rm -f ~/.skaffold/cache
    @echo "Clearing podman build cache for angzarr images..."
    podman image prune -f --filter label=angzarr=true 2>/dev/null || true
    @echo "Cache cleared."

# Force rebuild core angzarr images (no layer cache)
rebuild-core:
    @echo "Building angzarr core images (no cache)..."
    rm -f ~/.skaffold/cache
    BUILDAH_LAYERS=false skaffold build --cache-artifacts=false

# Force rebuild all images including examples (no layer cache)
rebuild-all:
    @echo "Building all images (no cache)..."
    rm -f ~/.skaffold/cache
    cd "{{TOP}}/examples/rust" && BUILDAH_LAYERS=false skaffold build --cache-artifacts=false

# Force pods to restart and pull fresh images
reload-pods:
    @echo "Restarting angzarr deployments..."
    kubectl rollout restart deployment -n angzarr -l app.kubernetes.io/component=aggregate 2>/dev/null || true
    kubectl rollout restart deployment -n angzarr -l app.kubernetes.io/component=saga 2>/dev/null || true
    kubectl rollout restart deployment -n angzarr angzarr-gateway 2>/dev/null || true
    kubectl rollout restart deployment -n angzarr angzarr-stream 2>/dev/null || true
    @echo "Waiting for rollouts..."
    kubectl rollout status deployment -n angzarr -l app.kubernetes.io/component=aggregate --timeout=120s 2>/dev/null || true
    kubectl rollout status deployment -n angzarr angzarr-gateway --timeout=60s 2>/dev/null || true

# Full fresh deploy: regenerate protos, clear cache, rebuild, deploy
fresh-deploy: proto-rust cache-clear
    @echo "=== Fresh Deploy ==="
    cd "{{TOP}}/examples/rust" && BUILDAH_LAYERS=false skaffold run --cache-artifacts=false --force
    @echo "Waiting for services..."
    @uv run "{{TOP}}/scripts/wait-for-grpc-health.py" --timeout 180 --interval 5 localhost:50051 || true
    @kubectl get pods -n angzarr

# Quick redeploy: rebuild with cache, force helm upgrade
quick-deploy:
    cd "{{TOP}}/examples/rust" && skaffold run --force
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

# Helm upgrade
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

# === Full Setup with Ingress ===

# Full deployment with ingress controller
deploy-with-ingress: kind-create-registry infra-local images-load-ingress ingress-install secrets-init
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
    @echo "  just grpc list-command"
    @echo "  just grpc list-gateway"
    @echo "  just grpc list-stream"
    @echo "  just grpc query-events customer <uuid>"
