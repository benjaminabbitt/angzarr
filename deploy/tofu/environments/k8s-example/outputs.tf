# Example K8s Environment - Outputs

output "namespace" {
  description = "Kubernetes namespace"
  value       = kubernetes_namespace.angzarr.metadata[0].name
}

output "topology_mermaid" {
  description = "Mermaid diagram of the topology"
  value       = module.stack.topology_mermaid
}

output "entry_points" {
  description = "Entry point domains (no inbound sagas/PMs)"
  value       = module.stack.entry_points
}

output "rust_features" {
  description = "Rust features required for this stack"
  value       = module.stack.rust_features
}

output "aggregate_urls" {
  description = "Internal URLs for domain aggregates"
  value       = module.stack.aggregate_urls
}

output "pm_urls" {
  description = "Internal URLs for process managers"
  value       = module.stack.pm_urls
}

output "postgres_service" {
  description = "PostgreSQL service name"
  value       = module.postgres.service_name
}

output "rabbit_service" {
  description = "RabbitMQ service name"
  value       = module.rabbit.service_name
}
