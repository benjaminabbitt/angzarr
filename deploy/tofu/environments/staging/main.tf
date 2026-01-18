# Staging environment
# Uses Helm charts with production-like settings
# Reads credentials from K8s secrets (created by `just secrets-init`)

terraform {
  required_version = ">= 1.0"

  required_providers {
    helm = {
      source  = "hashicorp/helm"
      version = "~> 2.0"
    }
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.0"
    }
  }

  # Remote backend REQUIRED for staging
  # Uncomment and configure one of the following backends:
  #
  # AWS S3:
  # backend "s3" {
  #   bucket         = "angzarr-terraform-state"
  #   key            = "staging/terraform.tfstate"
  #   region         = "us-east-1"
  #   encrypt        = true
  #   dynamodb_table = "angzarr-terraform-locks"
  # }
  #
  # GCS:
  # backend "gcs" {
  #   bucket = "angzarr-terraform-state"
  #   prefix = "staging"
  # }
  #
  # Terraform Cloud:
  # cloud {
  #   organization = "your-org"
  #   workspaces { name = "angzarr-staging" }
  # }
  #
  # For local testing only (NOT recommended):
  backend "local" {
    path = "terraform.tfstate"
  }
}

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

# Read credentials from K8s secret (created by `just secrets-init`)
# For cloud-managed services, override with tfvars or use ESO to sync external secrets
data "kubernetes_secret" "angzarr_secrets" {
  count = var.use_k8s_secrets ? 1 : 0

  metadata {
    name      = "angzarr-secrets"
    namespace = var.secrets_namespace
  }
}

locals {
  # Use K8s secrets if available, otherwise fall back to tfvars
  db_admin_password = var.use_k8s_secrets ? data.kubernetes_secret.angzarr_secrets[0].data["postgres-admin-password"] : var.db_admin_password
  db_password       = var.use_k8s_secrets ? data.kubernetes_secret.angzarr_secrets[0].data["postgres-password"] : var.db_password
  mq_password       = var.use_k8s_secrets ? data.kubernetes_secret.angzarr_secrets[0].data["rabbitmq-password"] : var.mq_password
}

# Namespace
resource "kubernetes_namespace" "angzarr" {
  metadata {
    name = var.namespace

    labels = {
      environment = "staging"
    }
  }
}

# Database - PostgreSQL
module "database" {
  source = "../../modules/database"

  type           = var.database_type
  managed        = var.database_managed
  release_name   = "angzarr-db"
  namespace      = kubernetes_namespace.angzarr.metadata[0].name
  admin_password = local.db_admin_password
  username       = "angzarr"
  password       = local.db_password
  database       = "angzarr"

  # External DB for managed
  external_host = var.db_external_host
  external_port = var.db_external_port
  external_uri  = var.db_external_uri

  persistence_enabled = true
  persistence_size    = "10Gi"

  resources = {
    requests = {
      memory = "256Mi"
      cpu    = "100m"
    }
    limits = {
      memory = "1Gi"
      cpu    = "500m"
    }
  }

  metrics_enabled = true
}

# Messaging - RabbitMQ
module "messaging" {
  source = "../../modules/messaging"

  type         = var.messaging_type
  managed      = var.messaging_managed
  release_name = "angzarr-mq"
  namespace    = kubernetes_namespace.angzarr.metadata[0].name
  username     = "angzarr"
  password     = local.mq_password

  # External broker for managed
  external_host = var.mq_external_host
  external_port = var.mq_external_port
  external_uri  = var.mq_external_uri

  persistence_enabled = true
  persistence_size    = "5Gi"

  resources = {
    requests = {
      memory = "256Mi"
      cpu    = "100m"
    }
    limits = {
      memory = "1Gi"
      cpu    = "500m"
    }
  }

  metrics_enabled = true
}

# Service Mesh - Required for staging
module "mesh" {
  source = "../../modules/mesh"

  type             = var.mesh_type
  namespace        = kubernetes_namespace.angzarr.metadata[0].name
  inject_namespace = true

  linkerd_trust_anchor_pem = var.linkerd_trust_anchor_pem
  linkerd_issuer_cert_pem  = var.linkerd_issuer_cert_pem
  linkerd_issuer_key_pem   = var.linkerd_issuer_key_pem

  proxy_resources = {
    requests = {
      memory = "64Mi"
      cpu    = "10m"
    }
    limits = {
      memory = "256Mi"
      cpu    = "1000m"
    }
  }

  control_plane_resources = {
    requests = {
      memory = "256Mi"
      cpu    = "100m"
    }
    limits = {
      memory = "1Gi"
      cpu    = "1000m"
    }
  }
}
