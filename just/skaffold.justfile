# Skaffold development commands
#
# UNIFIED WORKFLOW (with lefthook):
#   1. Make changes
#   2. Commit (lefthook triggers skaffold run automatically)
#   3. Done!
#
# MANUAL WORKFLOW:
#   just examples dev rust    - watch mode, rebuilds on file change
#   just examples run rust    - build and deploy once
#
# All images use sha256 content-based tags:
#   - New content = new tag = K8s pulls fresh image (even with IfNotPresent)
#   - Same content = same tag = skaffold cache hit (no rebuild)
#
# Setup lefthook: lefthook install

TOP := `git rev-parse --show-toplevel`

# One-time setup: configure Podman and Skaffold for local registry
init:
    @echo "Configuring Podman for local registry..."
    @uv run "{{TOP}}/scripts/configure_podman_registry.py"
    @echo "Configuring Skaffold for Kind..."
    @uv run "{{TOP}}/scripts/configure_skaffold.py"
    @echo ""
    @echo "Setup complete!"

# Check if Podman and Skaffold are configured
check:
    @echo "Checking Podman registry configuration..."
    @uv run "{{TOP}}/scripts/configure_podman_registry.py" --check || true
    @echo "Checking Skaffold configuration..."
    @uv run "{{TOP}}/scripts/configure_skaffold.py" --check || true

# Build angzarr framework images only (builder + 5 final images)
framework-build:
    @echo "Building angzarr framework images..."
    @echo "  1. angzarr-builder (compiles all binaries)"
    @echo "  2. 5 final images in parallel (just copy binaries)"
    skaffold build
    @echo ""
    @echo "Framework images built. Now run 'just examples dev' for business logic."

# Watch and rebuild framework images on change
framework-dev:
    @echo "Starting framework dev loop..."
    @echo "NOTE: For business logic changes, use 'just examples dev' in another terminal."
    skaffold dev

# Build and deploy once with skaffold (framework only)
run:
    skaffold run

# Delete skaffold deployment
delete:
    skaffold delete || true

# Render skaffold manifests (dry-run)
render:
    skaffold render
