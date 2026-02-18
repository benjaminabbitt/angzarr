---
sidebar_position: 5
---

# NATS

NATS provides **lightweight, high-performance** messaging with simple operations and cloud-native design.

---

## Why NATS

| Strength | Benefit |
|----------|---------|
| **Lightweight** | Single binary, minimal resources |
| **Fast** | Sub-millisecond latency |
| **Simple** | Minimal configuration |
| **Cloud native** | Built for Kubernetes |
| **JetStream** | Persistent streaming option |

---

## Trade-offs

| Concern | Consideration |
|---------|---------------|
| **Less mature** | Smaller ecosystem than Kafka/RabbitMQ |
| **Durability** | Requires JetStream for persistence |

---

## Configuration

```toml
[bus]
backend = "nats"

[bus.nats]
url = "nats://localhost:4222"
# For JetStream (persistent)
jetstream = true
stream_name = "angzarr-events"
```

### Environment Variables

```bash
export NATS_URL="nats://localhost:4222"
export BUS_BACKEND="nats"
```

---

## Subject Layout

NATS uses dot-separated subjects for hierarchical routing:

```
angzarr.events.player.PlayerRegistered
angzarr.events.player.FundsDeposited
angzarr.events.hand.CardsDealt
```

### Subscriptions

Wildcards enable flexible subscriptions:

```
angzarr.events.player.*    # All player events
angzarr.events.*.>         # All events
angzarr.events.hand.>      # All hand events and sub-subjects
```

---

## Core NATS vs JetStream

### Core NATS (Fire and Forget)
- Pub/sub without persistence
- At-most-once delivery
- Lowest latency
- Use for ephemeral events

### JetStream (Persistent)
- Stream persistence
- At-least-once delivery
- Consumer replay
- Use for event sourcing

```toml
# Enable JetStream for event sourcing
[bus.nats]
jetstream = true
```

---

## JetStream Configuration

```bash
# Create stream
nats stream add angzarr-events \
  --subjects "angzarr.events.>" \
  --retention limits \
  --max-msgs -1 \
  --max-bytes -1 \
  --max-age 7d \
  --storage file \
  --replicas 3

# Create consumer
nats consumer add angzarr-events player-projector \
  --filter "angzarr.events.player.>" \
  --ack explicit \
  --deliver all \
  --max-deliver 5 \
  --replay instant
```

---

## Docker Setup

```bash
# Core NATS
docker run -p 4222:4222 nats:latest

# NATS with JetStream
docker run -p 4222:4222 nats:latest -js
```

---

## Helm Deployment

```yaml
# values.yaml
bus:
  backend: nats

nats:
  enabled: true
  url: nats://nats.messaging.svc.cluster.local:4222
  jetstream: true
  streamName: angzarr-events
```

---

## Monitoring

NATS provides a monitoring endpoint:

```
http://localhost:8222/varz    # Server stats
http://localhost:8222/connz   # Connections
http://localhost:8222/subsz   # Subscriptions
http://localhost:8222/jsz     # JetStream stats
```

---

## When to Use NATS

- **Edge computing** — Minimal resource footprint
- **Kubernetes native** — Designed for containers
- **Simple operations** — Single binary, little config
- **High performance** — Sub-millisecond latency

---

## Next Steps

- **[AMQP](/tooling/buses/amqp)** — More features, larger ecosystem
- **[Kafka](/tooling/buses/kafka)** — Higher throughput alternative
