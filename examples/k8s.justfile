# Kubernetes mode operations for angzarr examples
# Deploys services to Kind cluster via Skaffold
#
# Usage:
#   just examples k8s deploy rust        # Deploy Rust examples to K8s
#   just examples k8s test rust          # Run acceptance tests against K8s
#   just examples k8s dev rust           # Watch mode with auto-rebuild
#   just examples k8s delete rust        # Remove deployment

set shell := ["bash", "-c"]

TOP := `git rev-parse --show-toplevel`

# Show available commands
default:
    @just --list

# ============================================================================
# Prerequisites
# ============================================================================

# Ensure Kind cluster and infrastructure are ready
[private]
ensure-cluster:
    cd {{TOP}} && just kind-create-registry
    cd {{TOP}} && just infra-local

# ============================================================================
# Deployment Targets
# ============================================================================

# Deploy examples to K8s cluster
deploy LANG: ensure-cluster
    cd "{{TOP}}/examples/{{LANG}}" && skaffold run
    @echo "Waiting for pods to be ready..."
    @kubectl wait --for=condition=ready pod -l app.kubernetes.io/component=aggregate -n angzarr --timeout=300s || true
    @kubectl get pods -n angzarr

# Start development mode with file watching
dev LANG: ensure-cluster
    cd "{{TOP}}/examples/{{LANG}}" && skaffold dev

# Build images only (no deploy)
build LANG:
    cd "{{TOP}}/examples/{{LANG}}" && skaffold build

# Delete deployment
delete LANG:
    cd "{{TOP}}/examples/{{LANG}}" && skaffold delete || true

# ============================================================================
# Test Targets
# ============================================================================

# Run acceptance tests against K8s deployment
test LANG: (deploy LANG)
    #!/usr/bin/env bash
    set -e
    cd "{{TOP}}"

    # Wait for gateway to be ready
    echo "Waiting for gateway to be ready..."
    uv run "{{TOP}}/scripts/wait-for-grpc-health.py" --timeout 180 --interval 5 localhost:1350 || {
        echo "Gateway not ready, checking pods..."
        kubectl get pods -n angzarr
        exit 1
    }

    # Run acceptance tests based on language
    echo "Running acceptance tests for {{LANG}} against K8s..."
    case "{{LANG}}" in
        rust)
            cd "{{TOP}}/examples/rust"
            ANGZARR_PORT=1350 cargo test --workspace --test acceptance || TEST_RESULT=$?
            ;;
        python)
            cd "{{TOP}}/examples/python/tests"
            ANGZARR_PORT=1350 ANGZARR_TEST_MODE=container uv run pytest -v || TEST_RESULT=$?
            ;;
        go)
            cd "{{TOP}}/examples/go"
            ANGZARR_PORT=1350 go test -v ./... || TEST_RESULT=$?
            ;;
    esac

    exit ${TEST_RESULT:-0}

# Run integration tests (technical, not BDD)
integration LANG: (deploy LANG)
    #!/usr/bin/env bash
    set -e
    cd "{{TOP}}"

    echo "Waiting for gateway to be ready..."
    uv run "{{TOP}}/scripts/wait-for-grpc-health.py" --timeout 180 --interval 5 localhost:1350 || {
        echo "Gateway not ready"
        exit 1
    }

    echo "Running integration tests for {{LANG}}..."
    ANGZARR_PORT=1350 TEST_LANGUAGE={{LANG}} cargo test --test container_integration --manifest-path "{{TOP}}/Cargo.toml"

# ============================================================================
# Multi-Language Operations
# ============================================================================

# Deploy all languages
deploy-all: (deploy "rust") (deploy "python") (deploy "go")
    @echo "All languages deployed!"

# Test all languages
test-all: (test "rust") (test "python") (test "go")
    @echo "All K8s tests complete!"

# Delete all deployments
delete-all: (delete "rust") (delete "python") (delete "go")
    @echo "All deployments deleted!"

# ============================================================================
# Utility Targets
# ============================================================================

# Show pod status
status:
    @kubectl get pods -n angzarr
    @echo ""
    @kubectl get svc -n angzarr

# View logs for a specific component
logs COMPONENT:
    kubectl logs -n angzarr -l app.kubernetes.io/component={{COMPONENT}} -f --tail=100

# Port forward gateway (if not using NodePort)
port-forward:
    kubectl port-forward -n angzarr svc/angzarr-gateway 1350:1350
