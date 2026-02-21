---
sidebar_position: 7
---

# OpenTofu

OpenTofu is an open-source infrastructure as code tool (a Terraform fork) used for provisioning angzarr deployments across cloud providers.

---

## Why OpenTofu

- **Open source** — Community-driven, BSL-free
- **Terraform compatible** — Works with existing providers and modules
- **Declarative** — Infrastructure defined as code, version controlled
- **Multi-cloud** — Single workflow for AWS, GCP, Azure, and Kubernetes

---

## Installation

```bash
# macOS
brew install opentofu

# Linux (Debian/Ubuntu)
curl -fsSL https://get.opentofu.org/install-opentofu.sh | sudo bash -s -- --install-method deb

# Linux (Fedora/RHEL)
curl -fsSL https://get.opentofu.org/install-opentofu.sh | sudo bash -s -- --install-method rpm

# Verify
tofu --version
```

---

## Project Structure

```
deploy/tofu/
├── modules/                    # Reusable infrastructure modules
│   ├── database/              # PostgreSQL/MongoDB via Helm (K8s)
│   ├── messaging/             # RabbitMQ/Kafka via Helm (K8s)
│   ├── mesh/                  # Linkerd/Istio via Helm (K8s)
│   ├── redis/                 # Redis via Helm (K8s)
│   ├── cloudsql/              # Cloud SQL PostgreSQL (GCP)
│   ├── pubsub/                # Pub/Sub event bus (GCP)
│   ├── domain/                # Cloud Run domain deployment (GCP)
│   ├── infrastructure/        # Stream + Topology services (GCP)
│   ├── registry/              # Service discovery (GCP)
│   ├── gke-domain/            # GKE domain deployment (GCP K8s)
│   ├── eks-domain/            # EKS domain deployment (AWS K8s)
│   ├── fargate-base/          # Fargate VPC, cluster, ALB (AWS)
│   ├── fargate-domain/        # Fargate domain deployment (AWS)
│   ├── fargate-ecr/           # ECR repositories (AWS)
│   ├── fargate-registry/      # Service discovery (AWS)
│   └── fargate-infrastructure/# Stream + Topology (AWS Fargate)
├── environments/              # Environment-specific configurations
│   ├── local/                 # Local Kind cluster
│   ├── gcp/                   # Google Cloud Run
│   ├── aws-staging/           # AWS Fargate
│   ├── staging/               # Generic K8s staging
│   └── prod/                  # Generic K8s production
└── local/                     # Legacy local environment
```

---

## Deployment Targets

Angzarr supports five deployment modes:

| Mode | Target | Storage | Messaging | Best For |
|------|--------|---------|-----------|----------|
| **Standalone** | Single process | SQLite | Channel bus | Development, testing |
| **Local K8s** | Kind cluster | PostgreSQL | RabbitMQ | Local integration testing |
| **GCP Cloud Run** | Serverless | Cloud SQL | Pub/Sub | Serverless production |
| **GCP GKE** | Kubernetes | Cloud SQL or PostgreSQL | Pub/Sub or Kafka | K8s production on GCP |
| **AWS Fargate** | Serverless containers | RDS | SNS/SQS | Serverless production on AWS |
| **AWS EKS** | Kubernetes | RDS or PostgreSQL | MSK or RabbitMQ | K8s production on AWS |

---

## Standalone Mode

Standalone mode runs angzarr as a single process without containers or orchestration. Ideal for development and testing.

### Configuration

```bash
# Build with standalone features
cargo build --features standalone

# Run
cargo run --features standalone --bin angzarr_standalone
```

### Characteristics

| Component | Implementation |
|-----------|----------------|
| Storage | SQLite (file-based) |
| Event Bus | Channel bus (in-process) |
| Transport | Unix domain sockets |
| Scaling | Single instance |

### Environment Variables

```bash
# Storage
ANGZARR_STORAGE_TYPE=sqlite
ANGZARR_SQLITE_PATH=./data/events.db

# Logging
ANGZARR_LOG=angzarr=debug
```

### When to Use

- Local development without containers
- Unit and integration testing
- CI pipelines
- Prototyping

---

## Local Kubernetes (Kind)

Development cluster using Kind with local registry.

### Quick Start

