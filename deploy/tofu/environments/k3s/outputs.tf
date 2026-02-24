# k3s Environment Outputs

output "namespace" {
  description = "Kubernetes namespace"
  value       = kubernetes_namespace.angzarr.metadata[0].name
}

output "bus_type" {
  description = "Deployed message bus type"
  value       = var.bus_type
}

output "bus_service" {
  description = "Message bus service name"
  value = var.bus_type == "rabbit" ? (
    length(module.rabbitmq) > 0 ? module.rabbitmq[0].service_name : null
    ) : var.bus_type == "nats" ? (
    length(module.nats) > 0 ? module.nats[0].service_name : null
    ) : var.bus_type == "kafka" ? (
    length(module.kafka) > 0 ? module.kafka[0].service_name : null
  ) : null
}

output "postgres_service" {
  description = "PostgreSQL service name"
  value       = var.enable_postgres && length(module.postgres) > 0 ? module.postgres[0].service_name : null
}

output "redis_service" {
  description = "Redis service name"
  value       = var.enable_redis && length(module.redis) > 0 ? module.redis[0].service_name : null
}

output "postgres_secret" {
  description = "Kubernetes secret name for PostgreSQL credentials"
  value       = var.enable_postgres && length(kubernetes_secret.postgres_credentials) > 0 ? kubernetes_secret.postgres_credentials[0].metadata[0].name : null
}

output "amqp_secret" {
  description = "Kubernetes secret name for AMQP credentials"
  value       = var.bus_type == "rabbit" && length(kubernetes_secret.amqp_credentials) > 0 ? kubernetes_secret.amqp_credentials[0].metadata[0].name : null
}

output "nats_secret" {
  description = "Kubernetes secret name for NATS credentials"
  value       = var.bus_type == "nats" && length(kubernetes_secret.nats_credentials) > 0 ? kubernetes_secret.nats_credentials[0].metadata[0].name : null
}

output "registry_internal" {
  description = "Registry URL for k8s to pull images"
  value       = var.enable_registry && length(module.registry) > 0 ? module.registry[0].internal_url : null
}

output "registry_external" {
  description = "Registry URL for pushing images"
  value       = var.enable_registry && length(module.registry) > 0 ? module.registry[0].external_url : null
}
