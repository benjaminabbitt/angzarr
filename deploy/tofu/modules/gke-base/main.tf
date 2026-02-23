# GKE Base Module - Main
# GCP GKE cluster with node pools

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
    "angzarr-component" = "compute"
    "angzarr-compute"   = "gke"
  })
}

#------------------------------------------------------------------------------
# VPC (optional)
#------------------------------------------------------------------------------

resource "google_compute_network" "gke" {
  count = var.create_network ? 1 : 0

  name                    = "${var.cluster_name}-network"
  project                 = var.project_id
  auto_create_subnetworks = false
}

resource "google_compute_subnetwork" "gke" {
  count = var.create_network ? 1 : 0

  name          = "${var.cluster_name}-subnet"
  project       = var.project_id
  region        = var.region
  network       = google_compute_network.gke[0].id
  ip_cidr_range = var.subnet_cidr

  secondary_ip_range {
    range_name    = "pods"
    ip_cidr_range = var.pods_cidr
  }

  secondary_ip_range {
    range_name    = "services"
    ip_cidr_range = var.services_cidr
  }

  private_ip_google_access = true
}

locals {
  network    = var.create_network ? google_compute_network.gke[0].name : var.network
  subnetwork = var.create_network ? google_compute_subnetwork.gke[0].name : var.subnetwork
}

#------------------------------------------------------------------------------
# Service Account for Nodes
#------------------------------------------------------------------------------

resource "google_service_account" "nodes" {
  account_id   = "${var.cluster_name}-nodes"
  project      = var.project_id
  display_name = "GKE Node Service Account for ${var.cluster_name}"
}

resource "google_project_iam_member" "nodes_log_writer" {
  project = var.project_id
  role    = "roles/logging.logWriter"
  member  = "serviceAccount:${google_service_account.nodes.email}"
}

resource "google_project_iam_member" "nodes_metric_writer" {
  project = var.project_id
  role    = "roles/monitoring.metricWriter"
  member  = "serviceAccount:${google_service_account.nodes.email}"
}

resource "google_project_iam_member" "nodes_artifact_reader" {
  project = var.project_id
  role    = "roles/artifactregistry.reader"
  member  = "serviceAccount:${google_service_account.nodes.email}"
}

#------------------------------------------------------------------------------
# GKE Cluster
#------------------------------------------------------------------------------

resource "google_container_cluster" "angzarr" {
  name     = var.cluster_name
  project  = var.project_id
  location = var.region # Regional cluster for HA

  network    = local.network
  subnetwork = local.subnetwork

  # We manage node pools separately
  remove_default_node_pool = true
  initial_node_count       = 1

  release_channel {
    channel = var.release_channel
  }

  ip_allocation_policy {
    cluster_secondary_range_name  = var.create_network ? "pods" : var.pods_range_name
    services_secondary_range_name = var.create_network ? "services" : var.services_range_name
  }

  private_cluster_config {
    enable_private_nodes    = var.enable_private_nodes
    enable_private_endpoint = var.enable_private_endpoint
    master_ipv4_cidr_block  = var.master_ipv4_cidr_block
  }

  workload_identity_config {
    workload_pool = "${var.project_id}.svc.id.goog"
  }

  dynamic "network_policy" {
    for_each = var.enable_network_policy ? [1] : []
    content {
      enabled  = true
      provider = "CALICO"
    }
  }

  resource_labels = local.labels

  deletion_protection = var.deletion_protection
}

#------------------------------------------------------------------------------
# Node Pools
#------------------------------------------------------------------------------

resource "google_container_node_pool" "pools" {
  for_each = var.node_pools

  name     = each.key
  project  = var.project_id
  location = var.region
  cluster  = google_container_cluster.angzarr.name

  node_count = lookup(each.value, "node_count", null)

  dynamic "autoscaling" {
    for_each = lookup(each.value, "autoscaling", null) != null ? [each.value.autoscaling] : []
    content {
      min_node_count = autoscaling.value.min_nodes
      max_node_count = autoscaling.value.max_nodes
    }
  }

  node_config {
    machine_type = lookup(each.value, "machine_type", "e2-small")
    disk_size_gb = lookup(each.value, "disk_size_gb", 50)
    disk_type    = lookup(each.value, "disk_type", "pd-standard")

    service_account = google_service_account.nodes.email
    oauth_scopes    = ["https://www.googleapis.com/auth/cloud-platform"]

    labels = merge(local.labels, lookup(each.value, "labels", {}))

    dynamic "taint" {
      for_each = lookup(each.value, "taints", [])
      content {
        key    = taint.value.key
        value  = lookup(taint.value, "value", "")
        effect = taint.value.effect
      }
    }

    workload_metadata_config {
      mode = "GKE_METADATA"
    }
  }
}
