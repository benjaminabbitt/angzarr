# Example: GCP Minimal Stack
# Cloud Run + Pub/Sub + Bigtable (serverless)
#
# Prerequisites:
# - GCP credentials configured (gcloud auth application-default login)
# - Terraform/OpenTofu installed

terraform {
  required_version = ">= 1.0"

  required_providers {
    google = {
      source  = "hashicorp/google"
      version = ">= 5.0"
    }
  }
}

#------------------------------------------------------------------------------
# GCP Provider
#------------------------------------------------------------------------------

provider "google" {
  project = var.project_id
  region  = var.region
}

variable "project_id" {
  description = "GCP project ID"
  type        = string
}

variable "region" {
  description = "GCP region"
  type        = string
  default     = "us-central1"
}

variable "zone" {
  description = "GCP zone (for Bigtable)"
  type        = string
  default     = "us-central1-a"
}

#------------------------------------------------------------------------------
# Infrastructure
#------------------------------------------------------------------------------

module "pubsub" {
  source = "../../../modules/pubsub"

  project_id = var.project_id
  name       = "angzarr-poker"
}

module "bigtable" {
  source = "../../../modules/bigtable"

  project_id    = var.project_id
  instance_name = "angzarr-poker"
  zone          = var.zone
  num_nodes     = 1
  storage_type  = "SSD"
}

module "cloudrun_base" {
  source = "../../../modules/cloudrun-base"

  project_id  = var.project_id
  region      = var.region
  name_prefix = "angzarr"
}

#------------------------------------------------------------------------------
# Stack
#------------------------------------------------------------------------------

module "stack" {
  source = "../../../modules/stack"

  name = "poker"

  compute = {
    compute_type    = "cloudrun"
    project_id      = var.project_id
    region          = var.region
    service_account = module.cloudrun_base.service_account_email
  }

  bus = module.pubsub.bus

  default_storage = {
    event_store    = module.bigtable.event_store
    position_store = module.bigtable.position_store
    snapshot_store = null # No snapshots for this minimal example
  }

  coordinator_images = {
    aggregate    = "gcr.io/${var.project_id}/coordinator-aggregate:latest"
    saga         = "gcr.io/${var.project_id}/coordinator-saga:latest"
    projector    = "gcr.io/${var.project_id}/coordinator-projector:latest"
    pm           = "gcr.io/${var.project_id}/coordinator-pm:latest"
    grpc_gateway = null
  }

  domains = {
    player = {
      aggregate = {
        image = "gcr.io/${var.project_id}/player-aggregate:latest"
      }
    }
    table = {
      aggregate = {
        image = "gcr.io/${var.project_id}/table-aggregate:latest"
      }
      sagas = {
        hand = {
          target_domain = "hand"
          image         = "gcr.io/${var.project_id}/saga-table-hand:latest"
        }
      }
    }
    hand = {
      aggregate = {
        image = "gcr.io/${var.project_id}/hand-aggregate:latest"
      }
      sagas = {
        table = {
          target_domain = "table"
          image         = "gcr.io/${var.project_id}/saga-hand-table:latest"
        }
      }
    }
  }

  process_managers = {}
}

#------------------------------------------------------------------------------
# Outputs
#------------------------------------------------------------------------------

output "topology_mermaid" {
  value = module.stack.topology_mermaid
}

output "bigtable_instance" {
  value = module.bigtable.instance_name
}

output "pubsub_topic" {
  value = module.pubsub.events_topic_name
}

output "service_account" {
  value = module.cloudrun_base.service_account_email
}
