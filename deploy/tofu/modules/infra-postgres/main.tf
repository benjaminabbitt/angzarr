# Infrastructure Module: PostgreSQL
# Deploys PostgreSQL using official image

terraform {
  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
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

resource "kubernetes_deployment" "postgresql" {
  metadata {
    name      = local.service_name
    namespace = var.namespace
    labels = {
      app = local.service_name
    }
  }

  spec {
    replicas = 1

    selector {
      match_labels = {
        app = local.service_name
      }
    }

    template {
      metadata {
        labels = {
          app = local.service_name
        }
      }

      spec {
        container {
          name  = "postgresql"
          image = var.image

          port {
            container_port = 5432
            name           = "postgresql"
          }

          env {
            name  = "POSTGRES_USER"
            value = var.username
          }

          env {
            name  = "POSTGRES_PASSWORD"
            value = local.password
          }

          env {
            name  = "POSTGRES_DB"
            value = var.database
          }

          resources {
            limits = {
              cpu    = var.resources.limits.cpu
              memory = var.resources.limits.memory
            }
            requests = {
              cpu    = var.resources.requests.cpu
              memory = var.resources.requests.memory
            }
          }

          liveness_probe {
            exec {
              command = ["pg_isready", "-U", var.username, "-d", var.database]
            }
            initial_delay_seconds = 30
            period_seconds        = 10
            timeout_seconds       = 5
          }

          readiness_probe {
            exec {
              command = ["pg_isready", "-U", var.username, "-d", var.database]
            }
            initial_delay_seconds = 5
            period_seconds        = 5
            timeout_seconds       = 5
          }
        }
      }
    }
  }
}

resource "kubernetes_service" "postgresql" {
  metadata {
    name      = local.service_name
    namespace = var.namespace
  }

  spec {
    selector = {
      app = local.service_name
    }

    port {
      name        = "postgresql"
      port        = 5432
      target_port = 5432
    }
  }
}
