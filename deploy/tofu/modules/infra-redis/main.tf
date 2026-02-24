# Infrastructure Module: Redis
# Deploys Redis using official image

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

resource "random_password" "redis" {
  count   = var.auth_enabled && var.password == "" ? 1 : 0
  length  = 24
  special = false
}

locals {
  password     = var.auth_enabled ? (var.password != "" ? var.password : random_password.redis[0].result) : ""
  service_name = "${var.name}-redis"
}

resource "kubernetes_deployment" "redis" {
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
          name  = "redis"
          image = var.image

          port {
            container_port = 6379
            name           = "redis"
          }

          dynamic "env" {
            for_each = var.auth_enabled ? [1] : []
            content {
              name  = "REDIS_PASSWORD"
              value = local.password
            }
          }

          args = var.auth_enabled ? ["--requirepass", local.password] : []

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
              command = ["redis-cli", "ping"]
            }
            initial_delay_seconds = 30
            period_seconds        = 10
            timeout_seconds       = 5
          }

          readiness_probe {
            exec {
              command = ["redis-cli", "ping"]
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

resource "kubernetes_service" "redis" {
  metadata {
    name      = local.service_name
    namespace = var.namespace
  }

  spec {
    selector = {
      app = local.service_name
    }

    port {
      name        = "redis"
      port        = 6379
      target_port = 6379
    }
  }
}
