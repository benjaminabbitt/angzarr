---
sidebar_position: 2
---

# Redis

Redis provides **lowest latency** for high-throughput scenarios where some durability trade-offs are acceptable.

---

## Why Redis

| Strength | Benefit |
|----------|---------|
| **Sub-millisecond latency** | Fastest event writes |
| **Horizontal scaling** | Redis Cluster for partitioning |
| **Memory-first** | No disk I/O on hot path |
| **Pub/Sub native** | Can double as event bus |

---

## Trade-offs

| Concern | Consideration |
|---------|---------------|
| **Durability** | Configurable (AOF, RDB, none) |
| **Memory cost** | Events live in RAM until evicted |
| **Complexity** | Cluster mode adds operational overhead |

---

## Configuration

```toml
[storage]
backend = "redis"

[storage.redis]
url = "redis://localhost:6379"
# For cluster mode
# urls = ["redis://node1:6379", "redis://node2:6379"]
```

### Environment Variables

```bash
export REDIS_URL="redis://localhost:6379"
export STORAGE_BACKEND="redis"
```

---

## Data Model

Redis stores events in sorted sets keyed by aggregate:

```
events:{domain}:{edition}:{root}
  └── ZADD with sequence as score
      └── Member: serialized event

positions:{handler}:{domain}:{edition}:{root}
  └── String: sequence number

snapshots:{domain}:{edition}:{root}
  └── Hash: { sequence, state }
```

---

## Durability Options

### AOF (Append-Only File)

```
# redis.conf
appendonly yes
appendfsync everysec  # fsync every second (default)
# appendfsync always  # fsync every write (slower, safest)
```

### RDB Snapshots

```
# redis.conf
save 900 1    # Save if 1 key changed in 900 seconds
save 300 10   # Save if 10 keys changed in 300 seconds
save 60 10000 # Save if 10000 keys changed in 60 seconds
```

### Production Recommendation

Use both AOF and RDB for durability with fast recovery:

```
appendonly yes
appendfsync everysec
save 900 1
```

---

## Helm Deployment

```yaml
# values.yaml
storage:
  backend: redis

redis:
  enabled: true
  host: redis.cache.svc.cluster.local
  port: 6379
  credentials:
    secretName: redis-credentials
    passwordKey: password
```

---

## Testing

```bash
# Run Redis tests (requires testcontainers)
cargo test --test storage_redis --features redis

# Requires podman socket
systemctl --user start podman.socket
```

---

## When to Use Redis

- **Gaming/real-time** — Latency-critical applications
- **Ephemeral data** — Acceptable to lose on crash
- **Caching layer** — In front of durable storage
- **Development** — Fast iteration without disk I/O

---

## Next Steps

- **[PostgreSQL](/tooling/databases/postgres)** — Durable alternative
- **[Testcontainers](/tooling/testcontainers)** — Container-based testing
