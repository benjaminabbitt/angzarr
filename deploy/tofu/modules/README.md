# Angzarr OpenTofu Modules

Infrastructure-as-code modules for deploying Angzarr domains across multiple cloud providers.

## Module Architecture

All domain modules share a common configuration structure:

- **Business Config (Portable)**: Defines WHAT the domain does - same across all providers
- **Operational Config (Provider-specific)**: Defines HOW it runs - uses native provider patterns

### Business Config Variables

#### `domain` (string, required)

Domain name identifier. Used in resource naming and service discovery.

```hcl
domain = "order"  # Results in: order-aggregate, saga-order-fulfillment, etc.
```

#### `aggregate` (object)

Command handler for this domain. Mutually exclusive with `process_manager`.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | `false` | Enable aggregate deployment |
| `env` | map(string) | `{}` | Environment variables for logic container |
| `upcaster.enabled` | bool | `false` | Enable upcaster sidecar for event migration |
| `upcaster.env` | map(string) | `{}` | Environment variables for upcaster container |

```hcl
aggregate = {
  enabled = true
  env = {
    "DATABASE_POOL_SIZE" = "10"
  }
  upcaster = {
    enabled = true
    env = {
      "MIGRATION_VERSION" = "3"
    }
  }
}
```

#### `process_manager` (object)

Cross-domain orchestrator. Mutually exclusive with `aggregate`.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | `false` | Enable process manager deployment |
| `source_domains` | list(string) | `[]` | Domains this PM subscribes to for events |
| `env` | map(string) | `{}` | Environment variables for logic container |

```hcl
process_manager = {
  enabled        = true
  source_domains = ["order", "inventory", "fulfillment"]
  env = {
    "TIMEOUT_SECONDS" = "300"
  }
}
```

#### `sagas` (map of objects)

Event translators that bridge this domain to other domains. Map key is the saga name.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `target_domain` | string | required | Domain this saga sends commands to |
| `env` | map(string) | `{}` | Environment variables for logic container |

```hcl
sagas = {
  fulfillment = {
    target_domain = "fulfillment"
    env = {}
  }
  notifications = {
    target_domain = "notification"
    env = {
      "TEMPLATE_ID" = "order-confirmation"
    }
  }
}
```

#### `projectors` (map of objects)

Read model builders that consume events from this domain. Map key is the projector name.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `env` | map(string) | `{}` | Environment variables for logic container |

```hcl
projectors = {
  web = {
    env = {
      "REDIS_URL"    = "redis://cache:6379"
      "CACHE_TTL_MS" = "30000"
    }
  }
  analytics = {
    env = {
      "BIGQUERY_DATASET" = "events"
    }
  }
}
```

## Available Modules

### Domain Modules

| Module | Provider | Description |
|--------|----------|-------------|
| `domain/` | GCP Cloud Run | Serverless containers on GCP |
| `fargate-domain/` | AWS Fargate | Serverless containers on AWS ECS |
| `eks-domain/` | AWS EKS | Kubernetes on AWS |
| `gke-domain/` | GCP GKE | Kubernetes on GCP |

### Supporting Modules

| Module | Provider | Description |
|--------|----------|-------------|
| `fargate-base/` | AWS | VPC, ECS cluster, ALB, IAM roles |
| `fargate-ecr/` | AWS | ECR container registry |
| `fargate-infrastructure/` | AWS | Stream and Topology services |
| `fargate-registry/` | AWS | Service discovery aggregation |

## Module Details

### GCP Cloud Run (`domain/`)

Uses Google Cloud Run multi-container services with sidecar pattern.

**Operational Config Highlights:**
- `scaling.aggregate.min_instances` / `max_instances` - Cloud Run autoscaling
- `scaling.sagas` - Map of per-saga scaling (name → config)
- `scaling.projectors` - Map of per-projector scaling (name → config)
- `networking.vpc_connector` - VPC access connector
- `execution.environment` - Gen1 or Gen2 execution environment
- `iam.allow_unauthenticated` - Public access control

### AWS Fargate (`fargate-domain/`)

Uses ECS Fargate tasks with multi-container task definitions.

**Operational Config Highlights:**
- `scaling.aggregate.cpu` / `memory` - Fargate task CPU/memory (integers)
- `scaling.sagas` - Map of per-saga scaling
- `cluster_arn` - ECS cluster ARN
- `subnet_ids` - VPC subnet IDs
- `execution_role_arn` - Task execution role
- `coordinator_secrets` - Secrets Manager ARNs

