# syntax=docker/dockerfile:1.4
# Rust poker examples - optimized multi-stage build
# Build: podman build -t rust-poker-player --target agg-player -f examples/rust/Containerfile .
# Multi-arch: podman build --platform linux/amd64,linux/arm64 ...
# Context must be repo root for proto access
#
# Optimizations:
# 1. Shared deps-fetcher stage - cargo build deps runs once
# 2. Named cache IDs for cargo registry/target persistence
# 3. Dev vs release profiles (fast dev builds, minimal release images)
# 4. Distroless runtime for release - minimal attack surface
# 5. Multi-arch support (amd64 + arm64)
#
# Base images from ghcr.io/angzarr (see build/images/)

# RUST_IMAGE must be provided via build arg (no default to avoid registry lookups)
ARG RUST_IMAGE
ARG RUST_VERSION=1.92

# ============================================================================
# Dev builder - native glibc (fast compilation)
# Uses custom angzarr-rust image with protoc and mold pre-installed
# ============================================================================
FROM ${RUST_IMAGE} AS builder-dev-deps

# protoc and libs already in base image

WORKDIR /app

# Copy proto files (needed by client library build.rs)
COPY proto ./proto

# Copy client library manifests (including macros subcrate)
COPY client/rust/Cargo.toml ./client/rust/Cargo.toml
COPY client/rust/build.rs ./client/rust/build.rs
COPY client/rust/angzarr-macros/Cargo.toml ./client/rust/angzarr-macros/Cargo.toml

# Copy examples workspace manifests
COPY examples/rust/Cargo.toml examples/rust/Cargo.lock ./examples/rust/

# Aggregates
COPY examples/rust/player/agg/Cargo.toml ./examples/rust/player/agg/Cargo.toml
COPY examples/rust/player/agg-oo/Cargo.toml ./examples/rust/player/agg-oo/Cargo.toml
COPY examples/rust/player/upc/Cargo.toml ./examples/rust/player/upc/Cargo.toml
COPY examples/rust/table/agg/Cargo.toml ./examples/rust/table/agg/Cargo.toml
COPY examples/rust/table/agg-oo/Cargo.toml ./examples/rust/table/agg-oo/Cargo.toml
COPY examples/rust/hand/agg/Cargo.toml ./examples/rust/hand/agg/Cargo.toml

# Sagas
COPY examples/rust/table/saga-hand/Cargo.toml ./examples/rust/table/saga-hand/Cargo.toml
COPY examples/rust/table/saga-hand-oo/Cargo.toml ./examples/rust/table/saga-hand-oo/Cargo.toml
COPY examples/rust/table/saga-player/Cargo.toml ./examples/rust/table/saga-player/Cargo.toml
COPY examples/rust/hand/saga-table/Cargo.toml ./examples/rust/hand/saga-table/Cargo.toml
COPY examples/rust/hand/saga-player/Cargo.toml ./examples/rust/hand/saga-player/Cargo.toml

# Process Manager
COPY examples/rust/pmg-hand-flow/Cargo.toml ./examples/rust/pmg-hand-flow/Cargo.toml

# Projector
COPY examples/rust/prj-output/Cargo.toml ./examples/rust/prj-output/Cargo.toml
COPY examples/rust/prj-output-oo/Cargo.toml ./examples/rust/prj-output-oo/Cargo.toml

# Tests (needed for workspace resolution)
COPY examples/rust/tests/Cargo.toml ./examples/rust/tests/Cargo.toml

