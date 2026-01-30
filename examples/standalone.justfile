# Standalone mode operations for angzarr examples
# Runs all services locally via angzarr-standalone orchestrator with UDS transport
#
# Usage:
#   just examples standalone run rust      # Run Rust examples in standalone mode
#   just examples standalone test rust     # Run acceptance tests against Rust standalone
#   just examples standalone run python    # Run Python examples in standalone mode
#   just examples standalone test python   # Run acceptance tests against Python standalone

set shell := ["bash", "-c"]

TOP := `git rev-parse --show-toplevel`

# Show available commands
default:
    @just --list

# ============================================================================
# Common Infrastructure
# ============================================================================

# Build angzarr binaries for standalone mode
build-angzarr:
    cd {{TOP}} && cargo build --features sqlite --bin angzarr-aggregate --bin angzarr-saga --bin angzarr-projector --bin angzarr-gateway --bin angzarr-standalone

# Clean up stale UDS sockets
clean-sockets:
    mkdir -p /tmp/angzarr
    rm -f /tmp/angzarr/*.sock 2>/dev/null || true

# Kill any stale standalone processes
kill:
    #!/usr/bin/env bash
    for proc in angzarr-standalone angzarr-aggregate angzarr-saga angzarr-projector angzarr-gateway; do
        pids=$(pgrep -x "$proc" 2>/dev/null || true)
        if [ -n "$pids" ]; then
            echo "$pids" | xargs kill -9 2>/dev/null || true
        fi
    done
    # Kill business logic servers (Rust)
    pkill -f "customer-server|product-server|inventory-server|order-server|cart-server|fulfillment-server|saga-.*-server|projector-.*-server" 2>/dev/null || true
    # Kill Python servers
    pkill -f "python server.py" 2>/dev/null || true
    # Kill Go servers
    pkill -f "go run.*examples/go" 2>/dev/null || true
    sleep 1

# ============================================================================
# Language-Specific Run Targets
# ============================================================================

# Run standalone mode for specified language
run LANG: build-angzarr kill clean-sockets (build-lang LANG)
    cd {{TOP}} && ANGZARR_CONFIG=examples/{{LANG}}/standalone.yaml {{TOP}}/target/debug/angzarr-standalone

# Build language-specific examples
[private]
build-lang LANG:
    #!/usr/bin/env bash
    case "{{LANG}}" in
        rust)
            cd "{{TOP}}" && cargo build --workspace --features sqlite
            ;;
        python)
            echo "Generating and distributing Python protos..."
            cd "{{TOP}}" && just proto python
            cd "{{TOP}}" && just examples proto-dist-python
            echo "Python examples use uv run (no pre-build needed)"
            ;;
        go)
            echo "Generating and distributing Go protos..."
            cd "{{TOP}}" && just proto go
            cd "{{TOP}}" && just examples proto-dist-go
            echo "Go examples use go run (no pre-build needed)"
            ;;
        *)
            echo "Unknown language: {{LANG}}"
            exit 1
            ;;
    esac

# ============================================================================
# Language-Specific Test Targets
# ============================================================================

# Run acceptance tests against standalone mode
test LANG: build-angzarr kill clean-sockets (build-lang LANG)
    #!/usr/bin/env bash
    set -e
    cd "{{TOP}}"

    # Cleanup function
    cleanup() {
        echo "Cleaning up..."
        for proc in angzarr-standalone angzarr-aggregate angzarr-saga angzarr-projector angzarr-gateway; do
            pids=$(pgrep -x "$proc" 2>/dev/null || true)
            if [ -n "$pids" ]; then
                echo "$pids" | xargs kill -9 2>/dev/null || true
            fi
        done
        pkill -f "customer-server|product-server|inventory-server|order-server|cart-server|fulfillment-server|saga-.*-server|projector-.*-server" 2>/dev/null || true
        pkill -f "python server.py" 2>/dev/null || true
        pkill -f "go run.*examples/go" 2>/dev/null || true
        rm -f /tmp/angzarr/*.sock 2>/dev/null || true
    }
    trap cleanup EXIT

    # Start standalone runtime in background
    echo "Starting standalone runtime for {{LANG}}..."
    ANGZARR_CONFIG=examples/{{LANG}}/standalone.yaml "{{TOP}}/target/debug/angzarr-standalone" &
    STANDALONE_PID=$!

    # Wait for services to be ready (gateway on port 9084)
    echo "Waiting for gateway to be ready..."
    for i in {1..90}; do
        if nc -z localhost 9084 2>/dev/null; then
            echo "Gateway ready!"
            break
        fi
        if [ $i -eq 90 ]; then
            echo "Timeout waiting for gateway"
            exit 1
        fi
        sleep 1
    done

    # Additional wait for business logic services
    sleep 3

    # Run acceptance tests based on language
    echo "Running acceptance tests for {{LANG}}..."
    case "{{LANG}}" in
        rust)
            cd "{{TOP}}/examples/rust"
            ANGZARR_PORT=9084 cargo test --workspace --test acceptance || TEST_RESULT=$?
            ;;
        python)
            cd "{{TOP}}/examples/python/tests"
            ANGZARR_PORT=9084 ANGZARR_TEST_MODE=standalone uv run pytest -v || TEST_RESULT=$?
            ;;
        go)
            cd "{{TOP}}/examples/go"
            ANGZARR_PORT=9084 go test -v ./... || TEST_RESULT=$?
            ;;
    esac

    exit ${TEST_RESULT:-0}

# ============================================================================
# Multi-Language Operations
# ============================================================================

# Run all languages sequentially in standalone mode (for CI)
test-all: (test "rust") (test "python") (test "go")
    @echo "All standalone tests complete!"
