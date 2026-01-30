# Redis Storage Backend

**Status: Implemented, untested**

This module has not yet been integration tested against a running Redis instance. MongoDB and SQLite are the tested storage backends. Use Redis at your own risk until integration tests are passing.

## Overview

Full `EventStore` and `SnapshotStore` implementation using sorted sets for event ordering and key-value storage for snapshots via [redis-rs](https://crates.io/crates/redis).

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
{prefix}:{domain}:{root}:events    -- Sorted set of events by sequence
{prefix}:{domain}:{root}:snapshot  -- Latest snapshot (binary)
{prefix}:{domain}:roots            -- Set of all root IDs in domain
{prefix}:domains                   -- Set of all domains
```

Default prefix: `angzarr`.

## What's Implemented

- `RedisEventStore` -- append, query by domain/root, sequence-based range queries
- `RedisSnapshotStore` -- save/load snapshots keyed by domain + root
- Connection pooling via `ConnectionManager`
- Configurable key prefix

## Known Gaps

- No integration tests against a real Redis instance
- `get_by_correlation()` not implemented (correlation_id is not indexed in the key structure)