```bash
# Full deployment
just deploy

# Step by step
just cluster-create      # Create Kind cluster + registry
just infra              # Deploy PostgreSQL + RabbitMQ
just framework-build    # Build coordinator images
skaffold run            # Deploy application
```

### OpenTofu Commands

```bash
cd deploy/tofu/environments/local

# Initialize
tofu init

# Deploy infrastructure
tofu apply
```

### Architecture

```mermaid
flowchart TB
    subgraph Kind[Kind Cluster]
        PG[(PostgreSQL<br/>Helm chart)]
        RMQ[(RabbitMQ<br/>Helm chart)]
        subgraph NS[angzarr Namespace]
            Order[Order Agg]
            Inventory[Inventory Agg]
            Saga[Saga]
        end
        PG & RMQ --- NS
    end
    Registry[(GHCR<br/>ghcr.io/angzarr-io)]
    Kind --- Registry
```

---

## GCP Cloud Run

Serverless deployment using Cloud Run multi-container services.

### Prerequisites

1. GCP project with billing enabled
2. APIs enabled:
   - Cloud Run API
   - Cloud SQL Admin API
   - Pub/Sub API
   - Secret Manager API
3. Container images in Artifact Registry or GCR
4. Service account with appropriate permissions

### Quick Start

```bash
cd deploy/tofu/environments/gcp

# Configure
cp terraform.tfvars.example terraform.tfvars
# Edit terraform.tfvars with your settings

# Deploy
tofu init
tofu plan
tofu apply
```

### terraform.tfvars Example

```hcl
project_id    = "my-gcp-project"
region        = "us-central1"
environment   = "staging"

# Container images
image_registry = "gcr.io/my-gcp-project"
image_tag      = "v1.0.0"

# Database
database_tier = "db-f1-micro"
database_ha   = false

# Access
allow_unauthenticated = true  # false for production
```

### Architecture

Each Cloud Run service contains multiple containers communicating via localhost:

```mermaid
flowchart TB
    subgraph CR[Cloud Run Service: order-aggregate]
        GW[grpc-gateway<br/>REST bridge<br/>Port 8080]
        Coord[Coordinator<br/>angzarr<br/>Port 1310]
        Logic[Business Logic<br/>user code<br/>Port 50053]
        Up[Upcaster sidecar<br/>Port 50054]
        GW <-->|localhost| Coord <-->|localhost| Logic
    end
    SQL[(Cloud SQL<br/>PostgreSQL)]
    Proxy[Cloud SQL Auth Proxy]
    PubSub[Pub/Sub<br/>Event Bus]

    CR -->|HTTPS| Proxy --> SQL
    CR -->|gRPC| PubSub
```

### Module Reference

#### Domain Module

Deploys all components for a single domain:

```hcl
module "order" {
  source = "../../modules/domain"

  domain     = "order"
  project_id = var.project_id
  region     = var.region

  aggregate = {
    enabled = true
    env     = { ORDER_FEATURE = "enabled" }
    upcaster = { enabled = true }
  }

  sagas = {
    fulfillment = {
      target_domain = "fulfillment"
      env           = {}
    }
  }

  projectors = {
    web = { env = {} }
  }

  images = {
    grpc_gateway          = local.images.grpc_gateway
    coordinator_aggregate = local.images.coordinator_aggregate
    coordinator_saga      = local.images.coordinator_saga
    coordinator_projector = local.images.coordinator_projector
    logic                 = local.domain_images.order.logic
    saga_logic            = local.domain_images.order.saga_logic
    projector_logic       = local.domain_images.order.projector_logic
  }

  discovery_env   = module.registry.discovery_env
  coordinator_env = merge(module.cloudsql.coordinator_env, module.pubsub.coordinator_env)
}
```

#### Cloud SQL Module

```hcl
module "cloudsql" {
  source = "../../modules/cloudsql"

  project_id        = var.project_id
  region            = var.region
  instance_name     = "angzarr-${var.environment}"
  database_name     = "angzarr"
  tier              = "db-f1-micro"       # db-custom-2-4096 for production
  availability_type = "ZONAL"             # REGIONAL for HA
}
```

#### Pub/Sub Module

```hcl
module "pubsub" {
  source = "../../modules/pubsub"

  project_id        = var.project_id
  events_topic_name = "angzarr-events-${var.environment}"
}
```

