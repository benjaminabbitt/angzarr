# PostgreSQL Storage Backend

**Status: Implemented, untested**

This module has not yet been integration tested against a running PostgreSQL instance. MongoDB and SQLite are the tested storage backends. Use PostgreSQL at your own risk until integration tests are passing.

## Overview

Full `EventStore` and `SnapshotStore` implementation using [sea-query](https://crates.io/crates/sea-query) for SQL generation and [sqlx](https://crates.io/crates/sqlx) for async query execution.

## Feature Flag

```toml
cargo build --features postgres
```

## Configuration

```yaml
storage:
  type: postgres
  path: postgres://user:pass@localhost:5432/angzarr
```

## What's Implemented

- `PostgresEventStore` -- append, query by domain/root, sequence validation, correlation ID lookups, temporal queries
- `PostgresSnapshotStore` -- save/load snapshots keyed by domain + root
- Schema auto-initialization via `init()` (creates tables and indexes if not present)
- Primary key on `(domain, root, sequence)` for optimistic concurrency

## Known Gaps

- No integration tests against a real PostgreSQL instance
- Schema migration tooling not provided (initial `CREATE TABLE IF NOT EXISTS` only)
