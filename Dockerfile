# Multi-stage build for evented server
FROM rust:1.92-slim AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    libprotobuf-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy all source files
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY proto/ ./proto/
COPY src/ ./src/

# Create stub test file (required by Cargo.toml)
RUN mkdir -p tests && echo "fn main() {}" > tests/acceptance.rs

# Build the actual binary
RUN cargo build --release --bin evented-server

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/evented-server /usr/local/bin/

# Create data directory and non-root user
RUN mkdir -p /app/data && \
    useradd -r -s /bin/false evented && \
    chown -R evented:evented /app

USER evented

# Environment variables
ENV RUST_LOG=info
ENV EVENTED_DB_PATH=/app/data/events.db

# Expose gRPC ports
EXPOSE 50051

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD test -x /usr/local/bin/evented-server || exit 1

CMD ["evented-server"]