### Scaling Configuration

```hcl
module "order" {
  # ... other config ...

  scaling = {
    aggregate = {
      min_instances = 0    # Scale to zero
      max_instances = 10
      resources = {
        cpu    = "1"
        memory = "512Mi"
      }
    }
    grpc_gateway = {
      resources = {
        cpu    = "0.5"
        memory = "256Mi"
      }
    }
  }

  execution = {
    environment     = "EXECUTION_ENVIRONMENT_GEN2"
    timeout_seconds = 300
    cpu_idle        = true  # CPU allocated only during requests
  }
}
```

---

## GCP GKE

Kubernetes deployment on Google Kubernetes Engine.

### Prerequisites

1. GKE cluster created
2. `kubectl` configured for the cluster
3. Helm installed
4. Container images in Artifact Registry

### Quick Start

```bash
# Configure kubectl
gcloud container clusters get-credentials CLUSTER_NAME --region REGION

# Deploy infrastructure
cd deploy/tofu/environments/gcp-k8s
tofu init
tofu apply
```

### Module Usage

```hcl
module "order" {
  source = "../../modules/gke-domain"

  domain    = "order"
  namespace = "angzarr"

  aggregate = {
    enabled = true
    env     = {}
  }

  images = {
    coordinator_aggregate = "gcr.io/project/angzarr-aggregate:latest"
    coordinator_saga      = "gcr.io/project/angzarr-saga:latest"
    coordinator_projector = "gcr.io/project/angzarr-projector:latest"
    logic                 = "gcr.io/project/agg-order:latest"
  }

  storage = {
    type = "postgres"
    postgres = {
      uri = "postgres://user:pass@cloudsql-proxy:5432/angzarr"
    }
  }

  messaging = {
    type = "amqp"
    amqp = {
      url = "amqp://user:pass@rabbitmq:5672"
    }
  }

  # GKE Workload Identity
  workload_identity = {
    enabled             = true
    gcp_service_account = "angzarr@project.iam.gserviceaccount.com"
  }
}
```

### Workload Identity

GKE Workload Identity allows pods to authenticate as GCP service accounts:

```hcl
# 1. Create GCP service account
resource "google_service_account" "angzarr" {
  account_id   = "angzarr"
  display_name = "Angzarr Service Account"
}

# 2. Grant permissions
resource "google_project_iam_member" "cloudsql" {
  project = var.project_id
  role    = "roles/cloudsql.client"
  member  = "serviceAccount:${google_service_account.angzarr.email}"
}

# 3. Bind to K8s service account
resource "google_service_account_iam_member" "workload_identity" {
  service_account_id = google_service_account.angzarr.name
  role               = "roles/iam.workloadIdentityUser"
  member             = "serviceAccount:${var.project_id}.svc.id.goog[angzarr/angzarr]"
}
```

---

## AWS Fargate

Serverless containers on AWS using ECS Fargate.

### Prerequisites

1. AWS account with appropriate permissions
2. VPC with private subnets
3. Container images in ECR
4. Secrets in Secrets Manager or Parameter Store

### Quick Start

```bash
cd deploy/tofu/environments/aws-staging

# Configure
cp terraform.tfvars.example terraform.tfvars
# Edit with your settings

# Deploy
tofu init
tofu plan
tofu apply
```

### terraform.tfvars Example

```hcl
aws_region  = "us-east-1"
environment = "staging"

# Images
image_tag = "v1.0.0"

# Database (RDS or external)
database_uri = "postgres://user:pass@rds-endpoint:5432/angzarr"

# Messaging (SNS/SQS or external)
messaging_uri = "amqp://user:pass@mq-endpoint:5672"

log_level = "info"
```

### Architecture

```mermaid
flowchart TB
    subgraph VPC[AWS VPC]
        subgraph Subnets[Private Subnets]
            subgraph ECS[ECS Cluster - Fargate]
                Order[Order Task]
                Inventory[Inventory Task]
                Fulfill[Fulfill Task]
            end
            ALB[Application Load Balancer]
            Order & Inventory & Fulfill --> ALB
        end
        CloudMap[Cloud Map<br/>Service Discovery]
        ALB --> CloudMap
    end
    RDS[(RDS<br/>Postgres)]
    ECR[(ECR<br/>Images)]
    Secrets[(Secrets<br/>Manager)]

    VPC --> RDS
    VPC --> ECR
    VPC --> Secrets
```

