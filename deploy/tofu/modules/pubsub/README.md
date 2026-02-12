# Pub/Sub Module

Creates Google Cloud Pub/Sub resources for the angzarr event bus.

## Overview

This module creates:
- Main events topic (all domain events)
- Per-domain topics (optional filtering)
- Push subscriptions for sagas/projectors (delivers to Cloud Run)
- Pull subscription for debugging/replay
- Dead letter topic for failed messages

## Architecture

```
Aggregates → [Events Topic] → Push Subscription → Saga Cloud Run Service
                           → Push Subscription → Projector Cloud Run Service
                           → Pull Subscription → Debugging/Replay
                           → Dead Letter Topic → Failed Messages
```

## Usage

### Basic Usage

```hcl
module "eventbus" {
  source = "../modules/pubsub"

  project_id        = var.project_id
  events_topic_name = "angzarr-events"

  # Publishers (aggregates that emit events)
  publishers = [
    "serviceAccount:${module.order_aggregate.service_account_email}",
    "serviceAccount:${module.inventory_aggregate.service_account_email}",
  ]

  # Push subscribers (sagas/projectors)
  push_subscribers = {
    "saga-order-fulfillment" = {
      endpoint        = "${module.saga_order_fulfillment.url}/events"
      service_account = module.saga_order_fulfillment.service_account_email
      type            = "saga"
      domain_filter   = "order"  # Only receive order domain events
    }
    "projector-inventory" = {
      endpoint        = "${module.projector_inventory.url}/events"
      service_account = module.projector_inventory.service_account_email
      type            = "projector"
      domain_filter   = "inventory"
    }
  }

  # Enable dead letter for reliability
  enable_dead_letter = true
}
```

### With Per-Domain Topics

For high-volume deployments, create separate topics per domain:

```hcl
module "eventbus" {
  source = "../modules/pubsub"

  project_id           = var.project_id
  events_topic_name    = "angzarr-events"
  domains              = ["order", "inventory", "fulfillment"]
  create_domain_topics = true

  # Subscribers can now subscribe to domain-specific topics
  # (managed separately)
}
```

### With Pull Subscription for Debugging

```hcl
module "eventbus" {
  source = "../modules/pubsub"

  project_id               = var.project_id
  events_topic_name        = "angzarr-events"
  create_pull_subscription = true

  # ...
}

# Pull messages for debugging:
# gcloud pubsub subscriptions pull angzarr-events-pull --limit=10 --auto-ack
```

## Message Format

Events are published with attributes for filtering:

```json
{
  "data": "<base64-encoded protobuf>",
  "attributes": {
    "domain": "order",
    "event_type": "OrderCreated",
    "aggregate_id": "uuid",
    "sequence": "1",
    "correlation_id": "workflow-uuid"
  }
}
```

## Filtering

Push subscriptions can filter by domain:

```hcl
push_subscribers = {
  "saga-order-fulfillment" = {
    endpoint      = "..."
    domain_filter = "order"  # Only receives order events
  }
}
```

Filter syntax follows [Pub/Sub filtering](https://cloud.google.com/pubsub/docs/filtering):
- `attributes.domain = "order"`
- `attributes.event_type = "OrderCreated"`

## Dead Letter Handling

Failed messages (after max retries) go to dead letter topic:

1. Monitor dead letter subscription for failures
2. Investigate and fix issues
3. Replay messages using `gcloud` or replay tool

```bash
# Monitor dead letters
gcloud pubsub subscriptions pull angzarr-events-dead-letter-sub --limit=10
```

## IAM

The module sets up:
- **Publishers**: Service accounts that can publish to the events topic
- **Subscribers**: Push subscriptions authenticate via OIDC

```hcl
module "eventbus" {
  # ...
  publishers = [
    "serviceAccount:order-aggregate@project.iam.gserviceaccount.com"
  ]
  push_subscriber_accounts = [
    "serviceAccount:pubsub@project.iam.gserviceaccount.com"
  ]
  grant_cloudrun_invoker = true  # Allow Pub/Sub to invoke Cloud Run
}
```

## Outputs

| Output | Description |
|--------|-------------|
| `events_topic_id` | Main events topic ID |
| `events_topic_name` | Main events topic name |
| `domain_topics` | Map of domain→topic ID (if created) |
| `dead_letter_topic_id` | Dead letter topic ID |
| `push_subscriptions` | Map of subscriber→subscription ID |
| `pull_subscription_id` | Pull subscription ID |
| `coordinator_env` | Env vars for angzarr coordinator |

## Integration with Cloud Run

```hcl
module "saga" {
  source = "../modules/cloudrun-service"

  # ...

  coordinator_env = merge(
    module.eventbus.coordinator_env,
    {
      SUBSCRIPTION_NAME = "saga-order-fulfillment"
    }
  )
}
```
