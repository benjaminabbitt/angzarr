# Multi-stage build for angzarr sidecars
# Uses cargo-chef for dependency caching: only rebuilds app code on source changes

# =============================================================================
# Stage 1: Chef - install cargo-chef
# =============================================================================
FROM rust:1.92-slim AS chef

RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    libprotobuf-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef --locked

WORKDIR /app

# =============================================================================
# Stage 2: Planner - analyze dependencies
# =============================================================================
FROM chef AS planner

COPY Cargo.toml Cargo.lock build.rs ./
COPY proto/ ./proto/
COPY src/ ./src/

RUN mkdir -p tests && echo "fn main() {}" > tests/acceptance.rs

# Create stub workspace members (examples not needed for sidecars)
RUN mkdir -p examples/rust/common/src && \
    echo '[package]\nname = "common"\nversion = "0.1.0"\nedition = "2021"\n[lib]\npath = "src/lib.rs"' > examples/rust/common/Cargo.toml && \
    echo "" > examples/rust/common/src/lib.rs && \
    for pkg in customer transaction saga-loyalty projector-receipt projector-log-customer projector-log-transaction; do \
        mkdir -p examples/rust/$pkg/src && \
        echo "[package]\nname = \"$pkg\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[[bin]]\nname = \"$pkg-server\"\npath = \"src/main.rs\"" > examples/rust/$pkg/Cargo.toml && \
        echo "fn main() {}" > examples/rust/$pkg/src/main.rs; \
    done

RUN cargo chef prepare --recipe-path recipe.json

# =============================================================================
# Stage 3: Cacher - build dependencies only (heavily cached)
# =============================================================================
FROM chef AS cacher

COPY --from=planner /app/recipe.json recipe.json
COPY --from=planner /app/Cargo.toml /app/Cargo.lock ./
COPY --from=planner /app/examples ./examples/
COPY proto/ ./proto/
COPY build.rs ./
RUN mkdir -p src && echo "" > src/lib.rs
RUN mkdir -p tests && echo "fn main() {}" > tests/acceptance.rs

# Build dependencies for all feature combinations
RUN cargo chef cook --release --recipe-path recipe.json

# =============================================================================
# Stage 4: Builder base - compile application code
# =============================================================================
FROM chef AS builder-base

COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
COPY --from=cacher /app/examples ./examples/

COPY Cargo.toml Cargo.lock build.rs ./
COPY proto/ ./proto/
COPY src/ ./src/

RUN mkdir -p tests && echo "fn main() {}" > tests/acceptance.rs

# Command sidecar
FROM builder-base AS builder-command
RUN cargo build --release --bin angzarr-command --features "mode-command,mongodb"

# Projector sidecar
FROM builder-base AS builder-projector
RUN cargo build --release --bin angzarr-projector --features "mode-projector,mongodb"

# Saga sidecar
FROM builder-base AS builder-saga
RUN cargo build --release --bin angzarr-saga --features "mode-saga,mongodb"

# Stream service (central infrastructure)
FROM builder-base AS builder-stream
RUN cargo build --release --bin angzarr-stream --features "mode-stream"

# Gateway service (central infrastructure)
FROM builder-base AS builder-gateway
RUN cargo build --release --bin angzarr-gateway --features "mode-gateway"

# =============================================================================
# Runtime base
# =============================================================================
FROM debian:bookworm-slim AS runtime-base

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -r -s /bin/false angzarr

USER angzarr

ENV ANGZARR_LOG=info

# =============================================================================
# Final images
# =============================================================================

# Command sidecar image
FROM runtime-base AS angzarr-command
COPY --from=builder-command /app/target/release/angzarr-command /usr/local/bin/
EXPOSE 1313 1314
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD test -x /usr/local/bin/angzarr-command || exit 1
CMD ["angzarr-command"]

# Projector sidecar image
FROM runtime-base AS angzarr-projector
COPY --from=builder-projector /app/target/release/angzarr-projector /usr/local/bin/
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD test -x /usr/local/bin/angzarr-projector || exit 1
CMD ["angzarr-projector"]

# Saga sidecar image
FROM runtime-base AS angzarr-saga
COPY --from=builder-saga /app/target/release/angzarr-saga /usr/local/bin/
EXPOSE 1313 1314
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD test -x /usr/local/bin/angzarr-saga || exit 1
CMD ["angzarr-saga"]

# Stream service image (central infrastructure)
FROM runtime-base AS angzarr-stream
COPY --from=builder-stream /app/target/release/angzarr-stream /usr/local/bin/
EXPOSE 1315
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD test -x /usr/local/bin/angzarr-stream || exit 1
CMD ["angzarr-stream"]

# Gateway service image (central infrastructure)
FROM runtime-base AS angzarr-gateway
COPY --from=builder-gateway /app/target/release/angzarr-gateway /usr/local/bin/
EXPOSE 1316
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD test -x /usr/local/bin/angzarr-gateway || exit 1
CMD ["angzarr-gateway"]
