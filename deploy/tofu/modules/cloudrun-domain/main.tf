# Cloud Run Domain Module
# Deploys domain components (aggregate, sagas, projectors) as Cloud Run services
# Uses multi-container support for sidecar pattern (coordinator + business logic)

terraform {
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = ">= 5.0"
    }
  }
}

locals {
  # Common coordinator environment variables
  coordinator_env = {
    ANGZARR_DOMAIN         = var.domain
    ANGZARR_EVENT_STORE    = var.storage.event_store.connection_uri
    ANGZARR_POSITION_STORE = var.storage.position_store.connection_uri
    ANGZARR_SNAPSHOT_STORE = var.storage.snapshot_store != null ? var.storage.snapshot_store.connection_uri : ""
    ANGZARR_BUS_URI        = var.bus.connection_uri
    ANGZARR_BUS_TYPE       = var.bus.type
  }

  common_labels = merge(var.labels, {
    "angzarr-domain" = var.domain
  })
}

#------------------------------------------------------------------------------
# Aggregate
#------------------------------------------------------------------------------

resource "google_cloud_run_v2_service" "aggregate" {
  name     = "${var.domain}-aggregate"
  location = var.region
  project  = var.project_id

  template {
    labels = merge(local.common_labels, {
      "angzarr-component" = "aggregate"
    })

    # Coordinator sidecar
    containers {
      name  = "coordinator"
      image = var.coordinator_images.aggregate

      ports {
        container_port = 1310
      }

      dynamic "env" {
        for_each = local.coordinator_env
        content {
          name  = env.key
          value = env.value
        }
      }

      resources {
        limits = {
          cpu    = var.resources.aggregate.coordinator.cpu
          memory = var.resources.aggregate.coordinator.memory
        }
      }

      startup_probe {
        grpc {
          port = 1310
        }
        initial_delay_seconds = 5
        period_seconds        = 10
      }
    }

    # Business logic container
    containers {
      name  = "logic"
      image = var.aggregate.image

      dynamic "env" {
        for_each = var.aggregate.env
        content {
          name  = env.key
          value = env.value
        }
      }

      resources {
        limits = {
          cpu    = var.resources.aggregate.logic.cpu
          memory = var.resources.aggregate.logic.memory
        }
      }
    }

    scaling {
      min_instance_count = var.scaling.aggregate.min_instances
      max_instance_count = var.scaling.aggregate.max_instances
    }

    service_account = var.service_account
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }

  labels = local.common_labels
}

#------------------------------------------------------------------------------
# Sagas
#------------------------------------------------------------------------------

resource "google_cloud_run_v2_service" "saga" {
  for_each = var.sagas

  name     = "saga-${var.domain}-${each.key}"
  location = var.region
  project  = var.project_id

  template {
    labels = merge(local.common_labels, {
      "angzarr-component"     = "saga"
      "angzarr-saga-name"     = each.key
      "angzarr-target-domain" = each.value.target_domain
    })

    # Coordinator sidecar
    containers {
      name  = "coordinator"
      image = var.coordinator_images.saga

      ports {
        container_port = 1310
      }

      dynamic "env" {
        for_each = merge(local.coordinator_env, {
          ANGZARR_TARGET_DOMAIN = each.value.target_domain
        })
        content {
          name  = env.key
          value = env.value
        }
      }

      resources {
        limits = {
          cpu    = var.resources.saga.coordinator.cpu
          memory = var.resources.saga.coordinator.memory
        }
      }

      startup_probe {
        grpc {
          port = 1310
        }
        initial_delay_seconds = 5
        period_seconds        = 10
      }
    }

    # Business logic container
    containers {
      name  = "logic"
      image = each.value.image

      dynamic "env" {
        for_each = each.value.env
        content {
          name  = env.key
          value = env.value
        }
      }

      resources {
        limits = {
          cpu    = var.resources.saga.logic.cpu
          memory = var.resources.saga.logic.memory
        }
      }
    }

    scaling {
      min_instance_count = var.scaling.saga.min_instances
      max_instance_count = var.scaling.saga.max_instances
    }

    service_account = var.service_account
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }

  labels = merge(local.common_labels, {
    "angzarr-component" = "saga"
  })
}

#------------------------------------------------------------------------------
# Projectors
#------------------------------------------------------------------------------

resource "google_cloud_run_v2_service" "projector" {
  for_each = var.projectors

  name     = "projector-${var.domain}-${each.key}"
  location = var.region
  project  = var.project_id

  template {
    labels = merge(local.common_labels, {
      "angzarr-component"      = "projector"
      "angzarr-projector-name" = each.key
    })

    # Coordinator sidecar
    containers {
      name  = "coordinator"
      image = var.coordinator_images.projector

      ports {
        container_port = 1310
      }

      dynamic "env" {
        for_each = local.coordinator_env
        content {
          name  = env.key
          value = env.value
        }
      }

      resources {
        limits = {
          cpu    = var.resources.projector.coordinator.cpu
          memory = var.resources.projector.coordinator.memory
        }
      }

      startup_probe {
        grpc {
          port = 1310
        }
        initial_delay_seconds = 5
        period_seconds        = 10
      }
    }

    # Business logic container
    containers {
      name  = "logic"
      image = each.value.image

      dynamic "env" {
        for_each = each.value.env
        content {
          name  = env.key
          value = env.value
        }
      }

      resources {
        limits = {
          cpu    = var.resources.projector.logic.cpu
          memory = var.resources.projector.logic.memory
        }
      }
    }

    scaling {
      min_instance_count = var.scaling.projector.min_instances
      max_instance_count = var.scaling.projector.max_instances
    }

    service_account = var.service_account
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }

  labels = merge(local.common_labels, {
    "angzarr-component" = "projector"
  })
}
