---
sidebar_position: 1
---

# just

[just](https://github.com/casey/just) is a command runner used throughout Angzarr. It provides consistent task execution across the project.

---

## Installation

```bash
# Via cargo
cargo install just

# macOS
brew install just

# Debian/Ubuntu
sudo apt install just
```

---

## Core Commands

| Command | Description |
|---------|-------------|
| `just build` | Build the framework |
| `just check` | Fast compile check |
| `just test` | Run unit tests |
| `just fmt` | Format code |
| `just lint` | Run clippy lints |
| `just watch` | Start bacon (background checker) |

---

## Cluster Management

| Command | Description |
|---------|-------------|
| `just cluster-create` | Create Kind cluster with local registry |
| `just cluster-status` | Show cluster and registry status |
| `just cluster-delete` | Delete Kind cluster |
| `just nuke-deploy` | Delete everything, rebuild from scratch |

---

## Deployment (Skaffold)

| Command | Description |
|---------|-------------|
| `just deploy` | Full deployment: cluster + infra + build + deploy |
| `just dev` | Watch mode: auto-rebuild on file changes |
| `just fresh-deploy` | Regenerate protos, bust caches, rebuild |

---

## Testing

| Command | Description |
|---------|-------------|
| `just test` | Run unit tests |
| `just integration` | Run integration tests |
| `just acceptance` | Run acceptance tests |
| `just test-interfaces` | Run interface contract tests (SQLite) |
| `just test-interfaces-postgres` | Run interface tests against PostgreSQL |
| `just test-interfaces-all` | Run interface tests against all backends |

---

## Example-Specific Commands

```bash
# Access example commands
just examples build     # Build all examples
just examples test      # Test all examples
just examples fmt       # Format all examples
```

---

## Next Steps

- **[just Overlays](/tooling/just-overlays)** — Platform-specific justfile patterns
- **[Getting Started](/getting-started)** — Full CLI reference
