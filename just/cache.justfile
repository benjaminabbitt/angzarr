# Container cache management commands

# Show podman disk usage and cache status
status:
    @echo "=== Podman System Disk Usage ==="
    podman system df
    @echo ""
    @echo "=== Build Cache Volumes ==="
    podman volume ls --filter name=buildah

# Prune unused images (keeps cache mounts)
prune:
    @echo "Pruning dangling images..."
    podman image prune -f
    @echo "Pruning stopped containers..."
    podman container prune -f
    @echo ""
    podman system df

# Prune old images (older than 4h)
prune-old:
    @echo "Pruning images older than 4 hours..."
    podman image prune -f --filter "until=4h"
    @echo ""
    podman system df

# Aggressive prune - removes all unused images and build caches
prune-all:
    @echo "WARNING: This will remove ALL unused images and build caches."
    @echo "Press Ctrl+C within 5 seconds to cancel..."
    @sleep 5
    podman system prune -af --volumes
    @echo ""
    podman system df

# EMERGENCY: Stop podman and wipe all storage (nuclear option)
# Works even with zero disk space by directly removing storage directories
nuke:
    @echo "!!! EMERGENCY WIPE !!!"
    @echo "This will STOP all containers and DELETE all podman data."
    @echo "Press Ctrl+C within 10 seconds to cancel..."
    @sleep 10
    @echo "Stopping all containers..."
    -podman stop -a -t 0 2>/dev/null
    -podman rm -af 2>/dev/null
    @echo "Attempting podman reset..."
    -podman system reset --force 2>/dev/null
    @echo "Direct storage removal (works when disk full)..."
    rm -rf ~/.local/share/containers/storage/* 2>/dev/null || true
    rm -rf ~/.local/share/containers/cache/* 2>/dev/null || true
    rm -rf ~/.config/containers/podman/machine/* 2>/dev/null || true
    @echo "Clearing buildah cache..."
    -podman volume rm -af 2>/dev/null
    rm -rf ~/.local/share/buildah 2>/dev/null || true
    @echo "Storage wiped."
    @echo "Recreate cluster with: just kind create-registry"

# Clear all build caches (skaffold artifact cache + podman layer cache)
clear:
    @echo "Clearing skaffold cache..."
    rm -f ~/.skaffold/cache
    @echo "Clearing podman build cache for angzarr images..."
    podman image prune -f --filter label=angzarr=true 2>/dev/null || true
    @echo "Cache cleared."
