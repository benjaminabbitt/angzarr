---
sidebar_position: 2
---

# Kafka

Apache Kafka provides **high-throughput** event streaming with log retention for replay and analytics.

---

## Why Kafka

| Strength | Benefit |
|----------|---------|
| **High throughput** | Millions of events/second |
| **Log retention** | Events persist for replay |
| **Partitioning** | Horizontal scaling |
| **Ecosystem** | Kafka Streams, Connect, ksqlDB |
| **Ordering** | Per-partition ordering guarantees |

---

## Trade-offs

| Concern | Consideration |
|---------|---------------|
| **Operational complexity** | ZooKeeper (or KRaft) coordination |
| **Resource heavy** | More memory/disk than AMQP |
| **Latency** | Batching adds milliseconds |

---

## Configuration

```toml
[bus]
backend = "kafka"

[bus.kafka]
brokers = ["localhost:9092"]
topic_prefix = "angzarr"
consumer_group = "angzarr-handlers"
```

### Environment Variables

```bash
export KAFKA_BROKERS="localhost:9092"
export KAFKA_TOPIC_PREFIX="angzarr"
export BUS_BACKEND="kafka"
```

---

## Topic Layout

Events are partitioned by aggregate root for ordering:

```
Topic: angzarr.events.player
  Partition 0: [player-001 events, player-004 events, ...]
  Partition 1: [player-002 events, player-005 events, ...]
  Partition 2: [player-003 events, player-006 events, ...]

Topic: angzarr.events.hand
  Partition 0: [hand-001 events, ...]
  ...
```

### Partitioning Strategy

Events partition by `hash(domain + root) % partitions`. All events for the same aggregate land in the same partition, preserving order.

---

## Consumer Groups

Handlers join consumer groups for load balancing:

```
Consumer Group: player-projector
  Consumer 1 ← Partition 0, 1
  Consumer 2 ← Partition 2, 3

Consumer Group: output-projector
  Consumer 1 ← Partition 0, 1, 2, 3
```

Each partition is consumed by exactly one consumer per group.

---

## Retention

Configure retention for replay capabilities:

```properties
# Topic-level configuration
retention.ms=604800000  # 7 days
retention.bytes=-1      # Unlimited by size
```

Events remain available for replay within the retention window.

---

## Helm Deployment

```yaml
# values.yaml
bus:
  backend: kafka

kafka:
  enabled: true
  brokers:
    - kafka-0.kafka.messaging.svc.cluster.local:9092
    - kafka-1.kafka.messaging.svc.cluster.local:9092
  topicPrefix: angzarr
```

---

## Testing

```bash
# Run Kafka tests (requires testcontainers)
cargo test --test bus_kafka --features kafka

# Requires podman socket
systemctl --user start podman.socket
```

---

## Monitoring

Key metrics to monitor:

| Metric | Concern |
|--------|---------|
| Consumer lag | Processing falling behind |
| Under-replicated partitions | Replication issues |
| Request latency | Broker performance |
| Disk usage | Retention capacity |

---

## When to Use Kafka

- **High volume** — Millions of events/second
- **Event replay** — Analytics, debugging, new projectors
- **Cross-team sharing** — Multiple teams consuming events
- **Stream processing** — Kafka Streams, ksqlDB

---

## Next Steps

- **[AMQP](/tooling/buses/amqp)** — Simpler alternative
- **[Pub/Sub](/tooling/buses/pubsub)** — GCP managed alternative
