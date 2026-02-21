# syntax=docker/dockerfile:1.4
# Angzarr sidecar images - unified dev/release build
#
# Dev (fast, ~2min):     podman build --target angzarr-aggregate-dev -t angzarr-aggregate .
# Release (slow, ~8min): podman build --target angzarr-aggregate -t angzarr-aggregate .
#
# Dev uses native glibc target + debian runtime (no cross-compilation)
# Release uses musl static target + distroless runtime (smallest images)
#
# Multi-arch release: podman build --platform linux/amd64,linux/arm64 --target angzarr-aggregate ...
#
# Base images from ghcr.io/angzarr (see build/images/)

ARG RUST_IMAGE=ghcr.io/angzarr-io/angzarr-rust:latest

# =============================================================================
# Dev builder - native glibc (fast compilation)
# Two-stage build: deps layer (cached) + source layer (rebuilt on changes)
# =============================================================================
FROM ${RUST_IMAGE} AS builder-dev-deps

# protoc and libs already in base image

WORKDIR /app

# Copy only dependency manifests first (layer cached until Cargo.toml/Cargo.lock change)
COPY Cargo.toml Cargo.lock build.rs ./
COPY proto/ ./proto/
COPY client/rust/Cargo.toml ./client/rust/Cargo.toml

# Create minimal source stubs to satisfy cargo
RUN mkdir -p src/bin client/rust/src && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub fn stub() {}" > src/lib.rs && \
    for bin in aggregate projector saga process_manager log stream upcaster event_projector standalone; do \
      echo "fn main() {}" > src/bin/angzarr_$bin.rs; \
    done && \
    echo "pub fn stub() {}" > client/rust/src/lib.rs && \
    mkdir -p tests/integration tests/interfaces && \
    for f in acceptance container_integration mongodb_debug \
             storage_mongodb storage_redis storage_postgres storage_sqlite \
             storage_immudb storage_nats bus_nats \
             standalone_integration; do \
      echo "fn main() {}" > tests/$f.rs; \
    done && \
    echo "fn main() {}" > tests/interfaces/main.rs && \
    for f in query_test; do \
      echo "fn main() {}" > tests/integration/$f.rs; \
    done && \
    mkdir -p migrations && touch migrations/.keep

# Build dependencies only (cached until Cargo.toml/Cargo.lock change)
RUN cargo build --profile container-dev --features otel,topology,sqlite \
    --bin angzarr-aggregate \
    --bin angzarr-projector \
    --bin angzarr-saga \
    --bin angzarr-process-manager \
    --bin angzarr-log \
    --bin angzarr-stream || true

# =============================================================================
# Dev builder - source build (invalidates when src/ changes)
# =============================================================================
FROM builder-dev-deps AS builder-dev

# Copy real source (invalidates layer when source changes)
COPY src/ ./src/
COPY client/ ./client/
COPY migrations/ ./migrations/

# Rebuild with real source (deps already compiled in previous stage)
RUN cargo build --profile container-dev --features otel,topology,sqlite \
    --bin angzarr-aggregate \
    --bin angzarr-projector \
    --bin angzarr-saga \
    --bin angzarr-process-manager \
    --bin angzarr-log \
    --bin angzarr-stream && \
    cp target/container-dev/angzarr-* /tmp/

# Generate protobuf FileDescriptorSet for runtime event decoding
RUN protoc --descriptor_set_out=/tmp/descriptors.pb --include_imports \
    -I proto \
    proto/examples/inventory.proto \
    proto/examples/order.proto \
    proto/examples/fulfillment.proto \
    proto/examples/projections.proto

# =============================================================================
# Release builder - musl static, multi-arch (small images, all features)
# Two-stage build: deps layer (cached) + source layer (rebuilt on changes)
# For release, we still use Alpine for musl static linking
# =============================================================================
FROM docker.io/library/rust:1.92-alpine AS builder-release-deps

# Build argument for target architecture (set by buildx/podman)
ARG TARGETARCH

RUN apk add --no-cache \
    musl-dev \
    protobuf-dev \
    protoc \
    openssl-dev \
    openssl-libs-static \
    pkgconfig \
    cmake \
    make \
    g++ \
    perl \
    linux-headers \
    cyrus-sasl-dev

# Install targets for both architectures
RUN rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl

ENV RUSTFLAGS="-C target-feature=+crt-static"
ENV OPENSSL_STATIC=1
ENV OPENSSL_DIR=/usr

WORKDIR /app

# Copy only dependency manifests first (layer cached until Cargo.toml/Cargo.lock change)
COPY Cargo.toml Cargo.lock build.rs ./
COPY proto/ ./proto/
COPY client/rust/Cargo.toml ./client/rust/Cargo.toml

# Create minimal source stubs to satisfy cargo
RUN mkdir -p src/bin client/rust/src && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub fn stub() {}" > src/lib.rs && \
    for bin in aggregate projector saga process_manager log stream upcaster event_projector standalone; do \
      echo "fn main() {}" > src/bin/angzarr_$bin.rs; \
    done && \
    echo "pub fn stub() {}" > client/rust/src/lib.rs && \
    mkdir -p tests/integration tests/interfaces && \
    for f in acceptance container_integration mongodb_debug \
             storage_mongodb storage_redis storage_postgres storage_sqlite \
             storage_immudb storage_nats bus_nats \
             standalone_integration; do \
      echo "fn main() {}" > tests/$f.rs; \
    done && \
    echo "fn main() {}" > tests/interfaces/main.rs && \
    for f in query_test; do \
      echo "fn main() {}" > tests/integration/$f.rs; \
    done && \
    mkdir -p migrations && touch migrations/.keep

