---
sidebar_position: 3
---

# Google Cloud Pub/Sub

Google Cloud Pub/Sub provides **managed messaging** for event sourcing on GCP with automatic scaling.

---

## Why Pub/Sub

| Strength | Benefit |
|----------|---------|
| **Fully managed** | No infrastructure to operate |
| **Global** | Multi-region by default |
| **Automatic scaling** | Handles traffic spikes |
| **GCP integration** | Dataflow, BigQuery, Cloud Functions |
| **At-least-once** | Guaranteed delivery |

---

## Trade-offs

| Concern | Consideration |
|---------|---------------|
| **GCP lock-in** | Not portable to other clouds |
| **Cost** | Pay per message/data |
| **Ordering** | Requires ordering keys |

---

## Configuration

```toml
[bus]
backend = "pubsub"

[bus.pubsub]
project_id = "my-gcp-project"
topic_prefix = "angzarr"
```

### Environment Variables

```bash
export PUBSUB_PROJECT_ID="my-gcp-project"
export PUBSUB_TOPIC_PREFIX="angzarr"
export BUS_BACKEND="pubsub"
export GOOGLE_APPLICATION_CREDENTIALS="/path/to/service-account.json"
```

---

## Topic/Subscription Layout

```
Topic: angzarr-events-player
  ├── Subscription: player-projector
  ├── Subscription: output-projector
  └── Subscription: topology-tracker

Topic: angzarr-events-hand
  ├── Subscription: hand-saga
  └── Subscription: output-projector
```

### Message Ordering

Enable ordering with ordering keys:

```rust
// Publish with ordering key (aggregate root)
publisher.publish_with_ordering_key(
    event_bytes,
    &format!("{}#{}", domain, root),
).await?;
```

---

## Subscription Configuration

```yaml
# Terraform/gcloud configuration
subscription:
  name: player-projector
  topic: angzarr-events-player
  ack_deadline_seconds: 60
  message_retention_duration: 604800s  # 7 days
  enable_message_ordering: true
  dead_letter_policy:
    dead_letter_topic: angzarr-dlq
    max_delivery_attempts: 5
```

---

## Dead Letter Handling

Failed messages route to a dead letter topic:

```
angzarr-dlq (topic)
  └── dlq-monitor (subscription)
```

Monitor via Cloud Console or subscribe programmatically.

---

## Setup

```bash
# Create topics
gcloud pubsub topics create angzarr-events-player
gcloud pubsub topics create angzarr-events-hand
gcloud pubsub topics create angzarr-dlq

# Create subscriptions
gcloud pubsub subscriptions create player-projector \
  --topic=angzarr-events-player \
  --enable-message-ordering \
  --dead-letter-topic=angzarr-dlq \
  --max-delivery-attempts=5
```

---

## Helm Deployment

```yaml
# values.yaml
bus:
  backend: pubsub

pubsub:
  enabled: true
  projectId: my-gcp-project
  credentials:
    secretName: pubsub-credentials
    keyFile: service-account.json
```

---

## When to Use Pub/Sub

- **GCP native** — Already on Google Cloud
- **Serverless** — Cloud Functions/Run integration
- **Global reach** — Multi-region by default
- **Managed** — No operational overhead

---

## Next Steps

- **[SNS/SQS](/tooling/buses/sns-sqs)** — AWS equivalent
- **[Kafka](/tooling/buses/kafka)** — Self-managed alternative
