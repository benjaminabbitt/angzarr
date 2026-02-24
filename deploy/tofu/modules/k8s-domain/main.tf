# K8s Domain Module - Main
# Deploys domain components via Helm

terraform {
  required_providers {
    helm = {
      source  = "hashicorp/helm"
      version = ">= 2.0"
    }
  }
}

locals {
  chart_path = "${path.module}/../../../k8s/helm/angzarr-domain"

  # Common coordinator environment variables
  coordinator_env = {
    ANGZARR_DOMAIN         = var.domain
    ANGZARR_EVENT_STORE    = var.storage.event_store.connection_uri
    ANGZARR_POSITION_STORE = var.storage.position_store.connection_uri
    ANGZARR_SNAPSHOT_STORE = var.storage.snapshot_store != null ? var.storage.snapshot_store.connection_uri : ""
    ANGZARR_BUS_URI        = var.bus.connection_uri
    ANGZARR_BUS_TYPE       = var.bus.type
  }
}

#------------------------------------------------------------------------------
# Aggregate
#------------------------------------------------------------------------------

resource "helm_release" "aggregate" {
  name      = "${var.domain}-aggregate"
  namespace = var.namespace
  chart     = local.chart_path

  values = [yamlencode({
    domain        = var.domain
    componentType = "aggregate"

    logic = {
      image = var.aggregate.image
      env   = var.aggregate.env
    }

    coordinator = {
      image = var.coordinator_images.aggregate
      env   = local.coordinator_env
    }

    grpcGateway = var.coordinator_images.grpc_gateway != null ? {
      enabled = true
      image   = var.coordinator_images.grpc_gateway
    } : { enabled = false }

    resources = var.resources.aggregate

    labels = merge(var.labels, {
      "angzarr-domain"    = var.domain
      "angzarr-component" = "aggregate"
    })
  })]
}

#------------------------------------------------------------------------------
# Sagas
#------------------------------------------------------------------------------

resource "helm_release" "saga" {
  for_each = var.sagas

  name      = "saga-${var.domain}-${each.key}"
  namespace = var.namespace
  chart     = local.chart_path

  values = [yamlencode({
    domain        = var.domain
    componentType = "saga"
    sagaName      = each.key
    targetDomain  = each.value.target_domain

    logic = {
      image = each.value.image
      env   = each.value.env
    }

    coordinator = {
      image = var.coordinator_images.saga
      env = merge(local.coordinator_env, {
        ANGZARR_TARGET_DOMAIN = each.value.target_domain
      })
    }

    grpcGateway = var.coordinator_images.grpc_gateway != null ? {
      enabled = true
      image   = var.coordinator_images.grpc_gateway
    } : { enabled = false }

    resources = var.resources.saga

    labels = merge(var.labels, {
      "angzarr-domain"        = var.domain
      "angzarr-component"     = "saga"
      "angzarr-saga-name"     = each.key
      "angzarr-target-domain" = each.value.target_domain
    })
  })]
}

#------------------------------------------------------------------------------
# Projectors
#------------------------------------------------------------------------------

resource "helm_release" "projector" {
  for_each = var.projectors

  name      = "projector-${var.domain}-${each.key}"
  namespace = var.namespace
  chart     = local.chart_path

  values = [yamlencode({
    domain        = var.domain
    componentType = "projector"
    projectorName = each.key

    logic = {
      image = each.value.image
      env   = each.value.env
    }

    coordinator = {
      image = var.coordinator_images.projector
      env   = local.coordinator_env
    }

    grpcGateway = var.coordinator_images.grpc_gateway != null ? {
      enabled = true
      image   = var.coordinator_images.grpc_gateway
    } : { enabled = false }

    resources = var.resources.projector

    labels = merge(var.labels, {
      "angzarr-domain"         = var.domain
      "angzarr-component"      = "projector"
      "angzarr-projector-name" = each.key
    })
  })]
}
