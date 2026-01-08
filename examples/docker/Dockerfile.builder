# Rust builder image - compiles all binaries as static musl binaries
# Use as base for dev image or source for runtime images
#
# Build: podman build -t evented-builder:dev -f examples/docker/Dockerfile.builder .
# Use:   COPY --from=evented-builder:dev /app/target/x86_64-unknown-linux-musl/release/evented-server .

FROM rust:1.92-alpine

# Install build dependencies for static musl builds
RUN apk add --no-cache \
    musl-dev \
    protobuf-dev \
    protoc \
    openssl-dev \
    openssl-libs-static \
    pkgconfig

# Set up musl target for fully static binaries
RUN rustup target add x86_64-unknown-linux-musl
ENV RUSTFLAGS="-C target-feature=+crt-static"
ENV OPENSSL_STATIC=1
ENV OPENSSL_DIR=/usr

WORKDIR /app

# Copy manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Copy proto files and build script
COPY proto proto/
COPY build.rs ./

# Copy source code
COPY src src/

# Copy all rust examples
COPY examples/rust examples/rust/

# Create stub test files (required by Cargo.toml)
RUN mkdir -p tests && \
    echo "fn main() {}" > tests/acceptance.rs && \
    echo "fn main() {}" > tests/docker_integration.rs

# Build all release binaries with musl target (static linking)
# Note: workspace members need -p flag to specify package
RUN cargo build --release --target x86_64-unknown-linux-musl --bin evented-server && \
    cargo build --release --target x86_64-unknown-linux-musl -p customer --bin customer-server && \
    cargo build --release --target x86_64-unknown-linux-musl -p transaction --bin transaction-server && \
    cargo build --release --target x86_64-unknown-linux-musl -p saga-loyalty --bin saga-loyalty-server && \
    cargo build --release --target x86_64-unknown-linux-musl -p projector-receipt --bin projector-receipt-server && \
    cargo build --release --target x86_64-unknown-linux-musl -p projector-log-customer --bin projector-log-customer-server && \
    cargo build --release --target x86_64-unknown-linux-musl -p projector-log-transaction --bin projector-log-transaction-server

# Binaries available at /app/target/x86_64-unknown-linux-musl/release/*
