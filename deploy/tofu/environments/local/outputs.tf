# Local environment outputs

output "namespace" {
  description = "Kubernetes namespace"
  value       = kubernetes_namespace.angzarr.metadata[0].name
}

# MongoDB - for aggregates (event store)
output "mongodb_host" {
  description = "MongoDB host for aggregates"
  value       = module.mongodb.host
}

output "mongodb_secret" {
  description = "MongoDB credentials secret name"
  value       = module.mongodb.secret_name
}

# PostgreSQL - for projectors (read models)
output "postgresql_host" {
  description = "PostgreSQL host for projectors"
  value       = module.postgresql.host
}

output "postgresql_secret" {
  description = "PostgreSQL credentials secret name"
  value       = module.postgresql.secret_name
}

# RabbitMQ - messaging
output "messaging_host" {
  description = "Message broker host"
  value       = module.messaging.host
}

output "messaging_secret" {
  description = "Messaging credentials secret name"
  value       = module.messaging.secret_name
}

# Redis - cache/session
output "redis_host" {
  description = "Redis host"
  value       = module.redis.host
}

output "redis_secret" {
  description = "Redis credentials secret name"
  value       = module.redis.secret_name
}

# Observability
output "otel_collector_endpoint" {
  description = "OTel Collector OTLP gRPC endpoint (for OTEL_EXPORTER_OTLP_ENDPOINT)"
  value       = var.enable_observability ? module.observability[0].otel_collector_endpoint : ""
}

output "grafana_url" {
  description = "Grafana URL"
  value       = var.enable_observability ? module.observability[0].grafana_url : ""
}

output "observability_enabled" {
  description = "Whether observability stack is enabled"
  value       = var.enable_observability
}

output "mesh_enabled" {
  description = "Whether service mesh is enabled"
  value       = var.enable_mesh
}

output "secrets_source" {
  description = "Source of credentials"
  value       = "kubernetes_secret/${var.secrets_namespace}/angzarr-secrets"
}
