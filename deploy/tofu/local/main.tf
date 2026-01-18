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

# Compute effective enable flags based on messaging_type
locals {
  enable_rabbitmq = var.enable_rabbitmq != null ? var.enable_rabbitmq : var.messaging_type == "amqp"
  enable_kafka    = var.enable_kafka != null ? var.enable_kafka : var.messaging_type == "kafka"
}

# Deploy RabbitMQ
resource "helm_release" "rabbitmq" {
  count = local.enable_rabbitmq ? 1 : 0

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

# Deploy Kafka
resource "helm_release" "kafka" {
  count = local.enable_kafka ? 1 : 0

  name       = "kafka"
  repository = "https://charts.bitnami.com/bitnami"
  chart      = "kafka"
  version    = var.kafka_chart_version
  namespace  = kubernetes_namespace.angzarr.metadata[0].name

  values = [
    yamlencode({
      # Controller configuration (KRaft mode - no ZooKeeper)
      controller = {
        replicaCount = 1
        persistence = {
          size = "2Gi"
        }
        resources = {
          requests = {
            memory = "512Mi"
            cpu    = "250m"
          }
          limits = {
            memory = "1Gi"
            cpu    = "1"
          }
        }
      }

      # Broker configuration
      broker = {
        replicaCount = 1
        persistence = {
          size = "2Gi"
        }
        resources = {
          requests = {
            memory = "512Mi"
            cpu    = "250m"
          }
          limits = {
            memory = "1Gi"
            cpu    = "1"
          }
        }
      }

      # SASL authentication
      sasl = {
        client = {
          users     = [var.kafka_user]
          passwords = var.kafka_password
        }
        interbroker = {
          user     = "interbroker"
          password = var.kafka_password
        }
        controller = {
          user     = "controller"
          password = var.kafka_password
        }
      }

      # Listeners
      listeners = {
        client = {
          protocol = "SASL_PLAINTEXT"
        }
        controller = {
          protocol = "SASL_PLAINTEXT"
        }
        interbroker = {
          protocol = "SASL_PLAINTEXT"
        }
      }

      # SASL mechanism
      saslMechanisms = var.kafka_sasl_mechanism

      # Topic auto-creation for development
      autoCreateTopicsEnable = true

      # Log retention
      logRetentionHours = 168 # 7 days
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

# Create Kubernetes secret for messaging credentials
resource "kubernetes_secret" "messaging_credentials" {
  metadata {
    name      = "angzarr-messaging-credentials"
    namespace = kubernetes_namespace.angzarr.metadata[0].name
  }

  data = {
    # AMQP credentials
    amqp_url = local.enable_rabbitmq ? "amqp://${var.rabbitmq_user}:${var.rabbitmq_password}@rabbitmq:5672" : ""

    # Kafka credentials
    kafka_bootstrap_servers = local.enable_kafka ? "kafka:9092" : ""
    kafka_sasl_username     = local.enable_kafka ? var.kafka_user : ""
    kafka_sasl_password     = local.enable_kafka ? var.kafka_password : ""
    kafka_sasl_mechanism    = local.enable_kafka ? var.kafka_sasl_mechanism : ""
  }

  type = "Opaque"
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

      # Messaging configuration
      messaging = {
        type = var.messaging_type

        amqp = {
          enabled = local.enable_rabbitmq
          # URL is provided via secret
          secretName = kubernetes_secret.messaging_credentials.metadata[0].name
          secretKey  = "amqp_url"
        }

        kafka = {
          enabled = local.enable_kafka
          # Credentials provided via secret
          secretName          = kubernetes_secret.messaging_credentials.metadata[0].name
          bootstrapServersKey = "kafka_bootstrap_servers"
          saslUsernameKey     = "kafka_sasl_username"
          saslPasswordKey     = "kafka_sasl_password"
          saslMechanismKey    = "kafka_sasl_mechanism"
          topicPrefix         = "angzarr"
          securityProtocol    = "SASL_PLAINTEXT"
        }
      }

      # Legacy amqp config for backwards compatibility
      amqp = {
        enabled = local.enable_rabbitmq
        url     = local.enable_rabbitmq ? "amqp://${var.rabbitmq_user}:${var.rabbitmq_password}@rabbitmq:5672" : ""
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

  depends_on = [
    helm_release.rabbitmq,
    helm_release.kafka,
    helm_release.redis,
    kubernetes_secret.messaging_credentials
  ]
  wait = true
}
