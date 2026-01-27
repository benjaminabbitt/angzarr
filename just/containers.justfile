# Container image build commands
# Multi-stage build - each target builds required stages and uses layer caching

REGISTRY_PORT := "5001"

# Build aggregate sidecar container image
aggregate:
    podman build --target angzarr-aggregate -t localhost:{{REGISTRY_PORT}}/angzarr-aggregate:latest .

# Build projector sidecar container image
projector:
    podman build --target angzarr-projector -t localhost:{{REGISTRY_PORT}}/angzarr-projector:latest .

# Build saga sidecar container image
saga:
    podman build --target angzarr-saga -t localhost:{{REGISTRY_PORT}}/angzarr-saga:latest .

# Build stream service container image
stream:
    podman build --target angzarr-stream -t localhost:{{REGISTRY_PORT}}/angzarr-stream:latest .

# Build gateway service container image
gateway:
    podman build --target angzarr-gateway -t localhost:{{REGISTRY_PORT}}/angzarr-gateway:latest .

# Build all sidecar container images
sidecars: aggregate projector saga

# Build all infrastructure container images
infrastructure: stream gateway

# Build all container images
all: sidecars infrastructure
