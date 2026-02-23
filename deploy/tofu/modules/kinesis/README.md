# Kinesis Module

AWS Kinesis Data Streams for angzarr event bus.

> **Note:** Rust bus implementation for Kinesis is TBD. This module provisions
> infrastructure in preparation. See `src/bus/README.md` for design notes.

## Architecture

```
┌──────────────┐     PutRecord      ┌─────────────────────────────┐
│  Aggregate   │───────────────────▶│  angzarr-events-{domain}    │
└──────────────┘   partition_key:   │  (Kinesis Data Stream)      │
                   root_id          └─────────────┬───────────────┘
                                                  │
                    ┌─────────────────────────────┼─────────────────────────────┐
                    │                             │                             │
                    ▼                             ▼                             ▼
           ┌───────────────┐             ┌───────────────┐             ┌───────────────┐
           │     Saga      │             │   Projector   │             │      PM       │
           │  (Consumer)   │             │  (Consumer)   │             │  (Consumer)   │
           └───────────────┘             └───────────────┘             └───────────────┘
```

### Key Design Decisions

- **Per-domain streams**: `angzarr-events-{domain}` isolates domains for independent scaling
- **Partition key = root_id**: Events from same aggregate land in same shard, preserving order
- **Enhanced fan-out**: Optional dedicated throughput per consumer (2 MB/s vs shared 2 MB/s)
- **DLQ streams**: `angzarr-dlq-{domain}` for failed event capture

### When to Use Kinesis vs SNS/SQS

| Factor | Kinesis | SNS/SQS |
|--------|---------|---------|
| Replay | Yes (retention up to 1 year) | No (once consumed, gone) |
| Ordering | Per-shard (partition key) | FIFO queues only |
| Multiple consumers | Each reads independently | Each gets copy (fan-out) |
| Throughput | 1 MB/s write, 2 MB/s read per shard | Higher limits |
| Latency | ~200ms (standard), ~70ms (enhanced) | ~20-50ms |
| Cost model | Per-shard-hour + data | Per-request |

**Use Kinesis when:**
- You need replay capability
- Multiple consumers need independent cursors
- Analytics/Firehose integration required
- Log-style append-only semantics preferred

**Use SNS/SQS when:**
- Lower latency required
- Simpler operational model preferred
- No replay requirements
- Cost-sensitive with bursty traffic

## Usage

```hcl
module "kinesis" {
  source = "../../modules/kinesis"

  domains = ["order", "inventory", "fulfillment"]

  # On-demand scaling (recommended for variable workloads)
  stream_mode = "ON_DEMAND"

  # Or fixed capacity for predictable workloads
  # stream_mode = "PROVISIONED"
  # shard_count = 2

  # Retention for replay (default 24h, max 8760h/1year)
  retention_hours = 168  # 7 days

  # Enhanced fan-out for latency-sensitive consumers
  enhanced_fanout_consumers = {
    "saga-order-fulfillment" = { domain = "order" }
  }

  # Alerting
  enable_alarms = true
  alarm_actions = [aws_sns_topic.alerts.arn]

  tags = {
    Environment = "staging"
  }
}

# Attach producer policy to aggregate task roles
resource "aws_iam_role_policy_attachment" "aggregate_producer" {
  for_each   = toset(["order", "inventory", "fulfillment"])
  role       = module.domains[each.key].task_role_name
  policy_arn = module.kinesis.producer_policy_arn
}

# Attach consumer policy to saga/projector task roles
resource "aws_iam_role_policy_attachment" "saga_consumer" {
  for_each   = toset(["saga-order-fulfillment"])
  role       = module.sagas[each.key].task_role_name
  policy_arn = module.kinesis.consumer_policy_arn
}
```

## Inputs

| Name | Description | Type | Default | Required |
|------|-------------|------|---------|:--------:|
| domains | List of domains to create streams for | `list(string)` | n/a | yes |
| stream_prefix | Prefix for stream names | `string` | `"angzarr"` | no |
| stream_mode | ON_DEMAND or PROVISIONED | `string` | `"ON_DEMAND"` | no |
| shard_count | Shards per stream (PROVISIONED only) | `number` | `1` | no |
| retention_hours | Data retention (24-8760) | `number` | `24` | no |
| encryption_type | NONE or KMS | `string` | `"KMS"` | no |
| kms_key_id | Custom KMS key ARN | `string` | `null` | no |
| enable_dlq | Create DLQ streams | `bool` | `true` | no |
| dlq_retention_hours | DLQ retention period | `number` | `168` | no |
| enhanced_fanout_consumers | Map of consumer name to domain | `map(object)` | `{}` | no |
| enable_alarms | Create CloudWatch alarms | `bool` | `true` | no |
| alarm_actions | SNS topic ARNs for alerts | `list(string)` | `[]` | no |
| tags | Tags for all resources | `map(string)` | `{}` | no |

## Outputs

| Name | Description |
|------|-------------|
| stream_arns | Map of domain to stream ARN |
| stream_names | Map of domain to stream name |
| dlq_stream_arns | Map of domain to DLQ stream ARN |
| producer_policy_arn | IAM policy ARN for producers |
| consumer_policy_arn | IAM policy ARN for consumers |
| dlq_consumer_policy_arn | IAM policy ARN for DLQ consumers |
| coordinator_env | Environment variables for angzarr |
| messaging_uri | `kinesis://<prefix>` URI |

## Rust Implementation Status

**Status: TBD**

The Kinesis bus backend needs to be implemented in Rust. Design notes from `src/bus/README.md`:

- Streams per domain: `angzarr-events-{domain}`
- Partition key: Aggregate root ID (ordering within aggregate)
- Consumer: Kinesis Consumer Library (KCL) pattern
- DLQ: `angzarr-dlq-{domain}` stream

Feature flag will be: `kinesis = ["dep:aws-sdk-kinesis", "dep:aws-config"]`