### Module Reference

#### Fargate Base Module

Creates VPC, ECS cluster, ALB, and service discovery:

```hcl
module "base" {
  source = "../../modules/fargate-base"

  name        = "angzarr"
  environment = var.environment

  vpc_cidr           = "10.0.0.0/16"
  availability_zones = ["us-east-1a", "us-east-1b"]

  create_alb               = true
  create_service_discovery = true
}
```

#### Fargate Domain Module

Deploys a domain with its components:

```hcl
module "order" {
  source = "../../modules/fargate-domain"

  domain      = "order"
  cluster_arn = module.base.cluster_arn
  vpc_id      = module.base.vpc_id
  subnet_ids  = module.base.private_subnet_ids

  security_group_ids             = [module.base.tasks_security_group_id]
  execution_role_arn             = module.base.execution_role_arn
  service_discovery_namespace_id = module.base.service_discovery_namespace_id
  lb_arn                         = module.base.lb_arn

  aggregate = {
    enabled       = true
    min_instances = 1
    max_instances = 5
    resources     = { cpu = "1", memory = "512Mi" }
  }

  sagas = {
    fulfillment = {
      target_domain = "fulfillment"
      min_instances = 1
      max_instances = 5
    }
  }

  images = {
    grpc_gateway          = "${module.ecr.images.grpc_gateway}:${var.image_tag}"
    coordinator_aggregate = "${module.ecr.images.coordinator_aggregate}:${var.image_tag}"
    logic                 = "${module.ecr.registry_url}/order-logic:${var.image_tag}"
    saga_logic = {
      fulfillment = "${module.ecr.registry_url}/saga-order-fulfillment:${var.image_tag}"
    }
  }

  discovery_env   = module.registry.discovery_env
  coordinator_env = local.coordinator_env
}
```

#### ECR Module

Creates ECR repositories for images:

```hcl
module "ecr" {
  source = "../../modules/fargate-ecr"

  name_prefix = "angzarr"
}
```

### Secrets Management

Use AWS Secrets Manager for sensitive configuration:

```hcl
resource "aws_secretsmanager_secret" "db_password" {
  name = "angzarr/database-password"
}

module "order" {
  # ... other config ...

  coordinator_secrets = {
    DATABASE_PASSWORD = {
      secret_arn = aws_secretsmanager_secret.db_password.arn
      key        = "password"
    }
  }
}
```

---

## AWS EKS

Kubernetes deployment on Amazon Elastic Kubernetes Service.

### Prerequisites

1. EKS cluster created
2. `kubectl` configured for the cluster
3. Helm installed
4. Container images in ECR
5. IRSA (IAM Roles for Service Accounts) configured

### Module Usage

```hcl
module "order" {
  source = "../../modules/eks-domain"

  domain    = "order"
  namespace = "angzarr"

  aggregate = {
    enabled = true
    env     = {}
  }

  images = {
    coordinator_aggregate = "123456789.dkr.ecr.us-east-1.amazonaws.com/angzarr-aggregate:latest"
    coordinator_saga      = "123456789.dkr.ecr.us-east-1.amazonaws.com/angzarr-saga:latest"
    coordinator_projector = "123456789.dkr.ecr.us-east-1.amazonaws.com/angzarr-projector:latest"
    logic                 = "123456789.dkr.ecr.us-east-1.amazonaws.com/agg-order:latest"
  }

  storage = {
    type = "postgres"
    postgres = {
      uri = "postgres://user:pass@rds-endpoint:5432/angzarr"
    }
  }

  messaging = {
    type = "kafka"
    kafka = {
      bootstrap_servers = "msk-endpoint:9092"
    }
  }

  # Image pull from ECR
  image_pull_secrets = ["ecr-credentials"]
}
```

### IRSA Configuration

