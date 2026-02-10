# Infrastructure Charts

Angzarr uses modular Helm charts for infrastructure. Each database and message bus is deployed separately from the core application.

## Available Charts

| Chart | Description |
|-------|-------------|
| `angzarr-db-mongodb` | MongoDB event store (wraps Bitnami) |
| `angzarr-db-postgres` | PostgreSQL for projector read models (wraps Bitnami) |
| `angzarr-mq-rabbitmq` | RabbitMQ message bus (wraps Bitnami) |
| `angzarr-mq-kafka` | Kafka message bus (wraps Bitnami) |
| `eventstore` | EventStoreDB (custom chart) |

## Deployment Order

1. Deploy infrastructure charts first
2. Deploy core angzarr chart with applications

```bash
# Create namespace
kubectl create namespace angzarr

# Deploy database
helm install angzarr-db ./deploy/helm/angzarr-db-mongodb -n angzarr \
  --set mongodb.auth.rootPassword=<root-password> \
  --set mongodb.auth.passwords[0]=<app-password>

# Deploy message bus
helm install angzarr-mq ./deploy/helm/angzarr-mq-rabbitmq -n angzarr \
  --set rabbitmq.auth.password=<password>

# Deploy application
helm install angzarr ./deploy/helm/angzarr -n angzarr \
  -f ./deploy/helm/angzarr/values-rust.yaml
```

## Service Naming

Infrastructure charts create services with predictable names:

| Chart Release | Service Name |
|---------------|--------------|
| `angzarr-db` (mongodb) | `angzarr-db-mongodb` |
| `angzarr-db-pg` (postgres) | `angzarr-db-pg-postgresql` |
| `angzarr-mq` (rabbitmq) | `angzarr-mq-rabbitmq` |
| `angzarr-mq` (kafka) | `angzarr-mq-kafka` |

The core angzarr chart's connection strings reference these service names.

## MongoDB Configuration

### values.yaml

```yaml
mongodb:
  auth:
    rootPassword: ""  # Required
    usernames: ["angzarr"]
    passwords: [""]   # Required
    databases: ["angzarr"]
  persistence:
    size: 8Gi
```

### Connection String

```yaml
# In angzarr values
storage:
  type: mongodb
  mongodb:
    uri: "mongodb://angzarr:<password>@angzarr-db-mongodb:27017/angzarr?authSource=angzarr"
```

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
    uri: "postgres://angzarr:<password>@angzarr-db-pg-postgresql:5432/angzarr"
```

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
  listeners:
    client:
      protocol: PLAINTEXT
```

### Connection String

```yaml
# In angzarr values
messaging:
  type: kafka
  kafka:
    bootstrapServers: "angzarr-mq-kafka:9092"
```

## Skaffold Deployment

For local development, skaffold deploys infrastructure automatically:

```yaml
# examples/rust/skaffold.yaml
deploy:
  helm:
    releases:
      # Infrastructure first
      - name: angzarr-db
        chartPath: ../../deploy/helm/angzarr-db-mongodb
        namespace: angzarr
        setValues:
          mongodb.auth.rootPassword: dev-password
          mongodb.auth.passwords[0]: dev-password

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

## Why Modular Charts?

1. **Independent lifecycle**: Upgrade databases without redeploying apps
2. **Flexibility**: Choose only the infrastructure you need
3. **Production parity**: Same charts for dev and prod
4. **Clear dependencies**: Explicit deployment order

## Migrating from Embedded Dependencies

If upgrading from an earlier version where infrastructure was embedded:

1. Delete the old release: `helm uninstall angzarr -n angzarr`
2. Deploy infrastructure charts
3. Deploy new angzarr chart

Data in PVCs is preserved if you use the same PVC names.
