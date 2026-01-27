# Kind/Podman development commands

TOP := `git rev-parse --show-toplevel`

# Infrastructure images
INFRA_IMAGES := "docker.io/library/mongo:7 docker.io/library/rabbitmq:3.13-management-alpine docker.io/library/redis:7-alpine"

# Ingress controller images
INGRESS_IMAGES := "registry.k8s.io/ingress-nginx/controller:v1.12.0 registry.k8s.io/ingress-nginx/kube-webhook-certgen:v1.4.4"

# Create Kind cluster for local development (idempotent) - uses tar loading
create:
    @KIND_EXPERIMENTAL_PROVIDER=podman kind get clusters 2>/dev/null | grep -q '^angzarr$' || \
        KIND_EXPERIMENTAL_PROVIDER=podman kind create cluster --config kind-config.yaml --name angzarr
    @kubectl config use-context kind-angzarr 2>/dev/null || true

# Create Kind cluster with local registry (faster image loading)
create-registry:
    uv run "{{TOP}}/scripts/kind-with-registry.py"

# Show Kind cluster and registry status
status:
    uv run "{{TOP}}/scripts/kind-with-registry.py" status

# Delete Kind cluster (keeps registry for reuse)
delete:
    uv run "{{TOP}}/scripts/kind-with-registry.py" delete

# Delete Kind cluster and registry
delete-all:
    uv run "{{TOP}}/scripts/kind-with-registry.py" delete-all

# === Infrastructure Images ===

# Pull infrastructure images
images-pull-infra:
    @for img in {{INFRA_IMAGES}}; do podman pull "$img" || true; done

# Load infrastructure images into kind cluster
images-load-infra: images-pull-infra
    @for img in {{INFRA_IMAGES}}; do \
        "{{TOP}}/scripts/kind-load-images.sh" angzarr "$img"; \
    done

# Pull ingress controller images
images-pull-ingress:
    @for img in {{INGRESS_IMAGES}}; do podman pull "$img" || true; done

# Load ingress images into kind cluster
images-load-ingress: images-pull-ingress
    @for img in {{INGRESS_IMAGES}}; do \
        "{{TOP}}/scripts/kind-load-images.sh" angzarr "$img"; \
    done
