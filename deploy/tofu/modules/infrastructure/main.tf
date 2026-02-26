# Infrastructure Module
# Deploys shared infrastructure services:
# - Stream: Event streaming service

terraform {
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 5.0"
    }
  }
}

locals {
  labels = merge(
    {
      "managed-by" = "opentofu"
      "component"  = "infrastructure"
    },
    var.labels
  )
}

#------------------------------------------------------------------------------
# Stream Service
#------------------------------------------------------------------------------
resource "google_cloud_run_v2_service" "stream" {
  count = var.stream.enabled ? 1 : 0

  name     = "angzarr-stream"
  location = var.region
  project  = var.project_id
  labels   = merge(local.labels, { "angzarr-component" = "stream" })

  template {
    labels          = merge(local.labels, { "angzarr-component" = "stream" })
    service_account = var.service_account

    scaling {
      min_instance_count = var.stream.min_instances
      max_instance_count = var.stream.max_instances
    }

    execution_environment = "EXECUTION_ENVIRONMENT_GEN2"
    timeout               = "300s"

    dynamic "vpc_access" {
      for_each = var.vpc_connector != null ? [1] : []
      content {
        connector = var.vpc_connector
        egress    = var.vpc_egress
      }
    }

    containers {
      name  = "stream"
      image = var.stream.image

      ports {
        name           = "h2c"
        container_port = 1340
      }

      resources {
        limits = {
          cpu    = var.stream.resources.cpu
          memory = var.stream.resources.memory
        }
        startup_cpu_boost = true
      }

      dynamic "env" {
        for_each = merge(var.coordinator_env, var.stream.env, {
          "RUST_LOG" = var.log_level
          "PORT"     = "1340"
        })
        content {
          name  = env.key
          value = env.value
        }
      }

      dynamic "env" {
        for_each = var.coordinator_secrets
        content {
          name = env.key
          value_source {
            secret_key_ref {
              secret  = env.value.secret
              version = env.value.version
            }
          }
        }
      }

      startup_probe {
        grpc {
          port = 1340
        }
        initial_delay_seconds = 5
        period_seconds        = 10
        failure_threshold     = 3
      }

      liveness_probe {
        grpc {
          port = 1340
        }
        period_seconds    = 30
        failure_threshold = 3
      }
    }
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }
}

#------------------------------------------------------------------------------
# IAM
#------------------------------------------------------------------------------
resource "google_cloud_run_v2_service_iam_member" "stream_public" {
  count = var.stream.enabled && var.allow_unauthenticated ? 1 : 0

  project  = var.project_id
  location = var.region
  name     = google_cloud_run_v2_service.stream[0].name
  role     = "roles/run.invoker"
  member   = "allUsers"
}
