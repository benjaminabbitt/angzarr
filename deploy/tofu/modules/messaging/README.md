# Messaging Module

Deploys RabbitMQ or Kafka via Bitnami Helm charts for Angzarr event messaging.

## Usage

```hcl
module "messaging" {
  source = "github.com/benjaminabbitt/angzarr//deploy/tofu/modules/messaging?ref=v0.1.0"

  type         = "rabbitmq"  # or "kafka"
  namespace    = "angzarr"
  release_name = "angzarr-mq"

  # Optional: Provide password or let module auto-generate
  # password = var.mq_password
}
```

## Inputs

| Name | Description | Type | Default | Required |
|------|-------------|------|---------|----------|
| type | Broker type: `rabbitmq` or `kafka` | string | - | yes |
| managed | Use cloud-managed broker instead of Helm | bool | `false` | no |
| release_name | Helm release name | string | `"angzarr-mq"` | no |
| namespace | Kubernetes namespace | string | `"angzarr"` | no |
| username | Broker username | string | `"angzarr"` | no |
| password | Broker password (auto-generated if null) | string | `null` | no |
| kafka_sasl_enabled | Enable SASL for Kafka | bool | `false` | no |
| persistence_enabled | Enable persistent storage | bool | `true` | no |
| persistence_size | Persistent volume size | string | `"8Gi"` | no |
| metrics_enabled | Enable Prometheus metrics | bool | `true` | no |

## Outputs

| Name | Description |
|------|-------------|
| host | Broker host |
| port | Broker port |
| uri | Connection URI (sensitive) |
| username | Broker username |
| password | Broker password (sensitive) |
| secret_name | Kubernetes secret name |
| type | Broker type |

## External/Managed Brokers

For cloud-managed message brokers (Amazon MQ, Confluent, etc.), set `managed = true`:

```hcl
module "messaging" {
  source = "github.com/benjaminabbitt/angzarr//deploy/tofu/modules/messaging?ref=v0.1.0"

  type    = "rabbitmq"
  managed = true

  external_host = "my-broker.mq.us-east-1.amazonaws.com"
  external_port = 5671
  external_uri  = "amqps://user:pass@my-broker.mq.us-east-1.amazonaws.com:5671"
}
```

## Requirements

| Name | Version |
|------|---------|
| terraform/opentofu | >= 1.0 |
| helm | ~> 2.0 |
| kubernetes | ~> 2.0 |
| random | ~> 3.0 |
