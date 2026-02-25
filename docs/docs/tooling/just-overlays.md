---
sidebar_position: 4
---

# just Overlays

Platform-specific justfiles enable cross-platform development while keeping tasks consistent.

---

## The Pattern

The root justfile imports platform-specific overlays:

```just
# justfile (root)
import? 'justfile.linux'
import? 'justfile.darwin'
import? 'justfile.windows'
import? 'justfile.container'
```

The `?` makes imports optional—missing files don't cause errors.

---

## Overlay Hierarchy

```
project/
├── justfile              # Cross-platform recipes
├── justfile.linux        # Linux-specific overrides
├── justfile.darwin       # macOS-specific overrides
├── justfile.windows      # Windows-specific overrides
└── justfile.container    # Container environment overrides
```

---

## How Overlays Work

### Base Recipe

```just
# justfile
build:
    cargo build --release
```

### Platform Override

```just
# justfile.linux
build:
    cargo build --release --target x86_64-unknown-linux-musl
```

When running on Linux, `just build` uses the overlay definition. On other platforms, it falls back to the base recipe.

---

## Container Overlay

For containerized builds where the same command should work on host and inside containers, see the **[Container Overlay Pattern](/docs/tooling/container-overlay)**. This technique mounts a container-specific build file over the host file, eliminating conditionals entirely.

---

## Common Patterns

### Tool Availability

```just
# justfile.darwin
brew-deps:
    brew install just skaffold helm kubectl

# justfile.linux
apt-deps:
    sudo apt-get install -y just
    # Manual install for others...
```

### Path Differences

```just
# justfile.linux
socket := "/run/podman/podman.sock"

# justfile.darwin
socket := "~/.colima/default/docker.sock"
```

### Binary Names

```just
# justfile.linux
container-runtime := "podman"

# justfile.darwin
container-runtime := "docker"
```

---

## Example: Angzarr

Angzarr uses overlays for container runtime differences:

```just
# justfile.container
# Inside devcontainer, use different registry access
registry := "ghcr.io/angzarr-io"

# justfile (base)
registry := "ghcr.io/angzarr-io"
```

---

## Best Practices

1. **Keep base portable** — Base justfile should work on most platforms
2. **Override only what differs** — Don't duplicate recipes unnecessarily
3. **Document requirements** — Note platform-specific tool dependencies
4. **Test overlays** — CI should test all supported platforms

---

## Next Steps

- **[just](/docs/tooling/just)** — Core just commands
- **[Container Overlay Pattern](/docs/tooling/container-overlay)** — Mount-based build file swapping
- **[Getting Started](/docs/getting-started)** — Full development setup
