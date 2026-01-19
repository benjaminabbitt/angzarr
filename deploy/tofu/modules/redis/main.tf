# Redis module - Cache/session storage via Helm
# Supports both local (Helm charts) and cloud-managed Redis

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

# Auto-generate password if not provided
resource "random_password" "redis" {
  count   = var.password == null ? 1 : 0
  length  = 32
  special = false
}

locals {
  password = var.password != null ? var.password : random_password.redis[0].result
}

# Redis via Bitnami Helm chart (OCI registry)
resource "helm_release" "redis" {
  count = var.managed == false ? 1 : 0

  name       = var.release_name
  repository = "oci://registry-1.docker.io/bitnamicharts"
  chart      = "redis"
  version    = var.chart_version
  namespace  = var.namespace

  values = [
    yamlencode({
      global = {
        security = {
          allowInsecureImages = true
        }
      }
      image = {
        registry   = "public.ecr.aws"
        repository = "bitnami/redis"
        tag        = "7.4"
      }
      auth = {
        enabled  = var.auth_enabled
        password = local.password
      }
      master = {
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
      replica = {
        replicaCount = var.replica_count
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

# Kubernetes secret for Redis credentials
resource "kubernetes_secret" "redis_credentials" {
  metadata {
    name      = "${var.release_name}-credentials"
    namespace = var.namespace
  }

  data = {
    password = local.password
    host     = local.host
    port     = tostring(local.port)
    uri      = local.uri
  }
}

locals {
  # Bitnami Redis chart uses release-name-master for the master service
  host = var.managed ? var.external_host : "${var.release_name}-master.${var.namespace}.svc.cluster.local"
  port = var.managed ? var.external_port : 6379

  # Redis URI format
  uri = var.managed ? var.external_uri : (
    var.auth_enabled
    ? "redis://:${local.password}@${local.host}:${local.port}"
    : "redis://${local.host}:${local.port}"
  )
}
