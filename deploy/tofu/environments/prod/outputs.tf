# Production environment outputs

output "namespace" {
  description = "Kubernetes namespace"
  value       = kubernetes_namespace.angzarr.metadata[0].name
}

output "database_host" {
  description = "Database host"
  value       = module.database.host
}

output "database_secret" {
  description = "Database credentials secret name"
  value       = module.database.secret_name
}

output "messaging_host" {
  description = "Message broker host"
  value       = module.messaging.host
}

output "messaging_secret" {
  description = "Messaging credentials secret name"
  value       = module.messaging.secret_name
}

output "mesh_type" {
  description = "Service mesh type"
  value       = module.mesh.type
}

output "mesh_namespace" {
  description = "Service mesh control plane namespace"
  value       = module.mesh.namespace
}
