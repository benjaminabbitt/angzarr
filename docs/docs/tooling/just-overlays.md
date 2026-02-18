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

The container overlay handles devcontainer environments where paths differ:

```just
# justfile.container
TOP := "/workspace"

deploy:
    skaffold run -f {{TOP}}/skaffold.yaml
```

The container overlay sets absolute paths that work inside containers, overriding the host's relative paths.

### Detection

The overlay is imported when `DEVCONTAINER=true` is set:

```just
# justfile
TOP := `git rev-parse --show-toplevel`

# Container-aware import
import? 'justfile.container'
```

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
registry := "host.docker.internal:5001"

# justfile (base)
registry := "localhost:5001"
```

---

## Best Practices

1. **Keep base portable** — Base justfile should work on most platforms
2. **Override only what differs** — Don't duplicate recipes unnecessarily
3. **Document requirements** — Note platform-specific tool dependencies
4. **Test overlays** — CI should test all supported platforms

---

## Next Steps

- **[just](/tooling/just)** — Core just commands
- **[Getting Started](/getting-started)** — Full development setup
