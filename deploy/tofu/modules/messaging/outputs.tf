# Messaging module outputs

output "host" {
  description = "Message broker host"
  value       = local.host
}

output "port" {
  description = "Message broker port"
  value       = local.port
}

output "uri" {
  description = "Message broker connection URI"
  value       = local.uri
  sensitive   = true
}

output "secret_name" {
  description = "Kubernetes secret containing credentials"
  value       = kubernetes_secret.messaging_credentials.metadata[0].name
}

output "type" {
  description = "Message broker type"
  value       = var.type
}
