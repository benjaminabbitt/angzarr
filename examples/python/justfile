# Python poker examples
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
#   - Podman mounts justfile.container as /workspace/examples/python/justfile
#   - `just test` on host → podman runs → `just test` in container → pytest
#
# When running inside a devcontainer (DEVCONTAINER=true):
#   - Commands execute directly via `just <target>`
#   - No container nesting

set shell := ["bash", "-c"]

TOP := `git rev-parse --show-toplevel`
IMAGE := "angzarr-python-dev"

# Build the devcontainer image
[private]
_build-image:
    podman build --network=host -t {{IMAGE}} -f "{{TOP}}/examples/python/.devcontainer/Containerfile" "{{TOP}}/examples/python/.devcontainer"

# Run just target in container (or directly if already in devcontainer)
[private]
_container +ARGS: _build-image
    #!/usr/bin/env bash
    if [ "${DEVCONTAINER:-}" = "true" ]; then
        just {{ARGS}}
    else
        podman run --rm --network=host \
            -v "{{TOP}}:/workspace:Z" \
            -v "{{TOP}}/examples/python/justfile.container:/workspace/examples/python/justfile:ro" \
            -w /workspace/examples/python \
            {{IMAGE}} just {{ARGS}}
    fi

default:
    @just --list

install:
    just _container install

test-unit:
    just _container test-unit

test-acceptance:
    just _container test-acceptance

test: test-unit test-acceptance

fmt:
    just _container fmt

lint:
    just _container lint

typecheck:
    just _container typecheck

run-player:
    just _container run-player

run-table:
    just _container run-table

run-hand:
    just _container run-hand
