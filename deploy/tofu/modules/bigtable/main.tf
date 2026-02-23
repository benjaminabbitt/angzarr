# Bigtable Module - Main
# GCP Cloud Bigtable for event store

terraform {
  required_version = ">= 1.0"

  required_providers {
    google = {
      source  = "hashicorp/google"
      version = ">= 5.0"
    }
  }
}

locals {
  labels = merge(var.labels, {
    "angzarr-component" = "storage"
    "angzarr-storage"   = "bigtable"
  })
}

#------------------------------------------------------------------------------
# Bigtable Instance
#------------------------------------------------------------------------------

resource "google_bigtable_instance" "angzarr" {
  name                = var.instance_name
  project             = var.project_id
  deletion_protection = var.deletion_protection

  labels = local.labels

  cluster {
    cluster_id   = "${var.instance_name}-cluster"
    zone         = var.zone
    num_nodes    = var.num_nodes
    storage_type = var.storage_type
    kms_key_name = var.kms_key_name

    dynamic "autoscaling_config" {
      for_each = var.autoscaling != null ? [var.autoscaling] : []
      content {
        min_nodes      = autoscaling_config.value.min_nodes
        max_nodes      = autoscaling_config.value.max_nodes
        cpu_target     = autoscaling_config.value.cpu_target
        storage_target = autoscaling_config.value.storage_target
      }
    }
  }
}

#------------------------------------------------------------------------------
# Events Table
#------------------------------------------------------------------------------

resource "google_bigtable_table" "events" {
  name          = "events"
  instance_name = google_bigtable_instance.angzarr.name
  project       = var.project_id

  deletion_protection = var.deletion_protection ? "PROTECTED" : "UNPROTECTED"

  column_family {
    family = "e" # events
  }

  column_family {
    family = "m" # metadata
  }

  dynamic "column_family" {
    for_each = var.events_gc_policy != null ? [1] : []
    content {
      family = "e"
    }
  }
}

#------------------------------------------------------------------------------
# Positions Table
#------------------------------------------------------------------------------

resource "google_bigtable_table" "positions" {
  name          = "positions"
  instance_name = google_bigtable_instance.angzarr.name
  project       = var.project_id

  deletion_protection = var.deletion_protection ? "PROTECTED" : "UNPROTECTED"

  column_family {
    family = "p" # positions
  }
}

#------------------------------------------------------------------------------
# Snapshots Table
#------------------------------------------------------------------------------

resource "google_bigtable_table" "snapshots" {
  name          = "snapshots"
  instance_name = google_bigtable_instance.angzarr.name
  project       = var.project_id

  deletion_protection = var.deletion_protection ? "PROTECTED" : "UNPROTECTED"

  column_family {
    family = "s" # snapshots
  }

  column_family {
    family = "m" # metadata
  }
}

#------------------------------------------------------------------------------
# IAM Bindings
#------------------------------------------------------------------------------

resource "google_bigtable_instance_iam_member" "reader" {
  for_each = toset(var.reader_members)

  project  = var.project_id
  instance = google_bigtable_instance.angzarr.name
  role     = "roles/bigtable.reader"
  member   = each.value
}

resource "google_bigtable_instance_iam_member" "writer" {
  for_each = toset(var.writer_members)

  project  = var.project_id
  instance = google_bigtable_instance.angzarr.name
  role     = "roles/bigtable.user"
  member   = each.value
}
