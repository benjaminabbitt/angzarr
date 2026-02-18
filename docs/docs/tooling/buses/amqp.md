---
sidebar_position: 1
---

# AMQP (RabbitMQ)

AMQP via RabbitMQ is the **production default** for distributed deployments. Mature, widely deployed, with excellent tooling.

---

## Why AMQP

| Strength | Benefit |
|----------|---------|
| **Mature** | 15+ years of production use |
| **Durable queues** | Messages survive broker restarts |
| **Flexible routing** | Topic exchanges, headers, fanout |
| **Management UI** | Built-in web console |
| **Dead letter queues** | Automatic failure handling |

---

## Configuration

```toml
[bus]
backend = "amqp"

[bus.amqp]
url = "amqp://guest:guest@localhost:5672"
exchange = "angzarr.events"
prefetch_count = 10
```

### Environment Variables

```bash
export AMQP_URL="amqp://guest:guest@localhost:5672"
export BUS_BACKEND="amqp"
```

---

## Exchange Topology

Angzarr creates a topic exchange for event routing:

```
Exchange: angzarr.events (topic)
    │
    ├── Binding: player.# → queue: player-projector
    ├── Binding: player.# → queue: output-projector
    ├── Binding: hand.# → queue: hand-saga
    └── Binding: *.# → queue: topology-tracker
```

### Routing Keys

Events are published with routing keys: `{domain}.{event_type}`

```
player.PlayerRegistered
player.FundsDeposited
hand.CardsDealt
hand.HandComplete
```

---

## Queue Configuration

Each handler gets a dedicated queue with configurable durability:

```yaml
# Handler queue settings
queues:
  player-projector:
    durable: true
    auto_delete: false
    exclusive: false
    arguments:
      x-dead-letter-exchange: angzarr.dlx
```

---

## Dead Letter Handling

Failed messages route to a dead letter exchange:

```
angzarr.dlx (fanout)
    │
    └── angzarr.dlq (queue)
```

Monitor the DLQ for processing failures:

```bash
# Check DLQ depth
rabbitmqctl list_queues name messages | grep dlq
```

---

## Helm Deployment

```yaml
# values.yaml
bus:
  backend: amqp

amqp:
  enabled: true
  host: rabbitmq.messaging.svc.cluster.local
  port: 5672
  credentials:
    secretName: rabbitmq-credentials
    usernameKey: username
    passwordKey: password
```

---

## Testing

```bash
# Run AMQP tests (requires testcontainers)
cargo test --test bus_amqp --features amqp

# Requires podman socket
systemctl --user start podman.socket
```

---

## Management Console

RabbitMQ includes a web management interface:

```
http://localhost:15672
Default credentials: guest/guest
```

Monitor:
- Queue depths
- Message rates
- Consumer connections
- Exchange bindings

---

## When to Use AMQP

- **Production default** — Most deployments
- **Existing RabbitMQ** — Already in infrastructure
- **Complex routing** — Headers, topics, fanout
- **Operational maturity** — Team familiarity

---

## Next Steps

- **[Kafka](/tooling/buses/kafka)** — High-throughput alternative
- **[Testcontainers](/tooling/testcontainers)** — Container-based testing
