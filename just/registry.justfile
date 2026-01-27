# Registry image lifecycle commands

TOP := `git rev-parse --show-toplevel`
REGISTRY_PORT := "5001"

# Show registry status (image count per repository)
status:
    uv run "{{TOP}}/scripts/registry_cleanup.py" status

# List all images in registry
list:
    uv run "{{TOP}}/scripts/registry_cleanup.py" list

# Clean all sha256 tags (keep named tags like 'latest')
clean-sha256:
    uv run "{{TOP}}/scripts/registry_cleanup.py" clean-sha256

# Delete ALL images from registry (full reset)
clean-all:
    @echo "WARNING: Deleting ALL registry images in 5 seconds..."
    @sleep 5
    uv run "{{TOP}}/scripts/registry_cleanup.py" clean-all

# Run garbage collection (reclaim disk after deletes)
gc:
    uv run "{{TOP}}/scripts/registry_cleanup.py" gc

# Clean sha256 tags + GC (typical cleanup)
prune: clean-sha256 gc

# Dry run - show what clean-sha256 would delete
clean-dry:
    uv run "{{TOP}}/scripts/registry_cleanup.py" clean-sha256 --dry-run

# Push image to local registry (faster than kind load)
push IMAGE:
    podman tag {{IMAGE}} localhost:{{REGISTRY_PORT}}/{{IMAGE}}
    podman push localhost:{{REGISTRY_PORT}}/{{IMAGE}} --tls-verify=false
