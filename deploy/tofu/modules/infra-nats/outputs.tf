# Infrastructure Module: NATS - Outputs

output "bus" {
  description = "Bus configuration for stack module"
  value = {
    type           = "nats"
    connection_uri = "nats://${local.service_name}.${var.namespace}.svc.cluster.local:4222"
    provides = {
      capabilities  = toset(["pub_sub", "subject_routing", "jetstream", "message_persistence"])
      rust_features = toset(["nats"])
    }
  }
}

output "connection_uri" {
  description = "NATS connection URI"
  value       = "nats://${local.service_name}.${var.namespace}.svc.cluster.local:4222"
}

output "service_name" {
  description = "Kubernetes service name"
  value       = local.service_name
}

output "port" {
  description = "NATS client port"
  value       = 4222
}

output "cluster_port" {
  description = "NATS cluster port"
  value       = 6222
}
