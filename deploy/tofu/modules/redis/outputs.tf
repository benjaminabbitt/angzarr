# Redis module outputs

output "host" {
  description = "Redis host"
  value       = local.host
}

output "port" {
  description = "Redis port"
  value       = local.port
}

output "uri" {
  description = "Redis connection URI"
  value       = local.uri
  sensitive   = true
}

output "secret_name" {
  description = "Kubernetes secret containing credentials"
  value       = kubernetes_secret.redis_credentials.metadata[0].name
}
