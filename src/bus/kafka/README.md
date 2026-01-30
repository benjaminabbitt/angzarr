# Kafka Event Bus

**Status: Implemented, untested**

This module has not yet been integration tested against a running Kafka cluster. RabbitMQ (AMQP) is the tested message bus. Use Kafka at your own risk until integration tests are passing.

## Overview

Async `EventBus` implementation using [rdkafka](https://crates.io/crates/rdkafka) (librdkafka wrapper). Publishes events to per-domain topics with aggregate root ID as the message key for partition-level ordering.

## Feature Flag

```toml
cargo build --features kafka
```

## Configuration

```yaml
bus:
  type: kafka
  kafka:
    brokers: kafka-1:9092,kafka-2:9092
    topic_prefix: angzarr
    group_id: my-consumer-group
```

## Topic Naming

```
{topic_prefix}.events.{domain}
```

Message key: hex-encoded root UUID (ensures per-aggregate ordering within a partition).

## What's Implemented

- `KafkaEventBus` -- publish and subscribe with consumer groups
- SASL authentication (PLAIN, SCRAM-SHA-256, SCRAM-SHA-512)
- SSL/TLS connections
- Idempotent producer settings
- Domain-based topic routing

## Known Gaps

- No integration tests against a real Kafka cluster
- No dead-letter queue configuration
- No schema registry integration
