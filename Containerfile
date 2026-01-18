# syntax=docker/dockerfile:1.4
# Angzarr sidecar images - unified dev/release build
#
# Dev (fast, ~2min):     podman build --target angzarr-aggregate-dev -t angzarr-aggregate .
# Release (slow, ~8min): podman build --target angzarr-aggregate -t angzarr-aggregate .
#
# Dev uses native glibc target + debian runtime (no cross-compilation)
# Release uses musl static target + distroless runtime (smallest images)

# =============================================================================
# Dev builder - native glibc (fast compilation)
# =============================================================================
FROM docker.io/library/rust:1.92-bookworm AS builder-dev

RUN apt-get update && apt-get install -y --no-install-recommends \
    protobuf-compiler \
    libprotobuf-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Cargo.toml Cargo.lock build.rs ./
COPY proto/ ./proto/
COPY src/ ./src/
COPY examples/rust examples/rust/

RUN mkdir -p tests && echo "fn main() {}" > tests/acceptance.rs && \
    echo "fn main() {}" > tests/container_integration.rs && \
    echo "fn main() {}" > tests/mongodb_debug.rs

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --profile container-dev \
    --bin angzarr-aggregate \
    --bin angzarr-projector \
    --bin angzarr-saga \
    --bin angzarr-stream \
    --bin angzarr-gateway && \
    cp target/container-dev/angzarr-* /tmp/

# =============================================================================
# Release builder - musl static (small images)
# =============================================================================
FROM docker.io/library/rust:1.92-alpine AS builder-release

RUN apk add --no-cache \
    musl-dev \
    protobuf-dev \
    protoc \
    openssl-dev \
    openssl-libs-static \
    pkgconfig

RUN rustup target add x86_64-unknown-linux-musl

ENV RUSTFLAGS="-C target-feature=+crt-static"
ENV OPENSSL_STATIC=1
ENV OPENSSL_DIR=/usr

WORKDIR /app

COPY Cargo.toml Cargo.lock build.rs ./
COPY proto/ ./proto/
COPY src/ ./src/
COPY examples/rust examples/rust/

RUN mkdir -p tests && echo "fn main() {}" > tests/acceptance.rs && \
    echo "fn main() {}" > tests/container_integration.rs && \
    echo "fn main() {}" > tests/mongodb_debug.rs

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release --target x86_64-unknown-linux-musl \
    --bin angzarr-aggregate \
    --bin angzarr-projector \
    --bin angzarr-saga \
    --bin angzarr-stream \
    --bin angzarr-gateway && \
    cp target/x86_64-unknown-linux-musl/release/angzarr-* /tmp/

# =============================================================================
# Runtime bases
# =============================================================================
FROM docker.io/library/debian:bookworm-slim AS runtime-dev-base
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
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

FROM runtime-dev-base AS angzarr-stream-dev
COPY --from=builder-dev /tmp/angzarr-stream ./server
EXPOSE 1315
ENTRYPOINT ["./server"]

FROM runtime-dev-base AS angzarr-gateway-dev
COPY --from=builder-dev /tmp/angzarr-gateway ./server
EXPOSE 1316
ENTRYPOINT ["./server"]

# =============================================================================
# Release images (slow builds, minimal runtime)
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

FROM runtime-release-base AS angzarr-stream
COPY --from=builder-release /tmp/angzarr-stream ./server
EXPOSE 1315
ENTRYPOINT ["./server"]

FROM runtime-release-base AS angzarr-gateway
COPY --from=builder-release /tmp/angzarr-gateway ./server
EXPOSE 1316
ENTRYPOINT ["./server"]
