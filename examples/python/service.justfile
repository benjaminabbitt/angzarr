# Python service template - define variables before importing:
#   PROTO_DIR   - proto output directory (e.g., "angzarr" or "angzarr/proto")
#   UNIT_TARGET - pytest unit test target (e.g., "--ignore=features/ --ignore=acceptance/")

set shell := ["bash", "-c"]

TOP := `git rev-parse --show-toplevel`

default:
    @just --list

install:
    uv sync --dev

proto:
    mkdir -p {{PROTO_DIR}} proto
    cp -r "{{TOP}}/generated/python/angzarr/"* {{PROTO_DIR}}/ 2>/dev/null || true
    cp -r "{{TOP}}/generated/python/examples/"* proto/ 2>/dev/null || true
    touch angzarr/__init__.py {{PROTO_DIR}}/__init__.py proto/__init__.py

setup: install proto

run: setup
    uv run python server.py

debug: setup
    uv run python -m debugpy --listen 0.0.0.0:5678 --wait-for-client server.py

run-port port:
    PORT={{port}} uv run python server.py

copy-features:
    rm -rf features
    cp -r {{TOP}}/examples/features .

unit: setup
    uv run pytest -v {{UNIT_TARGET}}

acceptance: setup copy-features
    ANGZARR_TEST_MODE=container uv run pytest -v features/

test: unit acceptance

test-unit: unit

clean:
    rm -rf __pycache__ .pytest_cache {{PROTO_DIR}}/*.py proto/*.py
    find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
