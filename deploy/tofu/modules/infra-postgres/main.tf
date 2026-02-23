# Infrastructure Module: PostgreSQL
# Deploys PostgreSQL via Helm and outputs connection info

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

resource "random_password" "postgres" {
  count   = var.password == "" ? 1 : 0
  length  = 24
  special = false
}

resource "random_password" "postgres_admin" {
  count   = var.admin_password == "" ? 1 : 0
  length  = 24
  special = false
}

locals {
  password       = var.password != "" ? var.password : random_password.postgres[0].result
  admin_password = var.admin_password != "" ? var.admin_password : random_password.postgres_admin[0].result
  service_name   = "${var.name}-postgresql"
}

resource "helm_release" "postgresql" {
  name      = var.name
  namespace = var.namespace
  chart     = "${path.module}/../../../helm/angzarr-db-postgres"

  values = [yamlencode({
    postgresql = {
      auth = {
        postgresPassword = local.admin_password
        username         = var.username
        password         = local.password
        database         = var.database
      }
      primary = {
        persistence = {
          enabled      = var.persistence_enabled
          size         = var.persistence_size
          storageClass = var.storage_class
        }
        resources = var.resources
      }
    }
  })]
}
