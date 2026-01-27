# Secrets management commands

TOP := `git rev-parse --show-toplevel`

# Generate and store secure secrets (idempotent - won't overwrite existing)
init:
    uv run "{{TOP}}/scripts/manage_secrets.py" init

# Force regenerate all secrets (credential rotation)
rotate:
    uv run "{{TOP}}/scripts/manage_secrets.py" rotate

# Show current secrets (from K8s secret store, masked)
show:
    uv run "{{TOP}}/scripts/manage_secrets.py" show

# Show current secrets (full values revealed)
reveal:
    uv run "{{TOP}}/scripts/manage_secrets.py" show --reveal

# Check if secrets exist
check:
    uv run "{{TOP}}/scripts/manage_secrets.py" check

# Sync secrets to target namespace (for Bitnami charts without ESO)
sync *ARGS:
    uv run "{{TOP}}/scripts/manage_secrets.py" sync {{ARGS}}

# === External Secrets Operator ===

# Install External Secrets Operator
eso-install:
    helm repo add external-secrets https://charts.external-secrets.io || true
    helm repo update
    helm upgrade --install external-secrets external-secrets/external-secrets \
        --namespace external-secrets \
        --create-namespace \
        --set installCRDs=true \
        --wait

# Full ESO setup (install + generate secrets)
eso-setup: eso-install init

# Check ESO status
eso-status:
    @kubectl get pods -n external-secrets 2>/dev/null || echo "ESO not installed"
    @echo "---"
    @kubectl get secretstores,externalsecrets -n angzarr 2>/dev/null || echo "No ESO resources in evented namespace"
