# Infrastructure Module: Redis
# Deploys Redis via Helm and outputs connection info

terraform {
  required_providers {
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

resource "random_password" "redis" {
  count   = var.password == "" ? 1 : 0
  length  = 24
  special = false
}

locals {
  password     = var.password != "" ? var.password : random_password.redis[0].result
  service_name = "${var.name}-redis-master"
}

resource "helm_release" "redis" {
  name       = var.name
  namespace  = var.namespace
  repository = "https://charts.bitnami.com/bitnami"
  chart      = "redis"
  version    = var.chart_version

  values = [yamlencode({
    auth = {
      enabled  = var.auth_enabled
      password = local.password
    }
    master = {
      persistence = {
        enabled      = var.persistence_enabled
        size         = var.persistence_size
        storageClass = var.storage_class
      }
      resources = var.resources
    }
    replica = {
      replicaCount = var.replica_count
      persistence = {
        enabled      = var.persistence_enabled
        size         = var.persistence_size
        storageClass = var.storage_class
      }
      resources = var.resources
    }
  })]
}
