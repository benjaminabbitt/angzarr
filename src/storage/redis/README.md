# Redis Storage Backend

**Status: Implemented and tested**

Storage backend for snapshot caching via [redis-rs](https://crates.io/crates/redis).

## Why Only Snapshots?

Redis is **not** used for event storage or position tracking. Both require strong durability guarantees that Redis cannot provide by default (periodic RDB snapshots and optional AOF can lose recent writes on crash).

Use Postgres, SQLite, or NATS for events and positions.

Redis excels at **snapshot caching**: reconstructed aggregate state for faster reads. Snapshots are an optimization—if lost, they can be rebuilt from events.

## Feature Flag

```toml
cargo build --features redis
```

## Configuration

```yaml
storage:
  redis:
    uri: redis://localhost:6379
```

## Key Structure

```
{prefix}:{domain}:{edition}:{root}:snapshot  -- Snapshot (binary)
```

Default prefix: `angzarr`.

## What's Implemented

- `RedisSnapshotStore` -- save/load snapshots keyed by domain + edition + root
- Connection pooling via `ConnectionManager`
- Configurable key prefix

## Testing

Integration tests run against a real Redis instance via testcontainers:

```bash
cargo test --features redis --test storage_redis
```
