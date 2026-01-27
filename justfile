# Angzarr development commands

set shell := ["bash", "-c"]

# Repository root
TOP := `git rev-parse --show-toplevel`

# Use sccache for faster compilation (if installed)
export RUSTC_WRAPPER := `command -v sccache || true`

# Local registry settings (used by multiple modules)
REGISTRY_PORT := "5001"

# Import justfile modules
mod examples "examples/justfile"
mod tofu "deploy/tofu/justfile"
mod grpc "scripts/grpc.justfile"

# Import modular justfiles
mod docs "just/docs.justfile"
mod proto "just/proto.justfile"
mod cargo "just/cargo.justfile"
mod containers "just/containers.justfile"
mod test "just/test.justfile"
mod quality "just/quality.justfile"
mod cache "just/cache.justfile"
mod registry "just/registry.justfile"
mod infra "just/infra.justfile"
mod kind "just/kind.justfile"
mod skaffold "just/skaffold.justfile"
mod deployment "just/deployment.justfile"
mod secrets "just/secrets.justfile"
mod k8s "just/k8s.justfile"
mod release "just/release.justfile"

# Default recipe - show available commands
default:
    @just --list

# === Orchestration Targets ===
# These combine multiple modules for common workflows

# Build the project (debug - fast compile, includes proto generation)
build:
    just proto rust
    cargo build

# === Bacon (watch mode) ===

# Watch and check on save (default)
watch:
    bacon

# Watch and build on save
watch-build:
    bacon build

# Watch and test on save
watch-test:
    bacon test

# Watch and clippy on save
watch-clippy:
    bacon clippy

# Run integration tests
# 1. Creates Kind cluster with local registry
# 2. Deploys backing services via Terraform/Helm (PostgreSQL, RabbitMQ)
# 3. Delegates to examples/rust for Skaffold deployment and test execution
integration: _cluster-ready
    cd "{{TOP}}/examples/rust" && just integration

# Run acceptance tests (BDD cucumber tests)
# 1. Creates Kind cluster with local registry
# 2. Deploys backing services via Terraform/Helm (PostgreSQL, RabbitMQ)
# 3. Delegates to examples/rust for Skaffold deployment and test execution
acceptance: _cluster-ready
    cd "{{TOP}}/examples/rust" && just acceptance

# Full deployment: create cluster, build, and deploy via skaffold (recommended)
deploy: _cluster-ready
    cd "{{TOP}}/examples/rust" && skaffold run
    @echo "Waiting for gateway via gRPC health..."
    @uv run "{{TOP}}/scripts/wait-for-grpc-health.py" --timeout 180 --interval 5 \
        localhost:1350 || true
    @kubectl get pods -n angzarr

# Full fresh deploy: regenerate protos, clear cache, rebuild, deploy
fresh-deploy: _cluster-ready
    just proto rust
    just cache clear
    @echo "=== Fresh Deploy ==="
    cd "{{TOP}}/examples/rust" && BUILDAH_LAYERS=false skaffold run --cache-artifacts=false --force
    @echo "Waiting for services..."
    @uv run "{{TOP}}/scripts/wait-for-grpc-health.py" --timeout 180 --interval 5 localhost:1350 || true
    @kubectl get pods -n angzarr

# Full deployment with ingress controller
deploy-with-ingress: _cluster-ready
    just kind images-load-ingress
    just k8s ingress-install
    cd "{{TOP}}/examples/rust" && skaffold run
    @echo "Waiting for gateway via gRPC health..."
    @uv run "{{TOP}}/scripts/wait-for-grpc-health.py" --timeout 180 --interval 5 \
        localhost:1350 localhost:1340
    @kubectl get pods -n angzarr
    @echo ""
    @echo "Services available (via NodePort):"
    @echo "  Gateway: localhost:1350  (commands + queries)"
    @echo "  Stream:  localhost:1340  (event streaming)"
    @echo ""
    @echo "Ingress endpoints (add to /etc/hosts: 127.0.0.1 gateway.angzarr.local stream.angzarr.local):"
    @echo "  Gateway: gateway.angzarr.local:80"
    @echo "  Stream:  stream.angzarr.local:80"
    @echo ""
    @echo "Example grpcurl commands:"
    @echo "  just grpc list"
    @echo "  just grpc send-command customer <uuid>"
    @echo "  just grpc query-events customer <uuid>"

# Framework (Angzarr Sidecars) - use when you change angzarr core code
# Build order: angzarr-builder first (cargo work), then 5 final images in parallel
framework-build: _skaffold-ready
    just skaffold framework-build

# Watch and rebuild framework images on change
framework-dev: _cluster-ready
    just skaffold framework-dev

# === Internal Helpers ===

# Ensure cluster and infrastructure are ready
_cluster-ready:
    just kind create-registry
    just secrets init
    just infra local

# Ensure skaffold is configured and cluster exists
_skaffold-ready:
    just skaffold init
    just kind create-registry
