# k3s Local Development Environment
# Deploys infrastructure using official images (no Bitnami)

terraform {
  required_version = ">= 1.0"

  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.0"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.0"
    }
  }

  backend "local" {
    path = "terraform.tfstate"
  }
}

provider "kubernetes" {
  config_path = var.kubeconfig_path
}

# Namespace for angzarr workloads
resource "kubernetes_namespace" "angzarr" {
  metadata {
    name = var.namespace
  }
}

# === Message Bus (choose one) ===

module "rabbitmq" {
  count  = var.bus_type == "rabbit" ? 1 : 0
  source = "../../modules/infra-rabbit"

  name      = "angzarr-mq"
  namespace = kubernetes_namespace.angzarr.metadata[0].name
}

module "nats" {
  count  = var.bus_type == "nats" ? 1 : 0
  source = "../../modules/infra-nats"

  name              = "angzarr-mq"
  namespace         = kubernetes_namespace.angzarr.metadata[0].name
  jetstream_enabled = true
}

module "kafka" {
  count  = var.bus_type == "kafka" ? 1 : 0
  source = "../../modules/infra-kafka"

  name      = "angzarr-mq"
  namespace = kubernetes_namespace.angzarr.metadata[0].name
}

# === Storage ===

module "postgres" {
  count  = var.enable_postgres ? 1 : 0
  source = "../../modules/infra-postgres"

  name      = "angzarr-db"
  namespace = kubernetes_namespace.angzarr.metadata[0].name
}

module "redis" {
  count  = var.enable_redis ? 1 : 0
  source = "../../modules/infra-redis"

  name         = "angzarr-cache"
  namespace    = kubernetes_namespace.angzarr.metadata[0].name
  auth_enabled = false
}

# === Registry ===

module "registry" {
  count  = var.enable_registry ? 1 : 0
  source = "../../modules/infra-registry"

  name      = "angzarr"
  namespace = kubernetes_namespace.angzarr.metadata[0].name
  node_port = 30500
}

# === Secrets ===
# Kubernetes secrets for Helm deployments
# Helm chart expects 'uri' key in each secret

resource "kubernetes_secret" "postgres_credentials" {
  count = var.enable_postgres ? 1 : 0

  metadata {
    name      = "angzarr-postgres"
    namespace = kubernetes_namespace.angzarr.metadata[0].name
  }

  data = {
    uri = length(module.postgres) > 0 ? module.postgres[0].connection_uri : ""
  }

  type = "Opaque"
}

resource "kubernetes_secret" "amqp_credentials" {
  count = var.bus_type == "rabbit" ? 1 : 0

  metadata {
    name      = "angzarr-amqp"
    namespace = kubernetes_namespace.angzarr.metadata[0].name
  }

  data = {
    uri = length(module.rabbitmq) > 0 ? module.rabbitmq[0].bus.connection_uri : ""
  }

  type = "Opaque"
}

resource "kubernetes_secret" "nats_credentials" {
  count = var.bus_type == "nats" ? 1 : 0

  metadata {
    name      = "angzarr-nats"
    namespace = kubernetes_namespace.angzarr.metadata[0].name
  }

  data = {
    uri = length(module.nats) > 0 ? module.nats[0].bus.connection_uri : ""
  }

  type = "Opaque"
}

resource "kubernetes_secret" "redis_credentials" {
  count = var.enable_redis ? 1 : 0

  metadata {
    name      = "angzarr-redis"
    namespace = kubernetes_namespace.angzarr.metadata[0].name
  }

  data = {
    uri = length(module.redis) > 0 ? "redis://${module.redis[0].service_name}:6379" : ""
  }

  type = "Opaque"
}
