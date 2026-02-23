# Infrastructure Module: PostgreSQL - Outputs

output "storage" {
  description = "Storage configuration for stack module (all stores)"
  value = {
    event_store = {
      connection_uri = "postgres://${var.username}:${local.password}@${local.service_name}.${var.namespace}.svc.cluster.local:5432/${var.database}"
      provides = {
        capabilities  = toset(["event_store", "position_store", "transactions", "queries"])
        rust_features = toset(["postgres"])
      }
    }
    position_store = {
      connection_uri = "postgres://${var.username}:${local.password}@${local.service_name}.${var.namespace}.svc.cluster.local:5432/${var.database}"
      provides = {
        capabilities  = toset(["position_store", "transactions"])
        rust_features = toset(["postgres"])
      }
    }
    snapshot_store = {
      connection_uri = "postgres://${var.username}:${local.password}@${local.service_name}.${var.namespace}.svc.cluster.local:5432/${var.database}"
      provides = {
        capabilities  = toset(["snapshot_store", "transactions"])
        rust_features = toset(["postgres"])
      }
    }
  }
  sensitive = true
}

output "connection_uri" {
  description = "PostgreSQL connection URI"
  value       = "postgres://${var.username}:${local.password}@${local.service_name}.${var.namespace}.svc.cluster.local:5432/${var.database}"
  sensitive   = true
}

output "service_name" {
  description = "Kubernetes service name"
  value       = local.service_name
}

output "port" {
  description = "PostgreSQL port"
  value       = 5432
}
