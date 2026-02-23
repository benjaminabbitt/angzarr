# Infrastructure Module: RabbitMQ - Outputs

output "bus" {
  description = "Bus configuration for stack module"
  value = {
    type           = "rabbit"
    connection_uri = "amqp://${var.username}:${local.password}@${local.service_name}.${var.namespace}.svc.cluster.local:5672"
    provides = {
      capabilities  = toset(["event_bus", "pub_sub", "topic_routing", "message_persistence"])
      rust_features = toset(["amqp"])
    }
  }
  sensitive = true
}

output "service_name" {
  description = "Kubernetes service name"
  value       = local.service_name
}

output "port" {
  description = "AMQP port"
  value       = 5672
}

output "management_port" {
  description = "Management UI port"
  value       = 15672
}