# Create minimal stubs for all workspace members
RUN mkdir -p client/rust/src client/rust/angzarr-macros/src client/rust/tests \
    examples/rust/player/agg/src \
    examples/rust/player/agg-oo/src \
    examples/rust/player/upc/src \
    examples/rust/table/agg/src \
    examples/rust/table/agg-oo/src \
    examples/rust/hand/agg/src \
    examples/rust/table/saga-hand/src \
    examples/rust/table/saga-hand-oo/src \
    examples/rust/table/saga-player/src \
    examples/rust/hand/saga-table/src \
    examples/rust/hand/saga-player/src \
    examples/rust/pmg-hand-flow/src \
    examples/rust/prj-output/src \
    examples/rust/prj-output-oo/src \
    examples/rust/tests/tests && \
    echo "pub fn stub() {}" > client/rust/src/lib.rs && \
    echo "pub fn stub() {}" > client/rust/angzarr-macros/src/lib.rs && \
    echo "fn main() {}" > client/rust/tests/features.rs && \
    echo "fn main() {}" > examples/rust/player/agg/src/main.rs && \
    echo "fn main() {}" > examples/rust/player/agg-oo/src/main.rs && \
    echo "fn main() {}" > examples/rust/player/upc/src/main.rs && \
    echo "fn main() {}" > examples/rust/table/agg/src/main.rs && \
    echo "fn main() {}" > examples/rust/table/agg-oo/src/main.rs && \
    echo "fn main() {}" > examples/rust/hand/agg/src/main.rs && \
    echo "fn main() {}" > examples/rust/table/saga-hand/src/main.rs && \
    echo "fn main() {}" > examples/rust/table/saga-hand-oo/src/main.rs && \
    echo "fn main() {}" > examples/rust/table/saga-player/src/main.rs && \
    echo "fn main() {}" > examples/rust/hand/saga-table/src/main.rs && \
    echo "fn main() {}" > examples/rust/hand/saga-player/src/main.rs && \
    echo "fn main() {}" > examples/rust/pmg-hand-flow/src/main.rs && \
    echo "fn main() {}" > examples/rust/prj-output/src/main.rs && \
    echo "fn main() {}" > examples/rust/prj-output-oo/src/main.rs && \
    echo "fn main() {}" > examples/rust/tests/tests/player.rs && \
    echo "fn main() {}" > examples/rust/tests/tests/table.rs && \
    echo "fn main() {}" > examples/rust/tests/tests/hand.rs

# Build deps only with persistent cache
WORKDIR /app/examples/rust
RUN --mount=type=cache,id=rust-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=rust-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    cargo build --profile container-dev --workspace 2>/dev/null || true

# ============================================================================
# Dev builder - source
# ============================================================================
FROM builder-dev-deps AS builder-dev

WORKDIR /app

# Copy real source - client library
COPY client/rust/src ./client/rust/src
COPY client/rust/angzarr-macros/src ./client/rust/angzarr-macros/src

# Aggregates
COPY examples/rust/player/agg/src ./examples/rust/player/agg/src
COPY examples/rust/player/agg-oo/src ./examples/rust/player/agg-oo/src
COPY examples/rust/player/upc/src ./examples/rust/player/upc/src
COPY examples/rust/table/agg/src ./examples/rust/table/agg/src
COPY examples/rust/table/agg-oo/src ./examples/rust/table/agg-oo/src
COPY examples/rust/hand/agg/src ./examples/rust/hand/agg/src

# Sagas
COPY examples/rust/table/saga-hand/src ./examples/rust/table/saga-hand/src
COPY examples/rust/table/saga-hand-oo/src ./examples/rust/table/saga-hand-oo/src
COPY examples/rust/table/saga-player/src ./examples/rust/table/saga-player/src
COPY examples/rust/hand/saga-table/src ./examples/rust/hand/saga-table/src
COPY examples/rust/hand/saga-player/src ./examples/rust/hand/saga-player/src

# Process Manager
COPY examples/rust/pmg-hand-flow/src ./examples/rust/pmg-hand-flow/src

# Projector
COPY examples/rust/prj-output/src ./examples/rust/prj-output/src
COPY examples/rust/prj-output-oo/src ./examples/rust/prj-output-oo/src

# Tests (test files only, for workspace completeness)
COPY examples/rust/tests/tests ./examples/rust/tests/tests

# Build all with persistent cache
WORKDIR /app/examples/rust
RUN --mount=type=cache,id=rust-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=rust-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    cargo build --profile container-dev --workspace && \
    cp target/container-dev/agg-player /tmp/ && \
    cp target/container-dev/upc-player /tmp/ && \
    cp target/container-dev/agg-table /tmp/ && \
    cp target/container-dev/agg-table-oo /tmp/ && \
    cp target/container-dev/agg-hand /tmp/ && \
    cp target/container-dev/saga-table-hand /tmp/ && \
    cp target/container-dev/saga-table-hand-oo /tmp/ && \
    cp target/container-dev/saga-table-player /tmp/ && \
    cp target/container-dev/saga-hand-table /tmp/ && \
    cp target/container-dev/saga-hand-player /tmp/ && \
    cp target/container-dev/pmg-hand-flow /tmp/ && \
    cp target/container-dev/prj-output /tmp/ && \
    cp target/container-dev/prj-output-oo /tmp/

