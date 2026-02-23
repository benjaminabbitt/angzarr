# Infrastructure Module: Kafka - Outputs

output "bus" {
  description = "Bus configuration for stack module"
  value = {
    type           = "kafka"
    connection_uri = "${local.service_name}.${var.namespace}.svc.cluster.local:9092"
    provides = {
      capabilities  = toset(["pub_sub", "topic_routing", "message_persistence", "partitioning", "consumer_groups"])
      rust_features = toset(["kafka"])
    }
  }
}

output "bootstrap_servers" {
  description = "Kafka bootstrap servers"
  value       = "${local.service_name}.${var.namespace}.svc.cluster.local:9092"
}

output "service_name" {
  description = "Kubernetes service name"
  value       = local.service_name
}

output "port" {
  description = "Kafka client port"
  value       = 9092
}
