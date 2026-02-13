# OpenTofu Infrastructure

Infrastructure as Code for angzarr backing services (databases, messaging, service mesh).
Uses [OpenTofu](https://opentofu.org/) (open-source Terraform fork).

## Structure

```
deploy/tofu/
├── modules/                    # Reusable infrastructure modules
│   ├── database/              # PostgreSQL or MongoDB via Helm (K8s)
│   ├── messaging/             # RabbitMQ or Kafka via Helm (K8s)
│   ├── mesh/                  # Linkerd or Istio via Helm (K8s)
│   ├── cloudsql/              # Cloud SQL PostgreSQL (GCP)
│   ├── pubsub/                # Pub/Sub event bus (GCP)
│   ├── domain/                # Domain module - aggregate/PM + sagas + projectors (GCP)
│   ├── infrastructure/        # Stream + Topology services (GCP)
│   └── registry/              # Service discovery aggregation (GCP)
└── environments/              # Environment-specific configurations
    ├── local/                 # Local development (K8s)
    ├── gcp/                   # Google Cloud Run deployment
    ├── staging/               # Staging environment
    └── prod/                  # Production environment
```

## Secrets Management

**Passwords are managed via Kubernetes secrets, not tfvars files.**

### Flow

1. `just secrets-init` generates random passwords and stores them in K8s secret
2. OpenTofu reads from the K8s secret (no passwords in tfvars)
3. OpenTofu modules deploy infrastructure using those passwords

### Benefits

- Single source of truth for credentials (K8s secrets)
- Works with External Secrets Operator (ESO) for production
- No sensitive values in version control or tfvars files
- Credentials can be rotated via `just secrets-rotate`

### Production with ESO

For production, use External Secrets Operator to sync from:
- AWS Secrets Manager
- HashiCorp Vault
- GCP Secret Manager
- Azure Key Vault

ESO syncs external secrets → K8s secret → OpenTofu reads it.

## Quick Start (Local Development - K8s)

```bash
# Deploy infrastructure (generates secrets + runs opentofu)
just infra-local

# Or step by step:
just secrets-init            # Generate and store credentials in K8s
just tofu init local    # Initialize OpenTofu
just tofu apply local   # Apply (will prompt for confirmation)
```

## Quick Start (Google Cloud Run)

Deploy angzarr to Google Cloud Run with serverless scaling.

```bash
cd deploy/tofu/environments/gcp

# Configure variables
cp terraform.tfvars.example terraform.tfvars
# Edit terraform.tfvars with your project settings

# Initialize and deploy
tofu init
tofu plan
tofu apply
```

### GCP Prerequisites

1. GCP project with billing enabled
2. APIs enabled:
   - Cloud Run API
   - Cloud SQL Admin API
   - Pub/Sub API
   - Secret Manager API
3. Container images pushed to GCR or Artifact Registry
4. Service account with appropriate permissions

### GCP Module Reference

#### Domain Module

Deploys all components for a single domain (aggregate/PM + sagas + projectors).

```hcl
module "order" {
  source = "../../modules/domain"

  domain     = "order"
  project_id = var.project_id
  region     = var.region

  aggregate = {
    enabled = true
    env     = { ORDER_FEATURE = "enabled" }
    upcaster = {
      enabled = true  # Optional upcaster sidecar
    }
  }

  sagas = {
    fulfillment = {
      target_domain = "fulfillment"
      env = { SAGA_TIMEOUT_MS = "5000" }
    }
  }

  projectors = {
    web = { env = {} }
  }

  images = {
    grpc_gateway          = "gcr.io/project/grpc-gateway:latest"
    coordinator_aggregate = "gcr.io/project/angzarr-aggregate:latest"
    coordinator_saga      = "gcr.io/project/angzarr-saga:latest"
    coordinator_projector = "gcr.io/project/angzarr-projector:latest"
    coordinator_pm        = "gcr.io/project/angzarr-pm:latest"
    logic                 = "gcr.io/project/agg-order:latest"
    upcaster              = "gcr.io/project/upcaster-order:latest"
    saga_logic            = { fulfillment = "gcr.io/project/saga-order-fulfillment:latest" }
    projector_logic       = { web = "gcr.io/project/projector-order-web:latest" }
  }

  discovery_env   = module.registry.discovery_env
  coordinator_env = merge(module.cloudsql.coordinator_env, module.pubsub.coordinator_env)
}
```

#### Cloud Run Architecture

Each Cloud Run service contains multiple containers:

```
┌──────────────────────────────────────────────────────────────┐
│  Cloud Run Service                                           │
│                                                              │
│  ┌─────────────────┐  ┌─────────────┐  ┌─────────────────┐  │
│  │  grpc-gateway   │  │ Coordinator │  │  Business Logic │  │
│  │  (REST bridge)  │  │ (angzarr)   │  │  (user code)    │  │
│  │  Port: 8080     │  │ Port: 1310  │  │  Port: 50053    │  │
│  └────────┬────────┘  └──────┬──────┘  └────────┬────────┘  │
│           └────────localhost─┴──────────────────┘            │
│  Optional: Upcaster sidecar (port 50054)                     │
└──────────────────────────────────────────────────────────────┘
```

- **grpc-gateway**: REST→gRPC bridge (exposed port 8080)
- **Coordinator**: Angzarr event sourcing machinery
- **Business Logic**: Your domain code
- **Upcaster**: Optional event upcasting sidecar

## State Backend Configuration

### Local Backend (Development Only)

The local environment uses a local state file (`terraform.tfstate`). OpenTofu
uses the same state format as Terraform. This is **only appropriate for local
development** where:

- State loss is acceptable (can be recreated)
- No team collaboration required
- No audit trail needed

### Remote Backend (Staging/Production)

For staging and production, **always use a remote backend** with:

- State locking (prevents concurrent modifications)
- Encryption at rest
- Versioning (state history)
- Access controls

#### AWS S3 Backend

```hcl
# In environments/staging/main.tf or environments/prod/main.tf
terraform {
  backend "s3" {
    bucket         = "angzarr-terraform-state"
    key            = "staging/terraform.tfstate"  # or "prod/terraform.tfstate"
    region         = "us-east-1"
    encrypt        = true
    dynamodb_table = "angzarr-terraform-locks"    # For state locking
  }
}
```

Setup:
```bash
# Create S3 bucket
aws s3api create-bucket --bucket angzarr-terraform-state --region us-east-1

# Enable versioning
aws s3api put-bucket-versioning --bucket angzarr-terraform-state \
    --versioning-configuration Status=Enabled

# Create DynamoDB table for locking
aws dynamodb create-table \
    --table-name angzarr-terraform-locks \
    --attribute-definitions AttributeName=LockID,AttributeType=S \
    --key-schema AttributeName=LockID,KeyType=HASH \
    --billing-mode PAY_PER_REQUEST
```

#### GCS Backend (Google Cloud)

```hcl
terraform {
  backend "gcs" {
    bucket = "angzarr-terraform-state"
    prefix = "staging"  # or "prod"
  }
}
```

#### Azure Storage Backend

```hcl
terraform {
  backend "azurerm" {
    resource_group_name  = "angzarr-tfstate"
    storage_account_name = "angzarrtfstate"
    container_name       = "tfstate"
    key                  = "staging.terraform.tfstate"
  }
}
```

#### Terraform Cloud/Enterprise

```hcl
terraform {
  cloud {
    organization = "your-org"
    workspaces {
      name = "angzarr-staging"
    }
  }
}
```

## Environment Configuration

### Local

- Uses Helm charts for all services
- Service mesh optional (disabled by default)
- Minimal resource requests
- State stored locally

```bash
just infra-local
```

### Staging

- Uses Helm charts or cloud-managed services
- Service mesh required
- Moderate resource allocation
- Remote state backend required

```bash
# Configure backend first (edit environments/staging/main.tf)
just infra-staging
```

### Production

- Prefers cloud-managed services (RDS, MSK, etc.)
- Service mesh required
- Production resource allocation
- Remote state backend required
- Requires explicit confirmation

```bash
# Configure backend first (edit environments/prod/main.tf)
just infra-prod
```

## Module Reference

### Database Module

Deploys PostgreSQL or MongoDB.

```hcl
module "database" {
  source = "../../modules/database"

  type           = "postgresql"  # or "mongodb"
  managed        = false         # true for cloud-managed (RDS, Cloud SQL)
  release_name   = "angzarr-db"
  namespace      = "angzarr"
  admin_password = var.db_admin_password
  password       = var.db_password
  database       = "angzarr"
}
```

### Messaging Module

Deploys RabbitMQ or Kafka.

```hcl
module "messaging" {
  source = "../../modules/messaging"

  type         = "rabbitmq"  # or "kafka"
  managed      = false       # true for cloud-managed (MSK, CloudAMQP)
  release_name = "angzarr-mq"
  namespace    = "angzarr"
  password     = var.mq_password
}
```

### Mesh Module

Deploys Linkerd or Istio service mesh.

```hcl
module "mesh" {
  source = "../../modules/mesh"

  type             = "linkerd"  # or "istio"
  namespace        = "angzarr"
  inject_namespace = true

  # Linkerd requires mTLS certificates
  linkerd_trust_anchor_pem = var.linkerd_trust_anchor_pem
  linkerd_issuer_cert_pem  = var.linkerd_issuer_cert_pem
  linkerd_issuer_key_pem   = var.linkerd_issuer_key_pem
}
```

## Justfile Targets

| Target | Description |
|--------|-------------|
| `tofu init ENV` | Initialize OpenTofu for environment |
| `tofu plan ENV` | Preview changes |
| `tofu apply ENV` | Apply with confirmation |
| `tofu apply-auto ENV` | Apply without confirmation |
| `tofu destroy ENV` | Destroy with confirmation |
| `tofu output ENV` | Show outputs |
| `tofu validate ENV` | Validate configuration |
| `tofu fmt` | Format all OpenTofu files |
| `infra-local` | Deploy local infrastructure |
| `infra-local-destroy` | Destroy local infrastructure |
| `infra-staging` | Deploy staging infrastructure |
| `infra-prod` | Deploy production infrastructure |