# ============================================================================
# Release builder - musl static for minimal images
# ============================================================================
FROM docker.io/library/rust:${RUST_VERSION}-alpine AS builder-release-deps

ARG TARGETARCH

RUN apk add --no-cache \
    musl-dev \
    protobuf-dev \
    protoc \
    openssl-dev \
    openssl-libs-static \
    pkgconfig

RUN rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl

ENV RUSTFLAGS="-C target-feature=+crt-static"
ENV OPENSSL_STATIC=1
ENV OPENSSL_DIR=/usr

WORKDIR /app

# Copy proto files
COPY proto ./proto

# Copy client library manifests (including macros subcrate)
COPY client/rust/Cargo.toml ./client/rust/Cargo.toml
COPY client/rust/build.rs ./client/rust/build.rs
COPY client/rust/angzarr-macros/Cargo.toml ./client/rust/angzarr-macros/Cargo.toml

# Copy examples workspace manifests
COPY examples/rust/Cargo.toml examples/rust/Cargo.lock ./examples/rust/

# Aggregates
COPY examples/rust/player/agg/Cargo.toml ./examples/rust/player/agg/Cargo.toml
COPY examples/rust/player/agg-oo/Cargo.toml ./examples/rust/player/agg-oo/Cargo.toml
COPY examples/rust/player/upc/Cargo.toml ./examples/rust/player/upc/Cargo.toml
COPY examples/rust/table/agg/Cargo.toml ./examples/rust/table/agg/Cargo.toml
COPY examples/rust/table/agg-oo/Cargo.toml ./examples/rust/table/agg-oo/Cargo.toml
COPY examples/rust/hand/agg/Cargo.toml ./examples/rust/hand/agg/Cargo.toml

# Sagas
COPY examples/rust/table/saga-hand/Cargo.toml ./examples/rust/table/saga-hand/Cargo.toml
COPY examples/rust/table/saga-hand-oo/Cargo.toml ./examples/rust/table/saga-hand-oo/Cargo.toml
COPY examples/rust/table/saga-player/Cargo.toml ./examples/rust/table/saga-player/Cargo.toml
COPY examples/rust/hand/saga-table/Cargo.toml ./examples/rust/hand/saga-table/Cargo.toml
COPY examples/rust/hand/saga-player/Cargo.toml ./examples/rust/hand/saga-player/Cargo.toml

# Process Manager
COPY examples/rust/pmg-hand-flow/Cargo.toml ./examples/rust/pmg-hand-flow/Cargo.toml

# Projector
COPY examples/rust/prj-output/Cargo.toml ./examples/rust/prj-output/Cargo.toml
COPY examples/rust/prj-output-oo/Cargo.toml ./examples/rust/prj-output-oo/Cargo.toml

# Tests (needed for workspace resolution)
COPY examples/rust/tests/Cargo.toml ./examples/rust/tests/Cargo.toml