# Build dependencies only (cached until Cargo.toml/Cargo.lock change)
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        TARGET="aarch64-unknown-linux-musl"; \
    else \
        TARGET="x86_64-unknown-linux-musl"; \
    fi && \
    cargo build --profile production --target $TARGET --features full \
    --bin angzarr-aggregate \
    --bin angzarr-projector \
    --bin angzarr-saga \
    --bin angzarr-process-manager \
    --bin angzarr-log \
    --bin angzarr-stream || true

# =============================================================================
# Release builder - source build (invalidates when src/ changes)
# =============================================================================
FROM builder-release-deps AS builder-release

ARG TARGETARCH

# Copy real source (invalidates layer when source changes)
COPY src/ ./src/
COPY client/ ./client/
COPY migrations/ ./migrations/

# Rebuild with real source (deps already compiled in previous stage)
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        TARGET="aarch64-unknown-linux-musl"; \
    else \
        TARGET="x86_64-unknown-linux-musl"; \
    fi && \
    cargo build --profile production --target $TARGET --features full \
    --bin angzarr-aggregate \
    --bin angzarr-projector \
    --bin angzarr-saga \
    --bin angzarr-process-manager \
    --bin angzarr-log \
    --bin angzarr-stream && \
    cp target/$TARGET/production/angzarr-* /tmp/

# Generate protobuf FileDescriptorSet for runtime event decoding
RUN protoc --descriptor_set_out=/tmp/descriptors.pb --include_imports \
    -I proto \
    proto/examples/inventory.proto \
    proto/examples/order.proto \
    proto/examples/fulfillment.proto \
    proto/examples/projections.proto

# =============================================================================
# Runtime bases
# =============================================================================
FROM docker.io/library/debian:bookworm-slim AS runtime-dev-base
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    gdb \
    gdbserver \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
ENV ANGZARR_LOG=info

FROM gcr.io/distroless/static-debian12:nonroot AS runtime-release-base
WORKDIR /app
USER nonroot:nonroot
ENV ANGZARR_LOG=info

# =============================================================================
# Dev images (fast builds, larger runtime)
# =============================================================================
FROM runtime-dev-base AS angzarr-aggregate-dev
COPY --from=builder-dev /tmp/angzarr-aggregate ./server
EXPOSE 1313 1314
ENTRYPOINT ["./server"]

FROM runtime-dev-base AS angzarr-projector-dev
COPY --from=builder-dev /tmp/angzarr-projector ./server
ENTRYPOINT ["./server"]

FROM runtime-dev-base AS angzarr-saga-dev
COPY --from=builder-dev /tmp/angzarr-saga ./server
EXPOSE 1313 1314
ENTRYPOINT ["./server"]

FROM runtime-dev-base AS angzarr-process-manager-dev
COPY --from=builder-dev /tmp/angzarr-process-manager ./server
EXPOSE 1313 1314
ENTRYPOINT ["./server"]

FROM runtime-dev-base AS angzarr-log-dev
COPY --from=builder-dev /tmp/angzarr-log ./server
COPY --from=builder-dev /tmp/descriptors.pb ./descriptors.pb
ENV DESCRIPTOR_PATH=/app/descriptors.pb
EXPOSE 50051
ENTRYPOINT ["./server"]

FROM runtime-dev-base AS angzarr-stream-dev
COPY --from=builder-dev /tmp/angzarr-stream ./server
EXPOSE 50051
ENTRYPOINT ["./server"]

# =============================================================================
# Release images (slow builds, minimal runtime, all features)
# =============================================================================
FROM runtime-release-base AS angzarr-aggregate
COPY --from=builder-release /tmp/angzarr-aggregate ./server
EXPOSE 1313 1314
ENTRYPOINT ["./server"]

FROM runtime-release-base AS angzarr-projector
COPY --from=builder-release /tmp/angzarr-projector ./server
ENTRYPOINT ["./server"]

FROM runtime-release-base AS angzarr-saga
COPY --from=builder-release /tmp/angzarr-saga ./server
EXPOSE 1313 1314
ENTRYPOINT ["./server"]

FROM runtime-release-base AS angzarr-process-manager
COPY --from=builder-release /tmp/angzarr-process-manager ./server
EXPOSE 1313 1314
ENTRYPOINT ["./server"]

FROM runtime-release-base AS angzarr-log
COPY --from=builder-release /tmp/angzarr-log ./server
COPY --from=builder-release /tmp/descriptors.pb ./descriptors.pb
ENV DESCRIPTOR_PATH=/app/descriptors.pb
EXPOSE 50051
ENTRYPOINT ["./server"]

FROM runtime-release-base AS angzarr-stream
COPY --from=builder-release /tmp/angzarr-stream ./server
EXPOSE 50051
ENTRYPOINT ["./server"]
