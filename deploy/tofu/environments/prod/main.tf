# Production environment
# Uses cloud-managed services where available
# Reads credentials from K8s secrets or External Secrets Operator

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

  # Remote backend REQUIRED for production
  # Uncomment and configure one of the following backends:
  #
  # AWS S3 (recommended):
  # backend "s3" {
  #   bucket         = "angzarr-terraform-state"
  #   key            = "prod/terraform.tfstate"
  #   region         = "us-east-1"
  #   encrypt        = true
  #   dynamodb_table = "angzarr-terraform-locks"
  # }
  #
  # GCS:
  # backend "gcs" {
  #   bucket = "angzarr-terraform-state"
  #   prefix = "prod"
  # }
  #
  # Terraform Cloud:
  # cloud {
  #   organization = "your-org"
  #   workspaces { name = "angzarr-prod" }
  # }
  #
  # TEMPORARY: Local backend for initial setup only
  # WARNING: Replace with remote backend before production use!
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

# Read credentials from K8s secret
# In production, these are typically synced via External Secrets Operator from:
# - AWS Secrets Manager
# - HashiCorp Vault
# - GCP Secret Manager
# - Azure Key Vault
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
      environment = "production"
    }
  }
}

# Database - Cloud-managed or Helm
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

  # External DB for managed (RDS, Cloud SQL, etc.)
  external_host = var.db_external_host
  external_port = var.db_external_port
  external_uri  = var.db_external_uri

  persistence_enabled = true
  persistence_size    = "50Gi"

  resources = {
    requests = {
      memory = "1Gi"
      cpu    = "500m"
    }
    limits = {
      memory = "4Gi"
      cpu    = "2000m"
    }
  }

  metrics_enabled = true
}

# Messaging - Cloud-managed or Helm
module "messaging" {
  source = "../../modules/messaging"

  type         = var.messaging_type
  managed      = var.messaging_managed
  release_name = "angzarr-mq"
  namespace    = kubernetes_namespace.angzarr.metadata[0].name
  username     = "angzarr"
  password     = local.mq_password

  kafka_sasl_enabled = var.messaging_type == "kafka"

  # External broker for managed (MSK, CloudAMQP, etc.)
  external_host = var.mq_external_host
  external_port = var.mq_external_port
  external_uri  = var.mq_external_uri

  persistence_enabled = true
  persistence_size    = "20Gi"

  resources = {
    requests = {
      memory = "512Mi"
      cpu    = "250m"
    }
    limits = {
      memory = "2Gi"
      cpu    = "1000m"
    }
  }

  metrics_enabled = true
}

# Service Mesh - Required for production
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
      memory = "128Mi"
      cpu    = "100m"
    }
    limits = {
      memory = "512Mi"
      cpu    = "1000m"
    }
  }

  control_plane_resources = {
    requests = {
      memory = "512Mi"
      cpu    = "250m"
    }
    limits = {
      memory = "2Gi"
      cpu    = "2000m"
    }
  }
}
