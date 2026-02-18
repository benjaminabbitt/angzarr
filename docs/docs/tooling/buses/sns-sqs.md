---
sidebar_position: 4
---

# AWS SNS/SQS

Amazon SNS (Simple Notification Service) and SQS (Simple Queue Service) provide **managed messaging** for event sourcing on AWS.

---

## Why SNS/SQS

| Strength | Benefit |
|----------|---------|
| **Fully managed** | No infrastructure to operate |
| **Serverless** | Lambda integration |
| **Cost effective** | Pay per request |
| **High availability** | Multi-AZ by default |
| **Fan-out** | SNS → multiple SQS queues |

---

## Trade-offs

| Concern | Consideration |
|---------|---------------|
| **AWS lock-in** | Not portable to other clouds |
| **256 KB limit** | Large events need S3 offloading |
| **Ordering** | FIFO queues required for ordering |

---

## Configuration

```toml
[bus]
backend = "sns_sqs"

[bus.sns_sqs]
region = "us-east-1"
topic_prefix = "angzarr"
```

### Environment Variables

```bash
export AWS_REGION="us-east-1"
export SNS_SQS_TOPIC_PREFIX="angzarr"
export BUS_BACKEND="sns_sqs"
# AWS credentials via standard mechanisms
```

---

## Topology

SNS provides fan-out to SQS queues:

```
SNS Topic: angzarr-events-player
    │
    ├── SQS: player-projector-queue
    ├── SQS: output-projector-queue
    └── SQS: topology-tracker-queue

SNS Topic: angzarr-events-hand
    │
    ├── SQS: hand-saga-queue
    └── SQS: output-projector-queue
```

---

## FIFO vs Standard

### Standard Queues
- At-least-once delivery
- Best-effort ordering
- Higher throughput

### FIFO Queues
- Exactly-once processing
- Strict ordering within message groups
- 300 TPS limit (3000 with batching)

For event sourcing, FIFO queues with message group ID = aggregate root:

```rust
sqs.send_message()
    .queue_url(&fifo_queue_url)
    .message_body(&event_json)
    .message_group_id(&format!("{}#{}", domain, root))
    .message_deduplication_id(&event_id)
    .send()
    .await?;
```

---

## Large Event Handling

Events exceeding 256 KB use S3 for payload storage:

```
1. Store payload in S3
2. Publish reference to SNS/SQS
3. Consumer fetches from S3
```

This is the "claim check" pattern, handled automatically by `OffloadingEventBus`.

---

## Dead Letter Queues

Configure DLQs for failed messages:

```yaml
# CloudFormation/Terraform
PlayerProjectorDLQ:
  Type: AWS::SQS::Queue
  Properties:
    QueueName: player-projector-dlq

PlayerProjectorQueue:
  Type: AWS::SQS::Queue
  Properties:
    QueueName: player-projector-queue
    RedrivePolicy:
      deadLetterTargetArn: !GetAtt PlayerProjectorDLQ.Arn
      maxReceiveCount: 5
```

---

## Setup

```bash
# Create SNS topics
aws sns create-topic --name angzarr-events-player
aws sns create-topic --name angzarr-events-hand

# Create SQS queues
aws sqs create-queue --queue-name player-projector-queue

# Subscribe queue to topic
aws sns subscribe \
  --topic-arn arn:aws:sns:us-east-1:123456789:angzarr-events-player \
  --protocol sqs \
  --notification-endpoint arn:aws:sqs:us-east-1:123456789:player-projector-queue
```

---

## Helm Deployment

```yaml
# values.yaml
bus:
  backend: sns_sqs

sns_sqs:
  enabled: true
  region: us-east-1
  topicPrefix: angzarr
  # IAM role-based auth recommended
```

---

## When to Use SNS/SQS

- **AWS native** — Already on AWS
- **Lambda integration** — Serverless event handlers
- **Cost sensitive** — Pay-per-use model
- **Managed** — No operational overhead

---

## Next Steps

- **[Pub/Sub](/tooling/buses/pubsub)** — GCP equivalent
- **[AMQP](/tooling/buses/amqp)** — Self-managed alternative
