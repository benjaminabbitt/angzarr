# Database module - PostgreSQL or MongoDB via Helm
# Supports both local (Helm charts) and cloud-managed databases

terraform {
  required_providers {
    helm = {
      source  = "hashicorp/helm"
      version = "~> 2.0"
    }
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.0"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.0"
    }
  }
}

# Auto-generate admin password if not provided
resource "random_password" "admin" {
  count   = var.admin_password == null ? 1 : 0
  length  = 32
  special = false
}

# Auto-generate user password if not provided
resource "random_password" "user" {
  count   = var.password == null ? 1 : 0
  length  = 32
  special = false
}

locals {
  admin_password = var.admin_password != null ? var.admin_password : random_password.admin[0].result
  user_password  = var.password != null ? var.password : random_password.user[0].result
}

# PostgreSQL via Bitnami Helm chart (OCI registry)
resource "helm_release" "postgresql" {
  count = var.type == "postgresql" && var.managed == false ? 1 : 0

  name       = var.release_name
  repository = "oci://registry-1.docker.io/bitnamicharts"
  chart      = "postgresql"
  version    = var.postgresql_chart_version
  namespace  = var.namespace

  values = [
    yamlencode({
      # Use AWS ECR registry for images (more stable tags than Docker Hub)
      global = {
        security = {
          allowInsecureImages = true
        }
      }
      image = {
        registry   = "public.ecr.aws"
        repository = "bitnami/postgresql"
        tag        = "18"
      }
      auth = {
        postgresPassword = local.admin_password
        username         = var.username
        password         = local.user_password
        database         = var.database
      }
      primary = {
        persistence = {
          enabled = var.persistence_enabled
          size    = var.persistence_size
        }
        resources = {
          requests = {
            memory = var.resources.requests.memory
            cpu    = var.resources.requests.cpu
          }
          limits = {
            memory = var.resources.limits.memory
            cpu    = var.resources.limits.cpu
          }
        }
      }
      metrics = {
        enabled = var.metrics_enabled
      }
    })
  ]

  wait = true
}

# MongoDB via Bitnami Helm chart (OCI registry)
resource "helm_release" "mongodb" {
  count = var.type == "mongodb" && var.managed == false ? 1 : 0

  name       = var.release_name
  repository = "oci://registry-1.docker.io/bitnamicharts"
  chart      = "mongodb"
  version    = var.mongodb_chart_version
  namespace  = var.namespace

  values = [
    yamlencode({
      # Use AWS ECR registry for images (more stable tags than Docker Hub)
      global = {
        security = {
          allowInsecureImages = true
        }
      }
      image = {
        registry   = "public.ecr.aws"
        repository = "bitnami/mongodb"
        tag        = "8.0"
      }
      auth = {
        enabled      = true
        rootUser     = "root"
        rootPassword = local.admin_password
        usernames    = [var.username]
        passwords    = [local.user_password]
        databases    = [var.database]
      }
      persistence = {
        enabled = var.persistence_enabled
        size    = var.persistence_size
      }
      resources = {
        requests = {
          memory = var.resources.requests.memory
          cpu    = var.resources.requests.cpu
        }
        limits = {
          memory = var.resources.limits.memory
          cpu    = var.resources.limits.cpu
        }
      }
      metrics = {
        enabled = var.metrics_enabled
      }
    })
  ]

  wait = true
}

# Kubernetes secret for database credentials
resource "kubernetes_secret" "database_credentials" {
  metadata {
    name      = "${var.release_name}-credentials"
    namespace = var.namespace
  }

  data = {
    username       = var.username
    password       = local.user_password
    admin_password = local.admin_password
    database       = var.database
    host           = local.host
    port           = tostring(local.port)
    uri            = local.uri
  }
}

locals {
  # Bitnami PostgreSQL and MongoDB charts use the release name directly for service name
  # (unlike RabbitMQ which appends -rabbitmq)
  host = var.type == "postgresql" ? "${var.release_name}.${var.namespace}.svc.cluster.local" : (
    var.type == "mongodb" ? "${var.release_name}.${var.namespace}.svc.cluster.local" : var.external_host
  )

  port = var.type == "postgresql" ? 5432 : (
    var.type == "mongodb" ? 27017 : var.external_port
  )

  uri = var.type == "postgresql" ? "postgres://${var.username}:${local.user_password}@${local.host}:${local.port}/${var.database}" : (
    var.type == "mongodb" ? "mongodb://${var.username}:${local.user_password}@${local.host}:${local.port}/${var.database}" : var.external_uri
  )
}
