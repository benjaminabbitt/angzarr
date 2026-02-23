# Infrastructure Module: Redis - Outputs

output "snapshot_store" {
  description = "Snapshot store configuration for stack module"
  value = {
    connection_uri = var.auth_enabled ? "redis://:${local.password}@${local.service_name}.${var.namespace}.svc.cluster.local:6379" : "redis://${local.service_name}.${var.namespace}.svc.cluster.local:6379"
    provides = {
      capabilities  = toset(["snapshot_store", "caching", "fast_reads"])
      rust_features = toset(["redis"])
    }
  }
  sensitive = true
}

output "connection_uri" {
  description = "Redis connection URI"
  value       = var.auth_enabled ? "redis://:${local.password}@${local.service_name}.${var.namespace}.svc.cluster.local:6379" : "redis://${local.service_name}.${var.namespace}.svc.cluster.local:6379"
  sensitive   = true
}

output "service_name" {
  description = "Kubernetes service name"
  value       = local.service_name
}

output "port" {
  description = "Redis port"
  value       = 6379
}
