# Redis Module

Deploys Redis via Bitnami Helm chart for caching and session storage.

## Usage

```hcl
module "redis" {
  source = "github.com/benjaminabbitt/angzarr//deploy/tofu/modules/redis?ref=v0.1.0"

  namespace    = "angzarr"
  release_name = "angzarr-redis"

  # Optional: Provide password or let module auto-generate
  # password = var.redis_password
}
```

## Inputs

| Name | Description | Type | Default | Required |
|------|-------------|------|---------|----------|
| managed | Use cloud-managed Redis instead of Helm | bool | `false` | no |
| release_name | Helm release name | string | `"angzarr-redis"` | no |
| namespace | Kubernetes namespace | string | `"angzarr"` | no |
| auth_enabled | Enable Redis authentication | bool | `true` | no |
| password | Redis password (auto-generated if null) | string | `null` | no |
| replica_count | Number of replicas (0 for standalone) | number | `0` | no |
| persistence_enabled | Enable persistent storage | bool | `true` | no |
| persistence_size | Persistent volume size | string | `"2Gi"` | no |
| metrics_enabled | Enable Prometheus metrics | bool | `true` | no |

## Outputs

| Name | Description |
|------|-------------|
| host | Redis host |
| port | Redis port |
| uri | Connection URI (sensitive) |
| password | Redis password (sensitive) |
| secret_name | Kubernetes secret name |

## External/Managed Redis

For cloud-managed Redis (ElastiCache, Redis Cloud, etc.), set `managed = true`:

```hcl
module "redis" {
  source = "github.com/benjaminabbitt/angzarr//deploy/tofu/modules/redis?ref=v0.1.0"

  managed = true

  external_host = "my-redis.xxx.cache.amazonaws.com"
  external_port = 6379
  external_uri  = "redis://:password@my-redis.xxx.cache.amazonaws.com:6379"
}
```

## Requirements

| Name | Version |
|------|---------|
| terraform/opentofu | >= 1.0 |
| helm | ~> 2.0 |
| kubernetes | ~> 2.0 |
| random | ~> 3.0 |
