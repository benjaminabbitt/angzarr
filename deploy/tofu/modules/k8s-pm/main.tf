# K8s PM Module - Main
# Deploys a process manager via Helm

terraform {
  required_providers {
    helm = {
      source  = "hashicorp/helm"
      version = ">= 2.0"
    }
  }
}

locals {
  chart_path = "${path.module}/../../../helm/angzarr-pm"

  # Common coordinator environment variables
  coordinator_env = {
    ANGZARR_PM_NAME        = var.name
    ANGZARR_EVENT_STORE    = var.storage.event_store.connection_uri
    ANGZARR_POSITION_STORE = var.storage.position_store.connection_uri
    ANGZARR_SNAPSHOT_STORE = var.storage.snapshot_store != null ? var.storage.snapshot_store.connection_uri : ""
    ANGZARR_BUS_URI        = var.bus.connection_uri
    ANGZARR_BUS_TYPE       = var.bus.type
    ANGZARR_SUBSCRIPTIONS  = join(";", var.subscriptions)
    ANGZARR_TARGETS        = join(";", var.targets)
  }
}

resource "helm_release" "pm" {
  name      = "pm-${var.name}"
  namespace = var.namespace
  chart     = local.chart_path

  values = [yamlencode({
    name          = var.name
    componentType = "pm"

    logic = {
      image = var.image
      env   = var.env
    }

    coordinator = {
      image = var.coordinator_images.pm
      env   = local.coordinator_env
    }

    subscriptions = var.subscriptions
    targets       = var.targets

    resources = var.resources

    labels = merge(var.labels, {
      "angzarr-component" = "pm"
      "angzarr-pm-name"   = var.name
    })
  })]
}
