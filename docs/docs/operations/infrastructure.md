---
sidebar_position: 3
---

# Infrastructure

Angzarr uses modular Helm charts for Kubernetes and OpenTofu for cloud infrastructure. Each database and message bus is deployed separately from the core application.

---

## Available Charts

| Chart | Description |
|-------|-------------|
| `angzarr-db-postgres` | PostgreSQL event store (wraps Bitnami) |
| `angzarr-mq-rabbitmq` | RabbitMQ message bus (wraps Bitnami) |
| `angzarr-mq-kafka` | Kafka message bus (wraps Bitnami) |

---

## Deployment Order

1. Deploy infrastructure charts first
2. Deploy core angzarr chart with applications

```bash
# Create namespace
kubectl create namespace angzarr

# Deploy database
helm install angzarr-db ./deploy/helm/angzarr-db-postgres -n angzarr \
  --set postgresql.auth.postgresPassword=<root-password> \
  --set postgresql.auth.password=<app-password>

# Deploy message bus
helm install angzarr-mq ./deploy/helm/angzarr-mq-rabbitmq -n angzarr \
  --set rabbitmq.auth.password=<password>

# Deploy application
helm install angzarr ./deploy/helm/angzarr -n angzarr \
  -f ./deploy/helm/angzarr/values-rust.yaml
```

---

## PostgreSQL Configuration

### values.yaml

```yaml
postgresql:
  auth:
    postgresPassword: ""  # Required
    username: "angzarr"
    password: ""          # Required
    database: "angzarr"
  primary:
    persistence:
      size: 8Gi
```

### Connection String

```yaml
# In angzarr values
storage:
  postgres:
    uri: "postgres://angzarr:<password>@angzarr-db-postgresql:5432/angzarr"
```

---

## RabbitMQ Configuration

### values.yaml

```yaml
rabbitmq:
  auth:
    username: "angzarr"
    password: ""  # Required
  persistence:
    size: 1Gi
```

### Connection String

```yaml
# In angzarr values
messaging:
  type: amqp
  amqp:
    url: "amqp://angzarr:<password>@angzarr-mq-rabbitmq:5672/%2F"
```

---

## Kafka Configuration

### values.yaml

```yaml
kafka:
  controller:
    replicaCount: 1
    persistence:
      size: 2Gi
  broker:
    replicaCount: 1
    persistence:
      size: 2Gi
```

### Connection String

```yaml
# In angzarr values
messaging:
  type: kafka
  kafka:
    bootstrapServers: "angzarr-mq-kafka:9092"
```

---

## Skaffold Deployment

For local development, skaffold deploys infrastructure automatically:

```yaml
# skaffold.yaml
deploy:
  helm:
    releases:
      # Infrastructure first
      - name: angzarr-db
        chartPath: ../../deploy/helm/angzarr-db-postgres
        namespace: angzarr
        setValues:
          postgresql.auth.postgresPassword: dev-password
          postgresql.auth.password: dev-password

      - name: angzarr-mq
        chartPath: ../../deploy/helm/angzarr-mq-rabbitmq
        namespace: angzarr
        setValues:
          rabbitmq.auth.password: dev-password

      # Then application
      - name: angzarr
        chartPath: ../../deploy/helm/angzarr
        namespace: angzarr
        valuesFiles:
          - ../../deploy/helm/angzarr/values-rust.yaml
```

---

## Why Modular Charts?

1. **Independent lifecycle** — Upgrade databases without redeploying apps
2. **Flexibility** — Choose only the infrastructure you need
3. **Production parity** — Same charts for dev and prod
4. **Clear dependencies** — Explicit deployment order

---

---

## OpenTofu

For cloud infrastructure provisioning, angzarr provides OpenTofu modules:

```
deploy/tofu/
├── modules/
│   ├── gcp/          # GCP resources (Bigtable, Pub/Sub, GKE)
│   ├── aws/          # AWS resources (DynamoDB, SNS/SQS, EKS)
│   └── common/       # Cloud-agnostic patterns
└── environments/
    ├── dev/
    ├── staging/
    └── prod/
```

### Example: GCP Bigtable

```hcl
module "angzarr_storage" {
  source = "./modules/gcp/bigtable"

  project_id  = var.project_id
  instance_id = "angzarr-events"
  zone        = "us-central1-a"

  tables = {
    events    = { column_families = ["e"] }
    positions = { column_families = ["p"] }
    snapshots = { column_families = ["s"] }
  }
}
```

### Why OpenTofu

- **Open source** — Community-driven fork of Terraform
- **Compatible** — Works with existing Terraform providers
- **No license restrictions** — BSL-free

---

## Next Steps

- **[Observability](/operations/observability)** — Monitoring and tracing
- **[Databases](/tooling/databases/postgres)** — Database configuration details