# Create minimal stubs for all workspace members
RUN mkdir -p client/rust/src client/rust/angzarr-macros/src client/rust/tests \
    examples/rust/player/agg/src \
    examples/rust/player/agg-oo/src \
    examples/rust/player/upc/src \
    examples/rust/table/agg/src \
    examples/rust/table/agg-oo/src \
    examples/rust/hand/agg/src \
    examples/rust/table/saga-hand/src \
    examples/rust/table/saga-hand-oo/src \
    examples/rust/table/saga-player/src \
    examples/rust/hand/saga-table/src \
    examples/rust/hand/saga-player/src \
    examples/rust/pmg-hand-flow/src \
    examples/rust/prj-output/src \
    examples/rust/prj-output-oo/src \
    examples/rust/tests/tests && \
    echo "pub fn stub() {}" > client/rust/src/lib.rs && \
    echo "pub fn stub() {}" > client/rust/angzarr-macros/src/lib.rs && \
    echo "fn main() {}" > client/rust/tests/features.rs && \
    echo "fn main() {}" > examples/rust/player/agg/src/main.rs && \
    echo "fn main() {}" > examples/rust/player/agg-oo/src/main.rs && \
    echo "fn main() {}" > examples/rust/player/upc/src/main.rs && \
    echo "fn main() {}" > examples/rust/table/agg/src/main.rs && \
    echo "fn main() {}" > examples/rust/table/agg-oo/src/main.rs && \
    echo "fn main() {}" > examples/rust/hand/agg/src/main.rs && \
    echo "fn main() {}" > examples/rust/table/saga-hand/src/main.rs && \
    echo "fn main() {}" > examples/rust/table/saga-hand-oo/src/main.rs && \
    echo "fn main() {}" > examples/rust/table/saga-player/src/main.rs && \
    echo "fn main() {}" > examples/rust/hand/saga-table/src/main.rs && \
    echo "fn main() {}" > examples/rust/hand/saga-player/src/main.rs && \
    echo "fn main() {}" > examples/rust/pmg-hand-flow/src/main.rs && \
    echo "fn main() {}" > examples/rust/prj-output/src/main.rs && \
    echo "fn main() {}" > examples/rust/prj-output-oo/src/main.rs && \
    echo "fn main() {}" > examples/rust/tests/tests/player.rs && \
    echo "fn main() {}" > examples/rust/tests/tests/table.rs && \
    echo "fn main() {}" > examples/rust/tests/tests/hand.rs

# Build deps with persistent cache
WORKDIR /app/examples/rust
RUN --mount=type=cache,id=rust-musl-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=rust-musl-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    if [ "$TARGETARCH" = "arm64" ]; then \
        TARGET="aarch64-unknown-linux-musl"; \
    else \
        TARGET="x86_64-unknown-linux-musl"; \
    fi && \
    cargo build --profile production --target $TARGET --workspace 2>/dev/null || true

# ============================================================================
# Release builder - source
# ============================================================================
FROM builder-release-deps AS builder-release

ARG TARGETARCH

WORKDIR /app

# Copy real source - client library
COPY client/rust/src ./client/rust/src
COPY client/rust/angzarr-macros/src ./client/rust/angzarr-macros/src

# Aggregates
COPY examples/rust/player/agg/src ./examples/rust/player/agg/src
COPY examples/rust/player/agg-oo/src ./examples/rust/player/agg-oo/src
COPY examples/rust/player/upc/src ./examples/rust/player/upc/src
COPY examples/rust/table/agg/src ./examples/rust/table/agg/src
COPY examples/rust/table/agg-oo/src ./examples/rust/table/agg-oo/src
COPY examples/rust/hand/agg/src ./examples/rust/hand/agg/src

# Sagas
COPY examples/rust/table/saga-hand/src ./examples/rust/table/saga-hand/src
COPY examples/rust/table/saga-hand-oo/src ./examples/rust/table/saga-hand-oo/src
COPY examples/rust/table/saga-player/src ./examples/rust/table/saga-player/src
COPY examples/rust/hand/saga-table/src ./examples/rust/hand/saga-table/src
COPY examples/rust/hand/saga-player/src ./examples/rust/hand/saga-player/src

# Process Manager
COPY examples/rust/pmg-hand-flow/src ./examples/rust/pmg-hand-flow/src

# Projector
COPY examples/rust/prj-output/src ./examples/rust/prj-output/src
COPY examples/rust/prj-output-oo/src ./examples/rust/prj-output-oo/src

# Tests (test files only, for workspace completeness)
COPY examples/rust/tests/tests ./examples/rust/tests/tests

