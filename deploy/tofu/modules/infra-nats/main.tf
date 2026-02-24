# Infrastructure Module: NATS
# Deploys NATS using official image with JetStream

terraform {
  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = ">= 2.0"
    }
  }
}

locals {
  service_name = "${var.name}-nats"
}

resource "kubernetes_deployment" "nats" {
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
          name  = "nats"
          image = var.image

          port {
            container_port = 4222
            name           = "client"
          }

          port {
            container_port = 8222
            name           = "monitor"
          }

          port {
            container_port = 6222
            name           = "cluster"
          }

          # Enable JetStream with monitoring
          args = var.jetstream_enabled ? ["-js", "-m", "8222"] : ["-m", "8222"]

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
            http_get {
              path = "/healthz"
              port = 8222
            }
            initial_delay_seconds = 10
            period_seconds        = 10
            timeout_seconds       = 5
          }

          readiness_probe {
            http_get {
              path = "/healthz"
              port = 8222
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

resource "kubernetes_service" "nats" {
  metadata {
    name      = local.service_name
    namespace = var.namespace
  }

  spec {
    selector = {
      app = local.service_name
    }

    port {
      name        = "client"
      port        = 4222
      target_port = 4222
    }

    port {
      name        = "monitor"
      port        = 8222
      target_port = 8222
    }
  }
}
