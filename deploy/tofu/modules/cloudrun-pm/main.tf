# Cloud Run PM Module
# Deploys a process manager as a Cloud Run service
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
    ANGZARR_PM_NAME        = var.name
    ANGZARR_EVENT_STORE    = var.storage.event_store.connection_uri
    ANGZARR_POSITION_STORE = var.storage.position_store.connection_uri
    ANGZARR_SNAPSHOT_STORE = var.storage.snapshot_store != null ? var.storage.snapshot_store.connection_uri : ""
    ANGZARR_BUS_URI        = var.bus.connection_uri
    ANGZARR_BUS_TYPE       = var.bus.type
    ANGZARR_SUBSCRIPTIONS  = join(";", var.subscriptions)
    ANGZARR_TARGETS        = join(";", var.targets)
  }

  common_labels = merge(var.labels, {
    "angzarr-component" = "pm"
    "angzarr-pm-name"   = var.name
  })
}

resource "google_cloud_run_v2_service" "pm" {
  name     = "pm-${var.name}"
  location = var.region
  project  = var.project_id

  template {
    labels = local.common_labels

    # Coordinator sidecar
    containers {
      name  = "coordinator"
      image = var.coordinator_images.pm

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
          cpu    = var.resources.coordinator.cpu
          memory = var.resources.coordinator.memory
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
      image = var.image

      dynamic "env" {
        for_each = var.env
        content {
          name  = env.key
          value = env.value
        }
      }

      resources {
        limits = {
          cpu    = var.resources.logic.cpu
          memory = var.resources.logic.memory
        }
      }
    }

    scaling {
      min_instance_count = var.scaling.min_instances
      max_instance_count = var.scaling.max_instances
    }

    service_account = var.service_account
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }

  labels = local.common_labels
}