WORKDIR /app/examples/rust
RUN --mount=type=cache,id=rust-musl-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=rust-musl-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    if [ "$TARGETARCH" = "arm64" ]; then \
        TARGET="aarch64-unknown-linux-musl"; \
    else \
        TARGET="x86_64-unknown-linux-musl"; \
    fi && \
    cargo build --profile production --target $TARGET --workspace && \
    cp target/$TARGET/production/agg-player /tmp/ && \
    cp target/$TARGET/production/upc-player /tmp/ && \
    cp target/$TARGET/production/agg-table /tmp/ && \
    cp target/$TARGET/production/agg-table-oo /tmp/ && \
    cp target/$TARGET/production/agg-hand /tmp/ && \
    cp target/$TARGET/production/saga-table-hand /tmp/ && \
    cp target/$TARGET/production/saga-table-hand-oo /tmp/ && \
    cp target/$TARGET/production/saga-table-player /tmp/ && \
    cp target/$TARGET/production/saga-hand-table /tmp/ && \
    cp target/$TARGET/production/saga-hand-player /tmp/ && \
    cp target/$TARGET/production/pmg-hand-flow /tmp/ && \
    cp target/$TARGET/production/prj-output /tmp/ && \
    cp target/$TARGET/production/prj-output-oo /tmp/

# ============================================================================
# Runtime bases
# ============================================================================
FROM docker.io/library/debian:bookworm-slim AS runtime-dev-base
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
ENV RUST_LOG=info

FROM gcr.io/distroless/static-debian12:nonroot AS runtime-release-base
WORKDIR /app
USER nonroot:nonroot
ENV RUST_LOG=info

# ============================================================================
# Dev images (fast builds, larger runtime with debug tools)
# ============================================================================
FROM runtime-dev-base AS agg-player-dev
COPY --from=builder-dev /tmp/agg-player ./server
ENV PORT=50001
EXPOSE 50001
ENTRYPOINT ["./server"]

FROM runtime-dev-base AS agg-table-dev
COPY --from=builder-dev /tmp/agg-table ./server
ENV PORT=50002
EXPOSE 50002
ENTRYPOINT ["./server"]

FROM runtime-dev-base AS agg-hand-dev
COPY --from=builder-dev /tmp/agg-hand ./server
ENV PORT=50003
EXPOSE 50003
ENTRYPOINT ["./server"]

# ============================================================================
# Release images (minimal, secure)
# ============================================================================
FROM runtime-release-base AS agg-player
COPY --from=builder-release --chown=nonroot:nonroot /tmp/agg-player ./server
ENV PORT=50001
EXPOSE 50001
ENTRYPOINT ["./server"]

FROM runtime-release-base AS agg-table
COPY --from=builder-release --chown=nonroot:nonroot /tmp/agg-table ./server
ENV PORT=50002
EXPOSE 50002
ENTRYPOINT ["./server"]

FROM runtime-release-base AS agg-hand
COPY --from=builder-release --chown=nonroot:nonroot /tmp/agg-hand ./server
ENV PORT=50003
EXPOSE 50003
ENTRYPOINT ["./server"]

# ============================================================================
# Dev Sagas
# ============================================================================
FROM runtime-dev-base AS saga-table-hand-dev
COPY --from=builder-dev /tmp/saga-table-hand ./server
ENV PORT=50011
EXPOSE 50011
ENTRYPOINT ["./server"]

FROM runtime-dev-base AS saga-table-player-dev
COPY --from=builder-dev /tmp/saga-table-player ./server
ENV PORT=50012
EXPOSE 50012
ENTRYPOINT ["./server"]

FROM runtime-dev-base AS saga-hand-table-dev
COPY --from=builder-dev /tmp/saga-hand-table ./server
ENV PORT=50013
EXPOSE 50013
ENTRYPOINT ["./server"]

FROM runtime-dev-base AS saga-hand-player-dev
COPY --from=builder-dev /tmp/saga-hand-player ./server
ENV PORT=50014
EXPOSE 50014
ENTRYPOINT ["./server"]

# ============================================================================
# Release Sagas
# ============================================================================
FROM runtime-release-base AS saga-table-hand
COPY --from=builder-release --chown=nonroot:nonroot /tmp/saga-table-hand ./server
ENV PORT=50011
EXPOSE 50011
ENTRYPOINT ["./server"]

FROM runtime-release-base AS saga-table-player
COPY --from=builder-release --chown=nonroot:nonroot /tmp/saga-table-player ./server
ENV PORT=50012
EXPOSE 50012
ENTRYPOINT ["./server"]

