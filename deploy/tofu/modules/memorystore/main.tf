# Memorystore Module - Main
# GCP Memorystore for Redis (snapshot store)

terraform {
  required_version = ">= 1.0"

  required_providers {
    google = {
      source  = "hashicorp/google"
      version = ">= 5.0"
    }
    random = {
      source  = "hashicorp/random"
      version = ">= 3.0"
    }
  }
}

locals {
  labels = merge(var.labels, {
    "angzarr-component" = "storage"
    "angzarr-storage"   = "memorystore"
  })
}

#------------------------------------------------------------------------------
# Memorystore Redis Instance
#------------------------------------------------------------------------------

resource "google_redis_instance" "angzarr" {
  name           = var.name
  project        = var.project_id
  region         = var.region
  tier           = var.tier
  memory_size_gb = var.memory_size_gb

  redis_version = var.redis_version

  authorized_network = var.authorized_network
  connect_mode       = var.connect_mode

  auth_enabled            = var.auth_enabled
  transit_encryption_mode = var.transit_encryption_mode

  redis_configs = var.redis_configs

  labels = local.labels
}

#------------------------------------------------------------------------------
# Secret Manager (for AUTH string)
#------------------------------------------------------------------------------

resource "google_secret_manager_secret" "auth_string" {
  count = var.auth_enabled ? 1 : 0

  secret_id = "${var.name}-auth-string"
  project   = var.project_id

  replication {
    auto {}
  }

  labels = local.labels
}

resource "google_secret_manager_secret_version" "auth_string" {
  count = var.auth_enabled ? 1 : 0

  secret      = google_secret_manager_secret.auth_string[0].id
  secret_data = google_redis_instance.angzarr.auth_string
}
