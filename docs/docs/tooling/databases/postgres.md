---
sidebar_position: 1
---

# PostgreSQL

PostgreSQL is the **production default** for angzarr. ACID guarantees, mature tooling, and familiar operations make it the safest choice for most deployments.

---

## Why PostgreSQL

| Strength | Benefit |
|----------|---------|
| **ACID transactions** | Events never partially commit |
| **Strong durability** | fsync by default, no data loss |
| **Mature tooling** | pg_dump, pg_restore, pgAdmin, extensions |
| **Operational familiarity** | Most teams already know PostgreSQL |
| **Vertical scaling** | Single instance handles most workloads |

---

## Configuration

```toml
[storage]
backend = "postgres"

[storage.postgres]
url = "postgres://user:pass@localhost:5432/angzarr"
pool_size = 10
connect_timeout_seconds = 5
```

### Environment Variables

```bash
export DATABASE_URL="postgres://user:pass@localhost:5432/angzarr"
export STORAGE_BACKEND="postgres"
```

---

## Schema

Angzarr auto-migrates on startup. The schema includes:

```sql
-- Events table (append-only)
CREATE TABLE events (
    id UUID PRIMARY KEY,
    domain VARCHAR(255) NOT NULL,
    edition VARCHAR(255) NOT NULL,
    root UUID NOT NULL,
    sequence BIGINT NOT NULL,
    event_type VARCHAR(255) NOT NULL,
    payload BYTEA NOT NULL,
    correlation_id VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (domain, edition, root, sequence)
);

-- Positions table (handler checkpoints)
CREATE TABLE positions (
    handler VARCHAR(255) NOT NULL,
    domain VARCHAR(255) NOT NULL,
    edition VARCHAR(255) NOT NULL,
    root UUID NOT NULL,
    sequence BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (handler, domain, edition, root)
);

-- Snapshots table (aggregate state cache)
CREATE TABLE snapshots (
    domain VARCHAR(255) NOT NULL,
    edition VARCHAR(255) NOT NULL,
    root UUID NOT NULL,
    sequence BIGINT NOT NULL,
    state BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (domain, edition, root)
);
```

---

## Optimistic Concurrency

The unique constraint on `(domain, edition, root, sequence)` enforces optimistic concurrency:

```sql
-- Two concurrent writes to same aggregate with same sequence
INSERT INTO events (..., sequence) VALUES (..., 5);  -- First succeeds
INSERT INTO events (..., sequence) VALUES (..., 5);  -- Second fails: unique violation
```

The second writer must reload state and retry with the correct sequence.

---

## Helm Deployment

```yaml
# values.yaml
storage:
  backend: postgres

postgres:
  enabled: true
  host: postgres.database.svc.cluster.local
  port: 5432
  database: angzarr
  credentials:
    secretName: postgres-credentials
    usernameKey: username
    passwordKey: password
```

---

## Testing

```bash
# Run PostgreSQL tests (requires testcontainers)
cargo test --test storage_postgres --features postgres

# Requires podman socket
systemctl --user start podman.socket
```

---

## Next Steps

- **[Testcontainers](/tooling/testcontainers)** — Container-based testing
- **[Redis](/tooling/databases/redis)** — Alternative for high-throughput
