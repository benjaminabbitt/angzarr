# Infrastructure Module: Kafka
# Deploys Kafka via Helm and outputs connection info

terraform {
  required_providers {
    helm = {
      source  = "hashicorp/helm"
      version = ">= 2.0"
    }
  }
}

locals {
  service_name = "${var.name}-kafka"
}

resource "helm_release" "kafka" {
  name      = var.name
  namespace = var.namespace
  chart     = "${path.module}/../../../helm/angzarr-mq-kafka"

  values = [yamlencode({
    kafka = {
      replicaCount = var.replicas
      kraft = {
        enabled = var.kraft_enabled
      }
      persistence = {
        enabled      = var.persistence_enabled
        size         = var.persistence_size
        storageClass = var.storage_class
      }
      resources = var.resources
      listeners = {
        client = {
          protocol = var.security_protocol
        }
      }
    }
    zookeeper = {
      enabled = !var.kraft_enabled
    }
  })]
}
