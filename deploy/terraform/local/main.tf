# Local Kubernetes deployment using Helm
# Works with minikube, kind, or any local k8s cluster

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
}

# Configure providers to use current kubectl context
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

# Create namespace
resource "kubernetes_namespace" "angzarr" {
  metadata {
    name = var.namespace
    labels = {
      name        = var.namespace
      environment = "local"
    }
  }
}

# Deploy RabbitMQ
resource "helm_release" "rabbitmq" {
  count = var.enable_rabbitmq ? 1 : 0

  name       = "rabbitmq"
  repository = "https://charts.bitnami.com/bitnami"
  chart      = "rabbitmq"
  version    = var.rabbitmq_chart_version
  namespace  = kubernetes_namespace.angzarr.metadata[0].name

  values = [
    yamlencode({
      auth = {
        username = var.rabbitmq_user
        password = var.rabbitmq_password
      }
      persistence = {
        size = "1Gi"
      }
      resources = {
        requests = {
          memory = "256Mi"
          cpu    = "100m"
        }
        limits = {
          memory = "512Mi"
          cpu    = "500m"
        }
      }
    })
  ]

  wait = true
}

# Deploy Redis
resource "helm_release" "redis" {
  count = var.enable_redis ? 1 : 0

  name       = "redis"
  repository = "https://charts.bitnami.com/bitnami"
  chart      = "redis"
  version    = var.redis_chart_version
  namespace  = kubernetes_namespace.angzarr.metadata[0].name

  values = [
    yamlencode({
      architecture = "standalone"
      auth = {
        enabled = false
      }
      master = {
        persistence = {
          size = "1Gi"
        }
        resources = {
          requests = {
            memory = "128Mi"
            cpu    = "100m"
          }
          limits = {
            memory = "256Mi"
            cpu    = "250m"
          }
        }
      }
    })
  ]

  wait = true
}

# Deploy angzarr with applications
resource "helm_release" "angzarr" {
  name      = "angzarr"
  chart     = "${path.module}/../../helm/angzarr"
  namespace = kubernetes_namespace.angzarr.metadata[0].name

  values = [
    yamlencode({
      replicaCount = var.replicas

      # Angzarr sidecar image
      image = {
        repository = var.angzarr_image_repository
        tag        = var.angzarr_image_tag
        pullPolicy = "IfNotPresent"
      }

      resources = {
        requests = {
          memory = "128Mi"
          cpu    = "100m"
        }
        limits = {
          memory = "512Mi"
          cpu    = "500m"
        }
      }

      storage = {
        type = var.storage_type
        sqlite = {
          persistence = {
            enabled = var.storage_type == "sqlite"
            size    = "1Gi"
          }
        }
        redis = {
          host = var.enable_redis ? "redis-master" : ""
          port = 6379
        }
      }

      amqp = {
        enabled = var.enable_rabbitmq
        url     = var.enable_rabbitmq ? "amqp://${var.rabbitmq_user}:${var.rabbitmq_password}@rabbitmq:5672" : ""
      }

      # Applications with evented as sidecar
      applications = {
        business   = var.business_applications
        projectors = var.projector_applications
        sagas      = var.saga_applications
      }

      logging = {
        level  = var.log_level
        format = "json"
      }
    })
  ]

  depends_on = [helm_release.rabbitmq, helm_release.redis]
  wait       = true
}
