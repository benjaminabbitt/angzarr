# Framework build commands

# Build the project (debug - fast compile)
debug:
    cargo build

# Build with fast-dev profile (fastest compile, no debug info)
fast:
    cargo build --profile fast-dev

# Build release (all binaries - slow compile, fast runtime)
release: sidecars infrastructure

# === Sidecar Binaries ===

# Build command sidecar binary
command:
    cargo build --release --bin angzarr-entity --features "mode-entity,mongodb"

# Build projector sidecar binary
projector:
    cargo build --release --bin angzarr-projector --features "mode-projector,mongodb"

# Build saga sidecar binary
saga:
    cargo build --release --bin angzarr-saga --features "mode-saga,mongodb"

# Build stream service binary (infrastructure projector)
stream:
    cargo build --release --bin angzarr-stream

# Build gateway service binary (infrastructure)
gateway:
    cargo build --release --bin angzarr-gateway

# Build all sidecar binaries
sidecars: command projector saga

# Build all infrastructure service binaries
infrastructure: stream gateway
