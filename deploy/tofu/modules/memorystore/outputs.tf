# Memorystore Module - Outputs

locals {
  protocol       = var.transit_encryption_mode == "SERVER_AUTHENTICATION" ? "rediss" : "redis"
  auth_part      = var.auth_enabled ? ":${google_redis_instance.angzarr.auth_string}@" : ""
  connection_uri = "${local.protocol}://${local.auth_part}${google_redis_instance.angzarr.host}:${google_redis_instance.angzarr.port}"
}

output "provides" {
  description = "Capabilities provided by this module"
  value = {
    capabilities    = toset(["snapshot_store", "caching"])
    cloud           = "gcp"
    rust_features   = toset(["redis"])
    ha_mode         = var.tier == "STANDARD_HA" ? "multi-az" : "none"
    secrets_backend = "gcp"
  }
}

output "requirements" {
  description = "Requirements for this module"
  value = {
    compute_types   = null
    vpc             = true
    capabilities    = null
    secrets_backend = "gcp"
  }
}

output "connection_uri" {
  description = "Redis connection URI"
  value       = local.connection_uri
  sensitive   = true
}

output "snapshot_store" {
  description = "Snapshot store configuration for stack module"
  value = {
    connection_uri = local.connection_uri
    provides = {
      capabilities  = toset(["snapshot_store"])
      rust_features = toset(["redis"])
    }
  }
  sensitive = true
}

output "host" {
  description = "Redis host"
  value       = google_redis_instance.angzarr.host
}

output "port" {
  description = "Redis port"
  value       = google_redis_instance.angzarr.port
}

output "auth_string" {
  description = "Redis AUTH string"
  value       = var.auth_enabled ? google_redis_instance.angzarr.auth_string : null
  sensitive   = true
}

output "secret_id" {
  description = "Secret Manager secret ID for AUTH string"
  value       = var.auth_enabled ? google_secret_manager_secret.auth_string[0].secret_id : null
}
