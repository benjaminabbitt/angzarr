# Infrastructure Module: Container Registry
# Deploys a local container registry for development

terraform {
  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = ">= 2.0"
    }
  }
}

locals {
  service_name = "${var.name}-registry"
}

resource "kubernetes_deployment" "registry" {
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
          name  = "registry"
          image = var.image

          port {
            container_port = 5000
            name           = "registry"
          }

          env {
            name  = "REGISTRY_STORAGE_DELETE_ENABLED"
            value = "true"
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
            http_get {
              path = "/v2/"
              port = 5000
            }
            initial_delay_seconds = 5
            period_seconds        = 10
          }

          readiness_probe {
            http_get {
              path = "/v2/"
              port = 5000
            }
            initial_delay_seconds = 2
            period_seconds        = 5
          }
        }
      }
    }
  }
}

resource "kubernetes_service" "registry" {
  metadata {
    name      = local.service_name
    namespace = var.namespace
  }

  spec {
    selector = {
      app = local.service_name
    }

    port {
      name        = "registry"
      port        = 5000
      target_port = 5000
      node_port   = var.node_port
    }

    type = "NodePort"
  }
}
