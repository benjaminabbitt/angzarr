# Messaging module - RabbitMQ or Kafka via Helm
# Supports both local (Helm charts) and cloud-managed message brokers

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
  }
}

# RabbitMQ via Bitnami Helm chart (OCI registry)
resource "helm_release" "rabbitmq" {
  count = var.type == "rabbitmq" && var.managed == false ? 1 : 0

  name       = var.release_name
  repository = "oci://registry-1.docker.io/bitnamicharts"
  chart      = "rabbitmq"
  version    = var.rabbitmq_chart_version
  namespace  = var.namespace

  values = [
    yamlencode({
      # Use AWS ECR registry for images (more stable tags than Docker Hub)
      global = {
        security = {
          allowInsecureImages = true
        }
      }
      image = {
        registry   = "public.ecr.aws"
        repository = "bitnami/rabbitmq"
        tag        = "4.0"
      }
      auth = {
        username = var.username
        password = var.password
      }
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
      metrics = {
        enabled = var.metrics_enabled
      }
      service = {
        ports = {
          amqp = 5672
        }
      }
    })
  ]

  wait = true
}

# Kafka via Bitnami Helm chart (OCI registry)
resource "helm_release" "kafka" {
  count = var.type == "kafka" && var.managed == false ? 1 : 0

  name       = var.release_name
  repository = "oci://registry-1.docker.io/bitnamicharts"
  chart      = "kafka"
  version    = var.kafka_chart_version
  namespace  = var.namespace

  values = [
    yamlencode({
      # Use AWS ECR registry for images (more stable tags than Docker Hub)
      global = {
        security = {
          allowInsecureImages = true
        }
      }
      image = {
        registry   = "public.ecr.aws"
        repository = "bitnami/kafka"
        tag        = "3.9"
      }
      listeners = {
        client = {
          protocol = var.kafka_sasl_enabled ? "SASL_PLAINTEXT" : "PLAINTEXT"
        }
      }
      sasl = {
        client = {
          users     = var.kafka_sasl_enabled ? [var.username] : []
          passwords = var.kafka_sasl_enabled ? var.password : ""
        }
      }
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
      metrics = {
        kafka = {
          enabled = var.metrics_enabled
        }
      }
      # Kraft mode (no zookeeper)
      kraft = {
        enabled = true
      }
      zookeeper = {
        enabled = false
      }
    })
  ]

  wait = true
}

# Kubernetes secret for messaging credentials
resource "kubernetes_secret" "messaging_credentials" {
  metadata {
    name      = "${var.release_name}-credentials"
    namespace = var.namespace
  }

  data = {
    username = var.username
    password = var.password
    host     = local.host
    port     = tostring(local.port)
    uri      = local.uri
  }
}

locals {
  # Bitnami charts append the chart name to the release name for services
  # e.g., release "angzarr-mq" -> service "angzarr-mq-rabbitmq"
  host = var.type == "rabbitmq" ? "${var.release_name}-rabbitmq.${var.namespace}.svc.cluster.local" : (
    var.type == "kafka" ? "${var.release_name}-kafka.${var.namespace}.svc.cluster.local" : var.external_host
  )

  port = var.type == "rabbitmq" ? 5672 : (
    var.type == "kafka" ? 9092 : var.external_port
  )

  uri = var.type == "rabbitmq" ? "amqp://${var.username}:${var.password}@${local.host}:${local.port}" : (
    var.type == "kafka" ? "${local.host}:${local.port}" : var.external_uri
  )
}
