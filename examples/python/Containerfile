# syntax=docker/dockerfile:1.4
# Python poker examples - multi-stage build
# Build: podman build -t poker-python-player --target agg-player -f examples/python/Containerfile .
# Context must be repo root for client library access
#
# Dev images use debian:slim with full Python runtime
# All targets use uv for fast dependency management

ARG PYTHON_VERSION=3.11
ARG UV_VERSION=0.10.3

# ============================================================================
# Base builder - Python with uv
# ============================================================================
FROM docker.io/library/python:${PYTHON_VERSION}-slim AS base

ARG UV_VERSION

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install uv
RUN curl -LsSf https://astral.sh/uv/${UV_VERSION}/install.sh | sh
ENV PATH=/root/.local/bin:$PATH

WORKDIR /app

# ============================================================================
# Dependencies - install all Python deps once
# ============================================================================
FROM base AS deps

# Install buf for proto generation
RUN ARCH=$(dpkg --print-architecture) && \
    case "$ARCH" in \
        amd64) BUF_ARCH="x86_64" ;; \
        arm64) BUF_ARCH="aarch64" ;; \
        *) echo "Unsupported architecture: $ARCH" && exit 1 ;; \
    esac && \
    curl -fLo /usr/local/bin/buf \
        "https://github.com/bufbuild/buf/releases/download/v1.47.2/buf-Linux-${BUF_ARCH}" && \
    chmod +x /usr/local/bin/buf

# Copy proto definitions and generate Python code
COPY proto /app/proto
RUN cd /app/proto && buf generate --template buf.gen.python.yaml

# Copy client library (proto files now generated)
COPY client/python /app/client/python

# Fix Python proto imports to use angzarr_client package prefix
RUN find /app/client/python/angzarr_client/proto -name "*.py" -exec sed -i 's/from angzarr import/from angzarr_client.proto.angzarr import/g' {} \; && \
    find /app/client/python/angzarr_client/proto/examples -name "*.py" -exec sed -i 's/from examples import/from angzarr_client.proto.examples import/g' {} \;

# Create a container-specific pyproject.toml that points to the right path
COPY examples/python/pyproject.toml ./pyproject.toml.orig

# Fix the client path reference for container build
RUN sed 's|path = "../../client/python"|path = "/app/client/python"|g' pyproject.toml.orig > pyproject.toml

# Install dependencies with uv (regenerate lock for container paths)
RUN --mount=type=cache,id=uv-cache,target=/root/.cache/uv \
    uv sync --no-dev

# ============================================================================
# Source - copy all Python source
# ============================================================================
FROM deps AS source

# Copy all example source
COPY examples/python/player ./player
COPY examples/python/table ./table
COPY examples/python/hand ./hand
COPY examples/python/hand-flow ./hand-flow
COPY examples/python/prj-output ./prj-output

# ============================================================================
# Runtime base - slim Python image
# ============================================================================
FROM docker.io/library/python:${PYTHON_VERSION}-slim AS runtime-base

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 1000 angzarr

WORKDIR /app
USER angzarr

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PYTHONPATH=/app

# ============================================================================
# Aggregates
# ============================================================================
FROM runtime-base AS agg-player
COPY --from=deps --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/player /app/player
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50301 \
    PYTHONPATH=/app:/app/client/python
EXPOSE 50301
CMD ["python", "player/agg/main.py"]

FROM runtime-base AS agg-table
COPY --from=deps --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/table /app/table
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50302 \
    PYTHONPATH=/app:/app/client/python
EXPOSE 50302
CMD ["python", "table/agg/main.py"]

FROM runtime-base AS agg-hand
COPY --from=deps --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/hand /app/hand
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50303 \
    PYTHONPATH=/app:/app/client/python
EXPOSE 50303
CMD ["python", "hand/agg/main.py"]

# ============================================================================
# Sagas
# ============================================================================
FROM runtime-base AS saga-table-hand
COPY --from=deps --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/table /app/table
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50311 \
    PYTHONPATH=/app:/app/client/python
EXPOSE 50311
CMD ["python", "table/saga-hand/main.py"]

FROM runtime-base AS saga-hand-table
COPY --from=deps --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/hand /app/hand
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50312 \
    PYTHONPATH=/app:/app/client/python
EXPOSE 50312
CMD ["python", "hand/saga-table/main.py"]

FROM runtime-base AS saga-table-player
COPY --from=deps --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/table /app/table
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50313 \
    PYTHONPATH=/app:/app/client/python
EXPOSE 50313
CMD ["python", "table/saga-player/main.py"]

FROM runtime-base AS saga-hand-player
COPY --from=deps --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/hand /app/hand
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50314 \
    PYTHONPATH=/app:/app/client/python
EXPOSE 50314
CMD ["python", "hand/saga-player/main.py"]

# ============================================================================
# Process Manager
# ============================================================================
FROM runtime-base AS pmg-hand-flow
COPY --from=deps --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/hand-flow /app/hand-flow
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50391 \
    PYTHONPATH=/app:/app/client/python
EXPOSE 50391
CMD ["python", "hand-flow/main.py"]

