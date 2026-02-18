---
sidebar_position: 3
---

# SQLite

SQLite is the **standalone/embedded** default. Single-file database with zero configuration, perfect for development and single-process deployments.

---

## Why SQLite

| Strength | Benefit |
|----------|---------|
| **Zero setup** | No server, no network |
| **Single file** | Easy backup, copy, share |
| **ACID compliant** | Full transaction support |
| **Fastest for local** | No network round-trips |
| **Testing** | Perfect for unit/integration tests |

---

## Limitations

| Limitation | Impact |
|------------|--------|
| **Single writer** | No concurrent write scaling |
| **Local only** | Can't share across machines |
| **File locking** | Potential issues on network filesystems |

---

## Configuration

```toml
[storage]
backend = "sqlite"

[storage.sqlite]
path = "./angzarr.db"
# Or in-memory for testing
# path = ":memory:"
```

### Environment Variables

```bash
export SQLITE_PATH="./angzarr.db"
export STORAGE_BACKEND="sqlite"
```

---

## File Locations

```
./angzarr.db              # Default location
:memory:                   # In-memory (lost on exit)
/var/lib/angzarr/data.db  # Production path
```

---

## WAL Mode

Angzarr enables WAL (Write-Ahead Logging) by default for better concurrency:

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
```

Benefits:
- Readers don't block writers
- Writers don't block readers
- Better crash recovery

---

## Testing

SQLite is the default for test suites:

```bash
# Run interface tests against SQLite
cargo test --test interfaces

# Fast, no containers needed
cargo test --lib
```

---

## Standalone Mode

Standalone deployments use SQLite by default:

```bash
# Start with SQLite storage
angzarr standalone --storage sqlite --db-path ./data/events.db
```

All three stores (events, positions, snapshots) live in the same file.

---

## Backup

Simple file copy when database is quiet:

```bash
# Safe backup (uses SQLite backup API)
sqlite3 angzarr.db ".backup backup.db"

# Or just copy during low activity
cp angzarr.db angzarr.db.backup
```

For hot backups, use the SQLite backup API or filesystem snapshots.

---

## When to Use SQLite

- **Development** — Fast iteration, no setup
- **Testing** — Isolated, deterministic
- **Embedded** — Single-process applications
- **Prototyping** — Get started immediately
- **Edge** — Limited resources, no network

---

## Next Steps

- **[PostgreSQL](/tooling/databases/postgres)** — Production scaling
- **[Testing](/operations/testing)** — SQLite in test suites
