# Local development environment
# Uses Helm charts for all infrastructure (no cloud-managed services)
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

  # Local backend - ONLY for development
  # WARNING: Local state is not suitable for production or team environments.
  # For staging/prod, use a remote backend (S3, GCS, Azure, Terraform Cloud).
  # See deploy/terraform/README.md for configuration examples.
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
# This is the source of truth for all passwords
data "kubernetes_secret" "angzarr_secrets" {
  metadata {
    name      = "angzarr-secrets"
    namespace = var.secrets_namespace
  }
}

locals {
  # Read passwords from K8s secret, decode from base64
  db_admin_password = data.kubernetes_secret.angzarr_secrets.data["postgres-admin-password"]
  db_password       = data.kubernetes_secret.angzarr_secrets.data["postgres-password"]
  mq_password       = data.kubernetes_secret.angzarr_secrets.data["rabbitmq-password"]
}

# Namespace for angzarr workloads
resource "kubernetes_namespace" "angzarr" {
  metadata {
    name = var.namespace
  }
}

# MongoDB - event store for angzarr core
module "mongodb" {
  source = "../../modules/database"

  type           = "mongodb"
  managed        = false
  release_name   = "angzarr-db-mongodb"
  namespace      = kubernetes_namespace.angzarr.metadata[0].name
  admin_password = local.db_admin_password
  username       = "angzarr"
  password       = local.db_password
  database       = "angzarr"

  persistence_enabled = true
  persistence_size    = "2Gi"

  resources = {
    requests = {
      memory = "128Mi"
      cpu    = "50m"
    }
    limits = {
      memory = "512Mi"
      cpu    = "500m"
    }
  }

  metrics_enabled = false
}

# PostgreSQL - projectors read models
module "postgresql" {
  source = "../../modules/database"

  type           = "postgresql"
  managed        = false
  release_name   = "angzarr-db-postgresql"
  namespace      = kubernetes_namespace.angzarr.metadata[0].name
  admin_password = local.db_admin_password
  username       = "angzarr"
  password       = local.db_password
  database       = "angzarr"

  persistence_enabled = true
  persistence_size    = "2Gi"

  resources = {
    requests = {
      memory = "128Mi"
      cpu    = "50m"
    }
    limits = {
      memory = "256Mi"
      cpu    = "250m"
    }
  }

  metrics_enabled = false
}

# Messaging - RabbitMQ for local dev
module "messaging" {
  source = "../../modules/messaging"

  type         = "rabbitmq"
  managed      = false
  release_name = "angzarr-mq"
  namespace    = kubernetes_namespace.angzarr.metadata[0].name
  username     = "angzarr"
  password     = local.mq_password

  persistence_enabled = true
  persistence_size    = "1Gi"

  resources = {
    requests = {
      memory = "128Mi"
      cpu    = "50m"
    }
    limits = {
      memory = "256Mi"
      cpu    = "250m"
    }
  }

  metrics_enabled = false
}

# Service Mesh - Linkerd for local (lightweight, optional)
module "mesh" {
  count  = var.enable_mesh ? 1 : 0
  source = "../../modules/mesh"

  type             = "linkerd"
  namespace        = kubernetes_namespace.angzarr.metadata[0].name
  inject_namespace = true

  linkerd_trust_anchor_pem = var.linkerd_trust_anchor_pem
  linkerd_issuer_cert_pem  = var.linkerd_issuer_cert_pem
  linkerd_issuer_key_pem   = var.linkerd_issuer_key_pem

  proxy_resources = {
    requests = {
      memory = "32Mi"
      cpu    = "10m"
    }
    limits = {
      memory = "128Mi"
      cpu    = "500m"
    }
  }

  control_plane_resources = {
    requests = {
      memory = "128Mi"
      cpu    = "50m"
    }
    limits = {
      memory = "512Mi"
      cpu    = "500m"
    }
  }
}
