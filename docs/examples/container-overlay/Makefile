# Makefile - runs on host, delegates to container
# Demonstrates the container overlay pattern

.PHONY: build test info image demo clean

IMAGE := overlay-demo:latest

# Container invocation with overlay mount
# Key: Makefile.container is mounted OVER Makefile inside container
DOCKER_RUN := podman run --rm \
	-v ./:/workspace \
	-v ./Makefile.container:/workspace/Makefile:ro \
	-w /workspace \
	$(IMAGE)

# Build the demo image (alpine + make, ~8MB)
image:
	podman build -t $(IMAGE) -f Containerfile .

build: image
	$(DOCKER_RUN) make build

test: image
	$(DOCKER_RUN) make test

info: image
	$(DOCKER_RUN) make info

# Demonstrate the pattern
demo: image
	@echo "=== Host Environment ==="
	@echo "Hostname: $$(hostname)"
	@echo "This Makefile: Makefile (host version)"
	@echo ""
	@echo "=== Running 'make info' (delegates to container) ==="
	@echo ""
	@$(DOCKER_RUN) make info

clean:
	podman rmi $(IMAGE) 2>/dev/null || true
