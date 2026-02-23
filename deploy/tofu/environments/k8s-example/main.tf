# Example K8s Environment
# Deploys angzarr stack on any Kubernetes cluster (Kind, GKE, EKS, AKS, etc.)
#
# Usage:
#   cd deploy/tofu/environments/k8s-example
#   tofu init
#   tofu apply

terraform {
  required_version = ">= 1.0"

  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = ">= 2.0"
    }
    helm = {
      source  = "hashicorp/helm"
      version = ">= 2.0"
    }
    random = {
      source  = "hashicorp/random"
      version = ">= 3.0"
    }
  }
}

#------------------------------------------------------------------------------
# Kubernetes Provider Configuration
# Configure based on your cluster type
#------------------------------------------------------------------------------

# For Kind (local development):
# provider "kubernetes" {
#   config_path = "~/.kube/config"
#   config_context = "kind-angzarr"
# }

# For GKE:
# provider "kubernetes" {
#   host                   = google_container_cluster.primary.endpoint
#   cluster_ca_certificate = base64decode(google_container_cluster.primary.master_auth[0].cluster_ca_certificate)
#   token                  = data.google_client_config.default.access_token
# }

# Default: use current kubectl context
provider "kubernetes" {
  config_path    = var.kubeconfig_path
  config_context = var.kubeconfig_context
}

provider "helm" {
  kubernetes {
    config_path    = var.kubeconfig_path
    config_context = var.kubeconfig_context
  }
}

#------------------------------------------------------------------------------
# Namespace
#------------------------------------------------------------------------------

resource "kubernetes_namespace" "angzarr" {
  metadata {
    name = var.namespace
    labels = {
      "app.kubernetes.io/managed-by" = "terraform"
      "app.kubernetes.io/part-of"    = "angzarr"
    }
  }
}

#------------------------------------------------------------------------------
# Infrastructure
#------------------------------------------------------------------------------

# PostgreSQL for event store, position store, and optionally snapshot store
module "postgres" {
  source = "../../modules/infra-postgres"

  name      = "angzarr-db"
  namespace = kubernetes_namespace.angzarr.metadata[0].name
}

# RabbitMQ for event bus
module "rabbit" {
  source = "../../modules/infra-rabbit"

  name      = "angzarr-mq"
  namespace = kubernetes_namespace.angzarr.metadata[0].name
}

# Optional: Redis for snapshot store (faster reads)
module "redis" {
  count  = var.use_redis_snapshots ? 1 : 0
  source = "../../modules/infra-redis"

  name      = "angzarr-cache"
  namespace = kubernetes_namespace.angzarr.metadata[0].name
}

#------------------------------------------------------------------------------
# Application Stack
#------------------------------------------------------------------------------

module "stack" {
  source = "../../modules/stack"

  name = var.stack_name

  # Compute configuration
  compute = {
    compute_type = "kubernetes"
    namespace    = kubernetes_namespace.angzarr.metadata[0].name
  }

  # Bus configuration from RabbitMQ module
  bus = module.rabbit.bus

  # Storage configuration from PostgreSQL module
  # Optionally use Redis for snapshot store
  default_storage = var.use_redis_snapshots ? {
    event_store    = module.postgres.storage.event_store
    position_store = module.postgres.storage.position_store
    snapshot_store = module.redis[0].snapshot_store
  } : module.postgres.storage

  # Coordinator images (from container registry)
  coordinator_images = {
    aggregate    = var.coordinator_images.aggregate
    saga         = var.coordinator_images.saga
    projector    = var.coordinator_images.projector
    pm           = var.coordinator_images.pm
    grpc_gateway = var.coordinator_images.grpc_gateway
  }

  # Domain configuration
  domains = var.domains

  # Process manager configuration
  process_managers = var.process_managers
}
