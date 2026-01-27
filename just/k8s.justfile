# Kubernetes operations commands

# Port forward evented service
port-forward:
    kubectl port-forward -n angzarr svc/evented 1313:1313 1314:1314

# View k8s logs
logs:
    kubectl logs -n angzarr -l app.kubernetes.io/name=evented -f

# === Service Discovery / DNS ===

# List all services with their DNS names
svc-list:
    @echo "Services in evented namespace:"
    @kubectl get svc -n angzarr -o custom-columns='NAME:.metadata.name,DNS:.metadata.annotations.evented\.io/dns-name,CLUSTER-IP:.spec.clusterIP,PORTS:.spec.ports[*].port'

# Test DNS resolution from within the cluster
svc-dns-test SERVICE:
    @kubectl run dns-test --rm -it --restart=Never --image=busybox:1.36 -n angzarr -- nslookup {{SERVICE}}.angzarr.svc.cluster.local

# Show service endpoints
svc-endpoints:
    @kubectl get endpoints -n angzarr

# === Ingress Controller ===

# Install nginx-ingress controller for Kind
ingress-install:
    kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/main/deploy/static/provider/kind/deploy.yaml
    @echo "Waiting for ingress controller to be ready..."
    kubectl wait --namespace ingress-nginx \
        --for=condition=ready pod \
        --selector=app.kubernetes.io/component=controller \
        --timeout=120s

# Check ingress status
ingress-status:
    @kubectl get pods -n ingress-nginx
    @echo "---"
    @kubectl get ingress -n angzarr
