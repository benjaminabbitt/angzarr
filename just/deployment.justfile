# Deploy operations commands

TOP := `git rev-parse --show-toplevel`
REGISTRY_PORT := "5001"

# Image tag for local development
IMAGE_TAG := "dev"

# Run the standalone server (local development)
run-standalone:
    cargo run --bin angzarr-standalone --features "mode-standalone,amqp,mongodb"

# Delete deployment
undeploy:
    cd "{{TOP}}/examples/rust" && skaffold delete || true
    kubectl delete namespace angzarr --ignore-not-found=true

# Rebuild and redeploy via skaffold (handles incremental builds)
redeploy:
    cd "{{TOP}}/examples/rust" && skaffold run
    @kubectl get pods -n angzarr

# === Reliable Deployment (Cache-Busting) ===
# Use these targets when skaffold's incremental builds fail to pick up changes.

# Force rebuild core angzarr images (no layer cache)
rebuild-core:
    @echo "Building angzarr core images (no cache)..."
    rm -f ~/.skaffold/cache
    BUILDAH_LAYERS=false skaffold build --cache-artifacts=false

# Force rebuild all images including examples (no layer cache)
rebuild-all:
    @echo "Building all images (no cache)..."
    rm -f ~/.skaffold/cache
    cd "{{TOP}}/examples/rust" && BUILDAH_LAYERS=false skaffold build --cache-artifacts=false

# Force pods to restart and pull fresh images
reload-pods:
    @echo "Restarting angzarr deployments..."
    kubectl rollout restart deployment -n angzarr -l app.kubernetes.io/component=aggregate 2>/dev/null || true
    kubectl rollout restart deployment -n angzarr -l app.kubernetes.io/component=saga 2>/dev/null || true
    kubectl rollout restart deployment -n angzarr angzarr-gateway 2>/dev/null || true
    kubectl rollout restart deployment -n angzarr angzarr-stream 2>/dev/null || true
    @echo "Waiting for rollouts..."
    kubectl rollout status deployment -n angzarr -l app.kubernetes.io/component=aggregate --timeout=120s 2>/dev/null || true
    kubectl rollout status deployment -n angzarr angzarr-gateway --timeout=60s 2>/dev/null || true

# Quick redeploy: rebuild with cache, force helm upgrade
quick:
    cd "{{TOP}}/examples/rust" && skaffold run --force
    @kubectl get pods -n angzarr

# Redeploy a single Rust service (e.g., just deploy service customer)
service SERVICE:
    @echo "Building {{SERVICE}}..."
    podman build --target {{SERVICE}} -t docker.io/library/rs-{{SERVICE}}:{{IMAGE_TAG}} \
        -f examples/rust/Containerfile "{{TOP}}/examples/rust"
    podman tag docker.io/library/rs-{{SERVICE}}:{{IMAGE_TAG}} localhost:{{REGISTRY_PORT}}/rs-{{SERVICE}}:{{IMAGE_TAG}}
    podman push localhost:{{REGISTRY_PORT}}/rs-{{SERVICE}}:{{IMAGE_TAG}} --tls-verify=false
    @kubectl rollout restart deployment/rs-{{SERVICE}} -n angzarr 2>/dev/null || true
    @kubectl rollout status deployment/rs-{{SERVICE}} -n angzarr --timeout=60s 2>/dev/null || true

# Helm upgrade
helm-upgrade:
    helm upgrade --install angzarr "{{TOP}}/deploy/helm/angzarr" \
        -f "{{TOP}}/deploy/helm/angzarr/values-local.yaml" \
        --namespace angzarr --create-namespace

# Uninstall helm release
helm-uninstall:
    helm uninstall angzarr --namespace angzarr || true
