# C# poker examples
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
#   - Podman mounts justfile.container as /workspace/examples/csharp/justfile
#   - `just build` on host → podman runs → `just build` in container → dotnet
#
# When running inside a devcontainer (DEVCONTAINER=true):
#   - Commands execute directly via `just <target>`
#   - No container nesting

set shell := ["bash", "-c"]

TOP := `git rev-parse --show-toplevel`
IMAGE := "angzarr-csharp-dev"

# Build the devcontainer image
[private]
_build-image:
    podman build --network=host -t {{IMAGE}} -f "{{TOP}}/examples/csharp/.devcontainer/Containerfile" "{{TOP}}/examples/csharp/.devcontainer"

# Run just target in container (or directly if already in devcontainer)
[private]
_container +ARGS: _build-image
    #!/usr/bin/env bash
    if [ "${DEVCONTAINER:-}" = "true" ]; then
        just {{ARGS}}
    else
        podman run --rm --network=host \
            -v "{{TOP}}:/workspace:Z" \
            -v "{{TOP}}/examples/csharp/justfile.container:/workspace/examples/csharp/justfile:ro" \
            -w /workspace/examples/csharp \
            {{IMAGE}} just {{ARGS}}
    fi

default:
    @just --list

restore:
    just _container restore

build:
    just _container build

build-dev:
    just _container build-dev

test-unit:
    just _container test-unit

test-acceptance:
    just _container test-acceptance

test:
    just _container test

fmt:
    just _container fmt

lint:
    just _container lint

# Run poker in standalone mode (host - needs Rust)
run: build
    mkdir -p "{{TOP}}/examples/csharp/data"
    cd "{{TOP}}" && cargo run \
        --bin angzarr-standalone \
        --features standalone,sqlite \
        -- --config examples/csharp/standalone.yaml

clean:
    just _container "dotnet clean /workspace/examples/csharp/Angzarr.Examples.sln" || true
    rm -rf "{{TOP}}/examples/csharp/data"
    find "{{TOP}}/examples/csharp" -type d \( -name 'bin' -o -name 'obj' \) -exec rm -rf {} + 2>/dev/null || true
