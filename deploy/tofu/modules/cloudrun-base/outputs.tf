# Cloud Run Base Module - Outputs

output "provides" {
  description = "Capabilities provided by this module"
  value = {
    capabilities    = toset(["compute"])
    compute_type    = "cloudrun"
    cloud           = "gcp"
    region          = var.region
    ha_mode         = "multi-az" # Cloud Run is regional
    rust_features   = []
    secrets_backend = "gcp"
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

output "service_account_email" {
  description = "Service account email for Cloud Run services"
  value       = google_service_account.cloudrun.email
}

output "service_account_name" {
  description = "Service account name"
  value       = google_service_account.cloudrun.name
}

output "vpc_connector_id" {
  description = "VPC Access connector ID (if created)"
  value       = var.create_vpc_connector ? google_vpc_access_connector.cloudrun[0].id : null
}

output "vpc_connector_name" {
  description = "VPC Access connector name (if created)"
  value       = var.create_vpc_connector ? google_vpc_access_connector.cloudrun[0].name : null
}

output "region" {
  description = "GCP region"
  value       = var.region
}

output "project_id" {
  description = "GCP project ID"
  value       = var.project_id
}
