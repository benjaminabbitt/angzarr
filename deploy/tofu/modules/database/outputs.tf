# Database module outputs

output "host" {
  description = "Database host"
  value       = local.host
}

output "port" {
  description = "Database port"
  value       = local.port
}

output "uri" {
  description = "Database connection URI"
  value       = local.uri
  sensitive   = true
}

output "secret_name" {
  description = "Kubernetes secret containing credentials"
  value       = kubernetes_secret.database_credentials.metadata[0].name
}

output "type" {
  description = "Database type"
  value       = var.type
}
