output "namespace" {
  description = "Kubernetes namespace"
  value       = kubernetes_namespace.angzarr.metadata[0].name
}

output "angzarr_service" {
  description = "Angzarr service name"
  value       = "angzarr.${kubernetes_namespace.angzarr.metadata[0].name}.svc.cluster.local"
}

output "command_port" {
  description = "Command handler port"
  value       = 1313
}

output "query_port" {
  description = "Event query port"
  value       = 1314
}

output "messaging_type" {
  description = "Messaging backend type (amqp or kafka)"
  value       = var.messaging_type
}

output "rabbitmq_service" {
  description = "RabbitMQ service name"
  value       = local.enable_rabbitmq ? "rabbitmq.${kubernetes_namespace.angzarr.metadata[0].name}.svc.cluster.local" : null
}

output "kafka_service" {
  description = "Kafka service name"
  value       = local.enable_kafka ? "kafka.${kubernetes_namespace.angzarr.metadata[0].name}.svc.cluster.local" : null
}

output "redis_service" {
  description = "Redis service name"
  value       = var.enable_redis ? "redis-master.${kubernetes_namespace.angzarr.metadata[0].name}.svc.cluster.local" : null
}

output "storage_type" {
  description = "Storage backend type"
  value       = var.storage_type
}

output "messaging_credentials_secret" {
  description = "Name of the Kubernetes secret containing messaging credentials"
  value       = kubernetes_secret.messaging_credentials.metadata[0].name
}
