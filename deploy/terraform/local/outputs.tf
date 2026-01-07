output "namespace" {
  description = "Kubernetes namespace"
  value       = kubernetes_namespace.evented.metadata[0].name
}

output "evented_service" {
  description = "Evented service name"
  value       = "evented.${kubernetes_namespace.evented.metadata[0].name}.svc.cluster.local"
}

output "command_port" {
  description = "Command handler port"
  value       = 1313
}

output "query_port" {
  description = "Event query port"
  value       = 1314
}

output "rabbitmq_service" {
  description = "RabbitMQ service name"
  value       = var.enable_rabbitmq ? "rabbitmq.${kubernetes_namespace.evented.metadata[0].name}.svc.cluster.local" : null
}

output "redis_service" {
  description = "Redis service name"
  value       = var.enable_redis ? "redis-master.${kubernetes_namespace.evented.metadata[0].name}.svc.cluster.local" : null
}

output "storage_type" {
  description = "Storage backend type"
  value       = var.storage_type
}
