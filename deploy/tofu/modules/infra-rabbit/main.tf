# Infrastructure Module: RabbitMQ
# Deploys RabbitMQ using official image

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

resource "random_password" "rabbitmq" {
  count   = var.password == "" ? 1 : 0
  length  = 24
  special = false
}

resource "random_password" "erlang_cookie" {
  count   = var.erlang_cookie == "" ? 1 : 0
  length  = 32
  special = false
}

locals {
  password      = var.password != "" ? var.password : random_password.rabbitmq[0].result
  erlang_cookie = var.erlang_cookie != "" ? var.erlang_cookie : random_password.erlang_cookie[0].result
  service_name  = "${var.name}-rabbitmq"
}

resource "kubernetes_deployment" "rabbitmq" {
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
          name  = "rabbitmq"
          image = var.image

          port {
            container_port = 5672
            name           = "amqp"
          }

          port {
            container_port = 15672
            name           = "management"
          }

          env {
            name  = "RABBITMQ_DEFAULT_USER"
            value = var.username
          }

          env {
            name  = "RABBITMQ_DEFAULT_PASS"
            value = local.password
          }

          env {
            name  = "RABBITMQ_ERLANG_COOKIE"
            value = local.erlang_cookie
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

          # Increased timeouts for resource-constrained environments
          liveness_probe {
            exec {
              command = ["rabbitmq-diagnostics", "check_running"]
            }
            initial_delay_seconds = 120
            period_seconds        = 60
            timeout_seconds       = 30
            failure_threshold     = 5
          }

          readiness_probe {
            exec {
              command = ["rabbitmq-diagnostics", "check_running"]
            }
            initial_delay_seconds = 30
            period_seconds        = 30
            timeout_seconds       = 30
            failure_threshold     = 5
          }
        }
      }
    }
  }
}

resource "kubernetes_service" "rabbitmq" {
  metadata {
    name      = local.service_name
    namespace = var.namespace
  }

  spec {
    selector = {
      app = local.service_name
    }

    port {
      name        = "amqp"
      port        = 5672
      target_port = 5672
    }

    port {
      name        = "management"
      port        = 15672
      target_port = 15672
    }
  }
}
