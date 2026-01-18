# syntax=docker/dockerfile:1.4
# Multi-stage build for angzarr sidecars
# Uses cargo-chef for dependency caching and musl for static binaries
# All binaries built in single stage to share compiled artifacts
#
# Build dev:  podman build --target angzarr-aggregate -t angzarr-aggregate .
# Build prod: podman build --build-arg PROFILE=release --target angzarr-aggregate -t angzarr-aggregate .

ARG PROFILE=container-dev

# =============================================================================
# Stage 1: Chef - install cargo-chef and sccache with musl toolchain
# =============================================================================
FROM docker.io/library/rust:1.92-alpine AS chef

RUN apk add --no-cache \
    musl-dev \
    protobuf-dev \
    protoc \
    openssl-dev \
    openssl-libs-static \
    pkgconfig

RUN rustup target add x86_64-unknown-linux-musl
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo install cargo-chef sccache --locked

ENV RUSTFLAGS="-C target-feature=+crt-static"
ENV OPENSSL_STATIC=1
ENV OPENSSL_DIR=/usr
ENV RUSTC_WRAPPER=sccache
ENV SCCACHE_DIR=/sccache

WORKDIR /app

# =============================================================================
# Stage 2: Planner - analyze dependencies
# =============================================================================
FROM chef AS planner

COPY Cargo.toml Cargo.lock build.rs ./
COPY proto/ ./proto/
COPY src/ ./src/

RUN mkdir -p tests && echo "fn main() {}" > tests/acceptance.rs && echo "fn main() {}" > tests/container_integration.rs && echo "fn main() {}" > tests/mongodb_debug.rs

# Create stub workspace members (examples not needed for sidecars)
RUN mkdir -p examples/rust/common/src && \
    echo -e '[package]\nname = "common"\nversion = "0.1.0"\nedition = "2021"\n[lib]\npath = "src/lib.rs"' > examples/rust/common/Cargo.toml && \
    echo "" > examples/rust/common/src/lib.rs && \
    for pkg in customer product inventory order cart fulfillment saga-loyalty-earn saga-fulfillment saga-cancellation; do \
        mkdir -p examples/rust/$pkg/src && \
        echo -e "[package]\nname = \"$pkg\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[[bin]]\nname = \"$pkg-server\"\npath = \"src/main.rs\"" > examples/rust/$pkg/Cargo.toml && \
        echo "fn main() {}" > examples/rust/$pkg/src/main.rs; \
    done && \
    for proj in accounting web; do \
        mkdir -p examples/rust/projectors/$proj/src && \
        echo -e "[package]\nname = \"projector-$proj\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[[bin]]\nname = \"projector-$proj-server\"\npath = \"src/main.rs\"" > examples/rust/projectors/$proj/Cargo.toml && \
        echo "fn main() {}" > examples/rust/projectors/$proj/src/main.rs; \
    done

RUN cargo chef prepare --recipe-path recipe.json

# =============================================================================
# Stage 3: Cacher - build dependencies only (heavily cached)
# =============================================================================
FROM chef AS cacher
ARG PROFILE

COPY --from=planner /app/recipe.json recipe.json
COPY --from=planner /app/Cargo.toml /app/Cargo.lock ./
COPY --from=planner /app/examples ./examples/
COPY proto/ ./proto/
COPY build.rs ./
RUN mkdir -p src && echo "" > src/lib.rs
RUN mkdir -p tests && echo "fn main() {}" > tests/acceptance.rs && echo "fn main() {}" > tests/container_integration.rs && echo "fn main() {}" > tests/mongodb_debug.rs

# Build dependencies with musl
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/sccache,sharing=locked \
    cargo chef cook --profile ${PROFILE} --target x86_64-unknown-linux-musl --recipe-path recipe.json

# =============================================================================
# Stage 4: Builder - compile ALL binaries in single stage (shared artifacts)
# =============================================================================
FROM chef AS builder
ARG PROFILE

COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
COPY --from=cacher /app/examples ./examples/

COPY Cargo.toml Cargo.lock build.rs ./
COPY proto/ ./proto/
COPY src/ ./src/

RUN mkdir -p tests && echo "fn main() {}" > tests/acceptance.rs && echo "fn main() {}" > tests/container_integration.rs && echo "fn main() {}" > tests/mongodb_debug.rs

# Build all binaries in single invocation - shares compilation across all targets
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/sccache,sharing=locked \
    cargo build --profile ${PROFILE} --target x86_64-unknown-linux-musl \
    --bin angzarr-aggregate \
    --bin angzarr-projector \
    --bin angzarr-saga \
    --bin angzarr-stream \
    --bin angzarr-gateway

# Create symlink for output path (release outputs to release/, others to profile name)
RUN if [ "$PROFILE" = "release" ]; then \
      ln -s /app/target/x86_64-unknown-linux-musl/release /app/target/x86_64-unknown-linux-musl/output; \
    else \
      ln -s /app/target/x86_64-unknown-linux-musl/${PROFILE} /app/target/x86_64-unknown-linux-musl/output; \
    fi

# =============================================================================
# Runtime base - distroless static image (no libc needed)
# =============================================================================
FROM gcr.io/distroless/static-debian12:nonroot AS runtime-base

WORKDIR /app
USER nonroot:nonroot

ENV ANGZARR_LOG=info

# =============================================================================
# Final images - all copy from single builder stage
# =============================================================================

# Aggregate sidecar image
FROM runtime-base AS angzarr-aggregate
COPY --from=builder /app/target/x86_64-unknown-linux-musl/output/angzarr-aggregate ./server
EXPOSE 1313 1314
ENTRYPOINT ["./server"]

# Projector sidecar image
FROM runtime-base AS angzarr-projector
COPY --from=builder /app/target/x86_64-unknown-linux-musl/output/angzarr-projector ./server
ENTRYPOINT ["./server"]

# Saga sidecar image
FROM runtime-base AS angzarr-saga
COPY --from=builder /app/target/x86_64-unknown-linux-musl/output/angzarr-saga ./server
EXPOSE 1313 1314
ENTRYPOINT ["./server"]

# Stream service image (central infrastructure)
FROM runtime-base AS angzarr-stream
COPY --from=builder /app/target/x86_64-unknown-linux-musl/output/angzarr-stream ./server
EXPOSE 1315
ENTRYPOINT ["./server"]

# Gateway service image (central infrastructure)
FROM runtime-base AS angzarr-gateway
COPY --from=builder /app/target/x86_64-unknown-linux-musl/output/angzarr-gateway ./server
EXPOSE 1316
ENTRYPOINT ["./server"]
