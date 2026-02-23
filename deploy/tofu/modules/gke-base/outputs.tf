# GKE Base Module - Outputs

output "provides" {
  description = "Capabilities provided by this module"
  value = {
    capabilities    = toset(["compute"])
    compute_type    = "gke"
    cloud           = "gcp"
    region          = var.region
    ha_mode         = "multi-az" # Regional cluster
    rust_features   = []
    secrets_backend = "k8s"
  }
}

output "requirements" {
  description = "Requirements for this module"
  value = {
    compute_types   = null
    vpc             = false
    capabilities    = null
    secrets_backend = null
  }
}

output "cluster_name" {
  description = "GKE cluster name"
  value       = google_container_cluster.angzarr.name
}

output "cluster_endpoint" {
  description = "GKE cluster endpoint"
  value       = google_container_cluster.angzarr.endpoint
}

output "cluster_ca_certificate" {
  description = "Base64 encoded cluster CA certificate"
  value       = google_container_cluster.angzarr.master_auth[0].cluster_ca_certificate
  sensitive   = true
}

output "cluster_location" {
  description = "Cluster location (region)"
  value       = google_container_cluster.angzarr.location
}

output "network" {
  description = "VPC network name"
  value       = local.network
}

output "subnetwork" {
  description = "Subnetwork name"
  value       = local.subnetwork
}

output "node_service_account" {
  description = "Service account email for nodes"
  value       = google_service_account.nodes.email
}

output "workload_identity_pool" {
  description = "Workload Identity pool for binding K8s SAs to GCP SAs"
  value       = "${var.project_id}.svc.id.goog"
}