```hcl
# 1. Create IAM role
resource "aws_iam_role" "angzarr" {
  name = "angzarr-eks-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRoleWithWebIdentity"
      Effect = "Allow"
      Principal = {
        Federated = aws_iam_openid_connect_provider.eks.arn
      }
      Condition = {
        StringEquals = {
          "${aws_iam_openid_connect_provider.eks.url}:sub" = "system:serviceaccount:angzarr:angzarr"
        }
      }
    }]
  })
}

# 2. Attach policies
resource "aws_iam_role_policy_attachment" "rds" {
  role       = aws_iam_role.angzarr.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonRDSDataFullAccess"
}

# 3. Annotate service account
module "order" {
  # ... other config ...

  service_account_annotations = {
    "eks.amazonaws.com/role-arn" = aws_iam_role.angzarr.arn
  }
}
```

---

## State Backend Configuration

### Local (Development Only)

```hcl
# No backend block - uses local terraform.tfstate
# Only for local development where state loss is acceptable
```

### AWS S3

```hcl
terraform {
  backend "s3" {
    bucket         = "angzarr-terraform-state"
    key            = "staging/terraform.tfstate"
    region         = "us-east-1"
    encrypt        = true
    dynamodb_table = "angzarr-terraform-locks"
  }
}
```

Setup:
```bash
# Create bucket
aws s3api create-bucket --bucket angzarr-terraform-state --region us-east-1

# Enable versioning
aws s3api put-bucket-versioning --bucket angzarr-terraform-state \
    --versioning-configuration Status=Enabled

# Create lock table
aws dynamodb create-table \
    --table-name angzarr-terraform-locks \
    --attribute-definitions AttributeName=LockID,AttributeType=S \
    --key-schema AttributeName=LockID,KeyType=HASH \
    --billing-mode PAY_PER_REQUEST
```

### GCS (Google Cloud Storage)

```hcl
terraform {
  backend "gcs" {
    bucket = "angzarr-terraform-state"
    prefix = "staging"
  }
}
```

### Terraform Cloud

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

---

## Justfile Commands

| Command | Description |
|---------|-------------|
| `just tofu init ENV` | Initialize OpenTofu for environment |
| `just tofu plan ENV` | Preview changes |
| `just tofu apply ENV` | Apply with confirmation |
| `just tofu apply-auto ENV` | Apply without confirmation |
| `just tofu destroy ENV` | Destroy with confirmation |
| `just tofu output ENV` | Show outputs |
| `just tofu validate ENV` | Validate configuration |
| `just tofu fmt` | Format all OpenTofu files |
| `just infra-local` | Deploy local infrastructure |
| `just infra-local-destroy` | Destroy local infrastructure |

---

## Secrets Management

### Development (K8s Secrets)

```bash
# Generate and store credentials
just secrets-init

# Rotate credentials
just secrets-rotate
```

### Production (External Secrets Operator)

Use ESO to sync from external secret stores:

```yaml
apiVersion: external-secrets.io/v1beta1
kind: ExternalSecret
metadata:
  name: angzarr-db
spec:
  secretStoreRef:
    name: aws-secrets-manager
    kind: SecretStore
  target:
    name: angzarr-db-credentials
  data:
    - secretKey: password
      remoteRef:
        key: angzarr/database
        property: password
```

Supported backends:
- AWS Secrets Manager
- GCP Secret Manager
- HashiCorp Vault
- Azure Key Vault

---

## Troubleshooting

### Common Issues

| Symptom | Cause | Fix |
|---------|-------|-----|
| "Error acquiring state lock" | Concurrent apply | Wait or force-unlock |
| "Provider not found" | Missing `tofu init` | Run `tofu init` |
| "Permission denied" | IAM/credentials | Check service account permissions |
| Cloud Run cold starts | Min instances = 0 | Set min_instances > 0 |
| Fargate task fails | Missing secrets | Check Secrets Manager access |

### Debug Commands

```bash
# Validate configuration
tofu validate

# Show plan in detail
tofu plan -out=plan.tfplan
tofu show plan.tfplan

# List state
tofu state list

# Show specific resource
tofu state show module.order.google_cloud_run_v2_service.aggregate[0]

# Import existing resource
tofu import module.order.google_cloud_run_v2_service.aggregate[0] projects/PROJECT/locations/REGION/services/order-aggregate
```

---

## Next Steps

- **[Infrastructure](/operations/infrastructure)** — Helm charts and Skaffold
- **[Observability](/operations/observability)** — Monitoring and tracing
- **[Testing](/operations/testing)** — Three-level testing strategy
