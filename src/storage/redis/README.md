# Redis Storage Backend

**Status: Implemented and tested**

Full storage backend with integration tests via testcontainers (Redis 7).

## Overview

Complete `EventStore`, `SnapshotStore`, `PositionStore`, and `TopologyStore` implementations using sorted sets for event ordering and key-value storage for snapshots via [redis-rs](https://crates.io/crates/redis).

## Feature Flag

```toml
cargo build --features redis
```

## Configuration

```yaml
storage:
  type: redis
  path: redis://localhost:6379
```

## Key Structure

```
{prefix}:{domain}:{edition}:{root}:events     -- Sorted set of events by sequence
{prefix}:{domain}:{edition}:{root}:snapshot   -- Latest snapshot (binary)
{prefix}:correlation:{correlation_id}         -- Set of event references (domain:edition:root:sequence)
{prefix}:position:{handler}:{domain}:{edition}:{root_hex}  -- Handler position checkpoint
{prefix}:topology:nodes                       -- Hash of topology nodes
{prefix}:topology:edges                       -- Hash of topology edges
```

Default prefix: `angzarr`.

## What's Implemented

- `RedisEventStore` -- append, query by domain/root, sequence-based range queries, correlation ID queries, edition support with composite reads
- `RedisSnapshotStore` -- save/load snapshots keyed by domain + edition + root
- `RedisPositionStore` -- handler checkpoint tracking
- `RedisTopologyStore` -- node/edge storage for topology visualization
- Connection pooling via `ConnectionManager`
- Configurable key prefix

## Testing

Integration tests run against a real Redis instance via testcontainers:

```bash
cargo test --features redis --test storage_redis
```