FROM runtime-release-base AS saga-hand-table
COPY --from=builder-release --chown=nonroot:nonroot /tmp/saga-hand-table ./server
ENV PORT=50013
EXPOSE 50013
ENTRYPOINT ["./server"]

FROM runtime-release-base AS saga-hand-player
COPY --from=builder-release --chown=nonroot:nonroot /tmp/saga-hand-player ./server
ENV PORT=50014
EXPOSE 50014
ENTRYPOINT ["./server"]

# ============================================================================
# Dev Process Manager
# ============================================================================
FROM runtime-dev-base AS pmg-hand-flow-dev
COPY --from=builder-dev /tmp/pmg-hand-flow ./server
ENV PORT=50391
EXPOSE 50391
ENTRYPOINT ["./server"]

# ============================================================================
# Release Process Manager
# ============================================================================
FROM runtime-release-base AS pmg-hand-flow
COPY --from=builder-release --chown=nonroot:nonroot /tmp/pmg-hand-flow ./server
ENV PORT=50391
EXPOSE 50391
ENTRYPOINT ["./server"]

# ============================================================================
# Dev Projector
# ============================================================================
FROM runtime-dev-base AS prj-output-dev
COPY --from=builder-dev /tmp/prj-output ./server
ENV PORT=50090
EXPOSE 50090
ENTRYPOINT ["./server"]

# ============================================================================
# Release Projector
# ============================================================================
FROM runtime-release-base AS prj-output
COPY --from=builder-release --chown=nonroot:nonroot /tmp/prj-output ./server
ENV PORT=50090
EXPOSE 50090
ENTRYPOINT ["./server"]

# ============================================================================
# Debug runtime base - with gdb/gdbserver
# ============================================================================
FROM docker.io/library/debian:bookworm-slim AS runtime-debug-base
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    gdb \
    gdbserver \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
ENV RUST_LOG=info

# ============================================================================
# Debug Aggregates
# ============================================================================
FROM runtime-debug-base AS agg-player-debug
COPY --from=builder-dev /tmp/agg-player ./server
ENV PORT=50001 GDB_PORT=20000
EXPOSE 50001 20000
ENTRYPOINT ["gdbserver", ":20000", "./server"]

FROM runtime-debug-base AS agg-table-debug
COPY --from=builder-dev /tmp/agg-table ./server
ENV PORT=50002 GDB_PORT=20001
EXPOSE 50002 20001
ENTRYPOINT ["gdbserver", ":20001", "./server"]

FROM runtime-debug-base AS agg-hand-debug
COPY --from=builder-dev /tmp/agg-hand ./server
ENV PORT=50003 GDB_PORT=20002
EXPOSE 50003 20002
ENTRYPOINT ["gdbserver", ":20002", "./server"]

# ============================================================================
# Debug Sagas
# ============================================================================
FROM runtime-debug-base AS saga-table-hand-debug
COPY --from=builder-dev /tmp/saga-table-hand ./server
ENV PORT=50011 GDB_PORT=20003
EXPOSE 50011 20003
ENTRYPOINT ["gdbserver", ":20003", "./server"]

FROM runtime-debug-base AS saga-table-player-debug
COPY --from=builder-dev /tmp/saga-table-player ./server
ENV PORT=50012 GDB_PORT=20004
EXPOSE 50012 20004
ENTRYPOINT ["gdbserver", ":20004", "./server"]

FROM runtime-debug-base AS saga-hand-table-debug
COPY --from=builder-dev /tmp/saga-hand-table ./server
ENV PORT=50013 GDB_PORT=20005
EXPOSE 50013 20005
ENTRYPOINT ["gdbserver", ":20005", "./server"]

FROM runtime-debug-base AS saga-hand-player-debug
COPY --from=builder-dev /tmp/saga-hand-player ./server
ENV PORT=50014 GDB_PORT=20006
EXPOSE 50014 20006
ENTRYPOINT ["gdbserver", ":20006", "./server"]

# ============================================================================
# Debug Projector
# ============================================================================
FROM runtime-debug-base AS prj-output-debug
COPY --from=builder-dev /tmp/prj-output ./server
ENV PORT=50090 GDB_PORT=20007
EXPOSE 50090 20007
ENTRYPOINT ["gdbserver", ":20007", "./server"]
