---
sidebar_position: 3
---

# Infrastructure

Angzarr uses modular Helm charts for Kubernetes and OpenTofu for cloud infrastructure. Each database and message bus is deployed separately from the core application using official images.

## Deployment Modes

| Mode | Infrastructure | Best For |
|------|---------------|----------|
| **Standalone** | SQLite + Channel bus | Development, testing |
| **Local K8s** | Kind/k3s + Helm | Integration testing |
| **GCP Cloud Run** | Cloud SQL + Pub/Sub | Serverless production |
| **GCP GKE** | Cloud SQL + Helm | K8s production on GCP |
| **AWS Fargate** | RDS + SNS/SQS | Serverless production on AWS |
| **AWS EKS** | RDS + Helm | K8s production on AWS |

See **[OpenTofu](/tooling/opentofu)** for complete deployment guides for each target.

---

## Available Charts

| Chart | Description | Operator |
|-------|-------------|----------|
| `postgres` | PostgreSQL event store | CloudNative-PG |
| `rabbitmq` | RabbitMQ message bus | RabbitMQ Cluster Operator |
| `kafka` | Kafka message bus | Strimzi |
| `nats` | NATS message bus | - |
| `redis` | Redis cache | - |

All charts use official upstream images, not Bitnami.

---

## Deployment Order

1. Deploy operators (for PostgreSQL and RabbitMQ)
2. Deploy infrastructure charts
3. Deploy core angzarr chart with applications

```bash title="illustrative - deployment order"
# Create namespace
kubectl create namespace angzarr

# Deploy operators
helm install angzarr-operators ./deploy/k8s/helm/operators -n operators --create-namespace

# Deploy database
helm install angzarr-db ./deploy/k8s/helm/postgres -n angzarr

# Deploy message bus
helm install angzarr-mq ./deploy/k8s/helm/rabbitmq -n angzarr

# Deploy application
helm install angzarr ./deploy/k8s/helm/angzarr -n angzarr \
  -f ./deploy/k8s/helm/angzarr/values-rust.yaml
```

---

## PostgreSQL Configuration

Uses CloudNative-PG operator with official PostgreSQL image.

### values.yaml

```yaml title="illustrative - PostgreSQL values"
name: angzarr-db
image:
  repository: ghcr.io/cloudnative-pg/postgresql
  tag: "16.4"
instances: 1
database:
  name: angzarr
  owner: angzarr
storage:
  size: 8Gi
```

### Connection String

CloudNative-PG creates services: `<name>-rw` (read-write), `<name>-ro` (read-only).

```yaml title="illustrative - PostgreSQL connection"
# In angzarr values
storage:
  postgres:
    uri: "postgres://angzarr:<password>@angzarr-db-rw:5432/angzarr"
```

---

## RabbitMQ Configuration

Uses RabbitMQ Cluster Operator with official RabbitMQ image.

### values.yaml

```yaml title="illustrative - RabbitMQ values"
name: angzarr-mq
image:
  repository: rabbitmq
  tag: "4.1-management"
replicas: 1
storage:
  size: 1Gi
```

### Connection String

```yaml title="illustrative - RabbitMQ connection"
# In angzarr values
messaging:
  type: amqp
  amqp:
    url: "amqp://guest:guest@angzarr-mq:5672/%2F"
```

---

## Kafka Configuration

Uses Strimzi operator with official Apache Kafka image.

### values.yaml

```yaml title="illustrative - Kafka values"
name: angzarr-kafka
version: "3.9.0"
kafka:
  replicas: 1
  storage:
    size: 2Gi
controller:
  replicas: 1
```

### Connection String

```yaml title="illustrative - Kafka connection"
# In angzarr values
messaging:
  type: kafka
  kafka:
    bootstrapServers: "angzarr-kafka-kafka-bootstrap:9092"
```

---

## Kind Deployment

For local development with Kind:

```bash title="illustrative - Kind deployment"
# Create cluster and deploy infrastructure
just -f deploy/kind/justfile up

# Or step by step:
just -f deploy/kind/justfile create
just -f deploy/kind/justfile infra-standard
just -f deploy/kind/justfile framework
```

---

## k3s/OpenTofu Deployment

For local development with k3s and OpenTofu:

```bash title="illustrative - k3s/OpenTofu deployment"
cd deploy/tofu/environments/k3s
tofu init
tofu apply
```

This uses the `infra-*` modules which deploy official images directly (no operators required).

---

## Why Modular Charts?

1. **Independent lifecycle** — Upgrade databases without redeploying apps
2. **Official images** — No third-party wrappers or Bitnami dependencies
3. **Flexibility** — Choose only the infrastructure you need
4. **Production parity** — Same charts for dev and prod
5. **Clear dependencies** — Explicit deployment order

---

## OpenTofu

For cloud infrastructure provisioning, angzarr provides OpenTofu modules supporting multiple deployment targets.

See **[OpenTofu](/tooling/opentofu)** for complete deployment guides including:
- Standalone mode
- Local Kubernetes (Kind/k3s)
- GCP Cloud Run
- GCP GKE
- AWS Fargate
- AWS EKS

### Quick Reference

```bash title="illustrative - OpenTofu environments"
# Local k3s
cd deploy/tofu/environments/k3s && tofu apply

# GCP Cloud Run
cd deploy/tofu/environments/gcp && tofu apply

# AWS Fargate
cd deploy/tofu/environments/aws-staging && tofu apply
```

### Why OpenTofu

- **Open source** — Community-driven fork of Terraform
- **Compatible** — Works with existing Terraform providers
- **No license restrictions** — BSL-free

---

## Next Steps

- **[OpenTofu](/tooling/opentofu)** — Cloud deployment guides (GCP, AWS, K8s)
- **[Observability](/operations/observability)** — Monitoring and tracing
- **[Databases](/tooling/databases/postgres)** — Database configuration details