# ============================================================================
# Projector
# ============================================================================
FROM runtime-base AS prj-output
COPY --from=deps --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/prj-output /app/prj-output
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50390 \
    PYTHONPATH=/app:/app/client/python
EXPOSE 50390
CMD ["python", "prj-output/main.py"]

# ============================================================================
# Debug dependencies - install debugpy
# ============================================================================
FROM deps AS deps-debug

RUN --mount=type=cache,id=uv-cache,target=/root/.cache/uv \
    uv pip install debugpy>=1.8.0

# ============================================================================
# Debug runtime base - includes debug tools
# ============================================================================
FROM docker.io/library/python:${PYTHON_VERSION}-slim AS runtime-debug-base

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 1000 angzarr

WORKDIR /app
USER angzarr

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PYTHONPATH=/app

# ============================================================================
# Debug Aggregates
# ============================================================================
FROM runtime-debug-base AS agg-player-debug
COPY --from=deps-debug --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps-debug --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/player /app/player
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50301 \
    PYTHONPATH=/app:/app/client/python \
    DEBUGPY_PORT=56780
EXPOSE 50301 56780
CMD ["python", "-m", "debugpy", "--listen", "0.0.0.0:56780", "--wait-for-client", "player/agg/main.py"]

FROM runtime-debug-base AS agg-table-debug
COPY --from=deps-debug --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps-debug --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/table /app/table
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50302 \
    PYTHONPATH=/app:/app/client/python \
    DEBUGPY_PORT=56781
EXPOSE 50302 56781
CMD ["python", "-m", "debugpy", "--listen", "0.0.0.0:56781", "--wait-for-client", "table/agg/main.py"]

FROM runtime-debug-base AS agg-hand-debug
COPY --from=deps-debug --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps-debug --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/hand /app/hand
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50303 \
    PYTHONPATH=/app:/app/client/python \
    DEBUGPY_PORT=56782
EXPOSE 50303 56782
CMD ["python", "-m", "debugpy", "--listen", "0.0.0.0:56782", "--wait-for-client", "hand/agg/main.py"]

# ============================================================================
# Debug Sagas
# ============================================================================
FROM runtime-debug-base AS saga-table-hand-debug
COPY --from=deps-debug --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps-debug --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/table /app/table
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50311 \
    PYTHONPATH=/app:/app/client/python \
    DEBUGPY_PORT=56783
EXPOSE 50311 56783
CMD ["python", "-m", "debugpy", "--listen", "0.0.0.0:56783", "--wait-for-client", "table/saga-hand/main.py"]

FROM runtime-debug-base AS saga-hand-table-debug
COPY --from=deps-debug --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps-debug --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/hand /app/hand
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50312 \
    PYTHONPATH=/app:/app/client/python \
    DEBUGPY_PORT=56784
EXPOSE 50312 56784
CMD ["python", "-m", "debugpy", "--listen", "0.0.0.0:56784", "--wait-for-client", "hand/saga-table/main.py"]

FROM runtime-debug-base AS saga-table-player-debug
COPY --from=deps-debug --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps-debug --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/table /app/table
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50313 \
    PYTHONPATH=/app:/app/client/python \
    DEBUGPY_PORT=56785
EXPOSE 50313 56785
CMD ["python", "-m", "debugpy", "--listen", "0.0.0.0:56785", "--wait-for-client", "table/saga-player/main.py"]

FROM runtime-debug-base AS saga-hand-player-debug
COPY --from=deps-debug --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps-debug --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/hand /app/hand
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50314 \
    PYTHONPATH=/app:/app/client/python \
    DEBUGPY_PORT=56786
EXPOSE 50314 56786
CMD ["python", "-m", "debugpy", "--listen", "0.0.0.0:56786", "--wait-for-client", "hand/saga-player/main.py"]

# ============================================================================
# Debug Process Manager
# ============================================================================
FROM runtime-debug-base AS pmg-hand-flow-debug
COPY --from=deps-debug --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps-debug --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/hand-flow /app/hand-flow
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50391 \
    PYTHONPATH=/app:/app/client/python \
    DEBUGPY_PORT=56787
EXPOSE 50391 56787
CMD ["python", "-m", "debugpy", "--listen", "0.0.0.0:56787", "--wait-for-client", "hand-flow/main.py"]

# ============================================================================
# Debug Projector
# ============================================================================
FROM runtime-debug-base AS prj-output-debug
COPY --from=deps-debug --chown=angzarr:angzarr /app/.venv /app/.venv
COPY --from=deps-debug --chown=angzarr:angzarr /app/client/python /app/client/python
COPY --from=source --chown=angzarr:angzarr /app/prj-output /app/prj-output
ENV PATH=/app/.venv/bin:$PATH \
    PORT=50390 \
    PYTHONPATH=/app:/app/client/python \
    DEBUGPY_PORT=56788
EXPOSE 50390 56788
CMD ["python", "-m", "debugpy", "--listen", "0.0.0.0:56788", "--wait-for-client", "prj-output/main.py"]
