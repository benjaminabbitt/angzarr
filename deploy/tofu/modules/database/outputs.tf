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

output "username" {
  description = "Database username"
  value       = var.username
}

output "password" {
  description = "Database password (generated or provided)"
  value       = local.user_password
  sensitive   = true
}

output "admin_password" {
  description = "Admin/root password (generated or provided)"
  value       = local.admin_password
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