### Kubernetes EKS (`eks-domain/`)

Uses Kubernetes Deployments, Services, and HPA.

**Operational Config Highlights:**
- `scaling.aggregate.replicas` / `min_replicas` / `max_replicas`
- `scaling.aggregate.resources.requests` / `limits` - K8s resource specs
- `scaling.sagas` - Map of per-saga scaling
- `namespace` - Kubernetes namespace
- `service_type` - ClusterIP, LoadBalancer, etc.
- `coordinator_secrets` - K8s Secret references

### Kubernetes GKE (`gke-domain/`)

Same as EKS with GKE-specific features.

**Additional Operational Config:**
- `workload_identity.enabled` - Enable GKE Workload Identity
- `workload_identity.gcp_service_account` - GCP SA for Workload Identity

## Container Architecture

Each component deploys 3-4 containers in a sidecar pattern:

```
┌─────────────────────────────────────────────────────────┐
│                    Pod/Task/Service                      │
├─────────────┬─────────────┬──────────────┬──────────────┤
│ grpc-gateway│ coordinator │    logic     │  upcaster    │
│   :8080     │   :1310     │   :50053     │   :50054     │
│  (REST API) │ (framework) │ (business)   │  (optional)  │
└─────────────┴─────────────┴──────────────┴──────────────┘
```

## Usage Example

```hcl
module "order_domain" {
  source = "./modules/eks-domain"

  # Business config (portable)
  domain = "order"

  aggregate = {
    enabled = true
    env = {
      "DATABASE_URL" = "postgres://..."
    }
    upcaster = {
      enabled = true
    }
  }

  sagas = {
    fulfillment = {
      target_domain = "fulfillment"
      env = {}
    }
  }

  projectors = {
    web = {
      env = {
        "REDIS_URL" = "redis://..."
      }
    }
  }

  # Operational config (provider-specific)
  namespace = "angzarr"

  images = {
    grpc_gateway          = "gcr.io/project/grpc-gateway:v1"
    coordinator_aggregate = "gcr.io/project/coordinator-aggregate:v1"
    coordinator_saga      = "gcr.io/project/coordinator-saga:v1"
    coordinator_projector = "gcr.io/project/coordinator-projector:v1"
    coordinator_pm        = "gcr.io/project/coordinator-pm:v1"
    logic                 = "gcr.io/project/order-logic:v1"
    upcaster              = "gcr.io/project/order-upcaster:v1"
    saga_logic = {
      fulfillment = "gcr.io/project/saga-order-fulfillment:v1"
    }
    projector_logic = {
      web = "gcr.io/project/projector-order-web:v1"
    }
  }

  scaling = {
    aggregate = {
      replicas     = 2
      min_replicas = 1
      max_replicas = 10
      resources = {
        requests = { cpu = "200m", memory = "256Mi" }
        limits   = { cpu = "2", memory = "1Gi" }
      }
    }
    sagas = {
      fulfillment = {
        replicas     = 1
        min_replicas = 1
        max_replicas = 5
      }
    }
  }

  coordinator_env = {
    "STORAGE_TYPE" = "postgres"
    "BUS_TYPE"     = "kafka"
  }

  coordinator_secrets = {
    "DATABASE_PASSWORD" = {
      secret_name = "db-credentials"
      key         = "password"
    }
  }
}
```

## Outputs

All domain modules provide consistent outputs:

| Output | Description |
|--------|-------------|
| `discovery_entries` | Map of env var name → service endpoint |
| `discovery_json` | Structured discovery data for JSON serialization |
| `aggregate_service_name` | Service name for the aggregate |
| `saga_service_names` | Map of saga name → service name |
| `projector_service_names` | Map of projector name → service name |

## Per-Component Scaling

Sagas and projectors support per-component scaling configuration:

```hcl
scaling = {
  # All sagas default to these values
  sagas = {
    # Override for specific saga
    high_volume = {
      replicas     = 3
      min_replicas = 2
      max_replicas = 20
      resources = {
        limits = { cpu = "2", memory = "1Gi" }
      }
    }
    # Other sagas use module defaults
  }
}
```

Components not listed in the map use sensible defaults.
