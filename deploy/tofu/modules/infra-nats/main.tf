# Infrastructure Module: NATS
# Deploys NATS via Helm and outputs connection info

terraform {
  required_providers {
    helm = {
      source  = "hashicorp/helm"
      version = ">= 2.0"
    }
  }
}

locals {
  service_name = "${var.name}-nats"
}

resource "helm_release" "nats" {
  name       = var.name
  namespace  = var.namespace
  repository = "https://nats-io.github.io/k8s/helm/charts/"
  chart      = "nats"
  version    = var.chart_version

  values = [yamlencode({
    nats = {
      jetstream = {
        enabled = var.jetstream_enabled
        memStorage = {
          enabled = true
          size    = var.jetstream_mem_size
        }
        fileStorage = {
          enabled      = var.jetstream_file_enabled
          size         = var.jetstream_file_size
          storageClass = var.storage_class
        }
      }
    }
    cluster = {
      enabled  = var.cluster_enabled
      replicas = var.replicas
    }
    natsBox = {
      enabled = var.nats_box_enabled
    }
  })]
}
